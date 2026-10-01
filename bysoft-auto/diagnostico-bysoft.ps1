# Diagnostico BySoft CAM - SOLO LECTURA.
# No ejecuta ningun .exe de BySoft, no modifica nada. Recoge evidencia para
# decidir como automatizar importacion de piezas y creacion de nesteos.
# Resultado: Escritorio\diagnostico-bysoft.txt  (revisalo antes de enviarlo)

$ErrorActionPreference = 'SilentlyContinue'
$out = Join-Path ([Environment]::GetFolderPath('Desktop')) 'diagnostico-bysoft.txt'
$log = New-Object System.Collections.Generic.List[string]
function W([string]$s) { $log.Add($s) }
function H([string]$s) { W ''; W ('=' * 70); W $s; W ('=' * 70) }
# Oculta contrasenas en cadenas de conexion / configs
function Mask([string]$s) {
    $s -replace '(?i)(password|pwd|secret|apikey|api_key|token)\s*([=:])\s*("?)[^;"<\s]+', '$1$2$3****'
}

H 'SISTEMA'
W ("Fecha: " + (Get-Date -Format 's'))
W ("Windows: " + (Get-CimInstance Win32_OperatingSystem).Caption)
W ("PowerShell: " + $PSVersionTable.PSVersion)

# --- 1. Localizar instalacion -------------------------------------------------
H 'CARPETAS DE INSTALACION'
$roots = @()
foreach ($p in Get-Process | Where-Object { $_.Path -match '(?i)bysoft|bystronic' }) {
    $roots += Split-Path $p.Path
}
foreach ($base in @($env:ProgramFiles, ${env:ProgramFiles(x86)}, $env:ProgramData)) {
    if ($base) { $roots += (Get-ChildItem $base -Directory | Where-Object { $_.Name -match '(?i)bystronic|bysoft' }).FullName }
}
$roots = $roots | Where-Object { $_ } | Sort-Object -Unique
$roots | ForEach-Object { W $_ }

$exeDirs = @()
foreach ($r in $roots) {
    $exeDirs += (Get-ChildItem $r -Recurse -Filter 'PartImporter.exe' | Select-Object -ExpandProperty DirectoryName)
}
$exeDirs = $exeDirs | Sort-Object -Unique
W ''; W 'Carpetas con PartImporter.exe:'; $exeDirs | ForEach-Object { W "  $_" }

# --- 2. Ejecutables: version y descripcion ------------------------------------
H 'EJECUTABLES (version / descripcion)'
foreach ($d in $exeDirs) {
    foreach ($f in Get-ChildItem $d -Filter '*.exe') {
        $v = $f.VersionInfo
        W ("{0,-55} {1,-14} {2}" -f $f.Name, $v.FileVersion, $v.FileDescription)
    }
}

# --- 3. Archivos de configuracion de los exe clave ----------------------------
H 'CONFIGURACION (.config / .json) DE EXE CLAVE'
$keys = 'PartImporter|ApiService|Massmutation|PartInfoUpdater|PreviewGenerator|BosBase|DwgConverter|BySoftCAM'
foreach ($d in $exeDirs) {
    foreach ($f in Get-ChildItem $d -File | Where-Object { $_.Name -match "(?i)($keys).*\.(config|json)$" -or $_.Name -match '(?i)^appsettings.*\.json$' }) {
        W ''; W "----- $($f.FullName)"
        Get-Content $f.FullName -TotalCount 300 | ForEach-Object { W (Mask $_) }
    }
}

# --- 4. Servicios y puertos (ApiService) --------------------------------------
H 'SERVICIOS WINDOWS BYSOFT/BYSTRONIC'
Get-CimInstance Win32_Service | Where-Object { $_.Name -match '(?i)bysoft|bystronic|bos' -or $_.PathName -match '(?i)bysoft|bystronic' } |
    ForEach-Object { W ("{0} | {1} | {2} | {3}" -f $_.Name, $_.State, $_.StartMode, $_.PathName) }

H 'PUERTOS EN ESCUCHA DE PROCESOS BYSOFT'
$procs = Get-Process | Where-Object { $_.Path -match '(?i)bysoft|bystronic' }
$ports = @()
foreach ($p in $procs) {
    foreach ($c in Get-NetTCPConnection -State Listen -OwningProcess $p.Id) {
        W ("{0} (pid {1}) -> {2}:{3}" -f $p.ProcessName, $p.Id, $c.LocalAddress, $c.LocalPort)
        $ports += $c.LocalPort
    }
}

H 'SONDEO HTTP LOCAL (solo GET a documentacion de API)'
foreach ($port in ($ports | Sort-Object -Unique)) {
    foreach ($path in '/', '/swagger', '/swagger/index.html', '/swagger/v1/swagger.json', '/api', '/help') {
        foreach ($scheme in 'http', 'https') {
            $u = "${scheme}://localhost:$port$path"
            try {
                $r = Invoke-WebRequest $u -UseBasicParsing -TimeoutSec 3
                W ("{0} -> {1} ({2} bytes)" -f $u, $r.StatusCode, $r.RawContentLength)
                if ($path -like '*swagger.json') { W ($r.Content.Substring(0, [Math]::Min(20000, $r.Content.Length))) }
            } catch { }
        }
    }
}

# --- 5. Pistas de linea de comandos dentro de los ensamblados -----------------
H 'CADENAS TIPO ARGUMENTO EN EXE CLAVE (posible linea de comandos)'
foreach ($d in $exeDirs) {
    foreach ($name in 'PartImporter.exe', 'Massmutation.exe', 'PartInfoUpdater.exe', 'BySoftCAM.exe', 'Bystronic.BySoft.ApiService.exe') {
        $f = Join-Path $d $name
        if (-not (Test-Path $f)) { continue }
        $bytes = [IO.File]::ReadAllBytes($f)
        $txt = [Text.Encoding]::Unicode.GetString($bytes) + [Text.Encoding]::ASCII.GetString($bytes)
        $hits = [regex]::Matches($txt, '(?<![\w])(--?|/)[A-Za-z][A-Za-z]{2,30}(?=[\s:=\x00"])') |
            ForEach-Object { $_.Value } | Sort-Object -Unique | Select-Object -First 200
        W ''; W "----- $name"; W ($hits -join '  ')
        $kw = [regex]::Matches($txt, '(?i)[\w\.]*(watch|hotfolder|hot folder|import|nest|order|job|excel|csv|xml|quantity|cantidad)[\w\.]*') |
            ForEach-Object { $_.Value } | Where-Object { $_.Length -lt 60 } | Sort-Object -Unique | Select-Object -First 300
        W ''; W "  palabras clave:"; W ('  ' + ($kw -join '  '))
    }
}

# --- 6. Tipos .NET publicos (solo metadatos, no ejecuta codigo) ---------------
H 'TIPOS .NET EN ENSAMBLADOS DE API/IMPORT/NEST'
foreach ($d in $exeDirs) {
    foreach ($f in Get-ChildItem $d -File | Where-Object { $_.Name -match '(?i)(Api|Import|Nest|Order|Job|Automation|Interface).*\.(dll|exe)$' }) {
        try {
            $asm = [Reflection.Assembly]::ReflectionOnlyLoadFrom($f.FullName)
            $types = $asm.GetExportedTypes() | ForEach-Object { $_.FullName } | Select-Object -First 150
            W ''; W "----- $($f.Name)"; $types | ForEach-Object { W "  $_" }
        } catch {
            W ''; W "----- $($f.Name) (no .NET clasico / no se pudo leer metadatos)"
        }
    }
}

# --- 7. Listado de DLL (nombres) ---------------------------------------------
H 'DLL INSTALADAS (nombres)'
foreach ($d in $exeDirs) { (Get-ChildItem $d -Filter '*.dll').Name -join '  ' | ForEach-Object { W $_ } }

$log | Set-Content -Path $out -Encoding UTF8
Write-Host "Listo: $out"
