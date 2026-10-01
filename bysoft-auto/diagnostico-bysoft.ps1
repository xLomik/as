# Diagnostico BySoft CAM - SOLO LECTURA.
# No ejecuta ningun .exe de BySoft, no modifica nada. Recoge evidencia para
# decidir como automatizar importacion de piezas y creacion de nesteos.
# Resultado: Escritorio\diagnostico-bysoft.txt  (revisalo antes de enviarlo)

$ErrorActionPreference = 'SilentlyContinue'
Write-Host 'Diagnostico BySoft iniciado...'
$out = Join-Path ([Environment]::GetFolderPath('Desktop')) 'diagnostico-bysoft.txt'
$log = New-Object System.Collections.Generic.List[string]
function LogLine([string]$s) { $log.Add($s) }
function LogHead([string]$s) { Write-Host "  - $s"; LogLine ''; LogLine ('=' * 70); LogLine $s; LogLine ('=' * 70) }
# Oculta contrasenas en cadenas de conexion / configs
function Mask([string]$s) {
    $s -replace '(?i)(password|pwd|secret|apikey|api_key|token)\s*([=:])\s*("?)[^;"<\s]+', '$1$2$3****'
}

LogHead 'SISTEMA'
LogLine ("Fecha: " + (Get-Date -Format 's'))
LogLine ("Windows: " + (Get-CimInstance Win32_OperatingSystem).Caption)
LogLine ("PowerShell: " + $PSVersionTable.PSVersion)

# --- 1. Localizar instalacion -------------------------------------------------
LogHead 'CARPETAS DE INSTALACION'
$roots = @()
foreach ($p in Get-Process | Where-Object { $_.Path -match '(?i)bysoft|bystronic' }) {
    $roots += Split-Path $p.Path
}
foreach ($base in @($env:ProgramFiles, ${env:ProgramFiles(x86)}, $env:ProgramData)) {
    if ($base) { $roots += (Get-ChildItem $base -Directory | Where-Object { $_.Name -match '(?i)bystronic|bysoft' }).FullName }
}
$roots = $roots | Where-Object { $_ } | Sort-Object -Unique
$roots | ForEach-Object { LogLine $_ }

$exeDirs = @()
foreach ($r in $roots) {
    $exeDirs += (Get-ChildItem $r -Recurse -Filter 'PartImporter.exe' | Select-Object -ExpandProperty DirectoryName)
}
$exeDirs = $exeDirs | Sort-Object -Unique
LogLine ''; LogLine 'Carpetas con PartImporter.exe:'; $exeDirs | ForEach-Object { LogLine "  $_" }

# --- 2. Ejecutables: version y descripcion ------------------------------------
LogHead 'EJECUTABLES (version / descripcion)'
foreach ($d in $exeDirs) {
    foreach ($f in Get-ChildItem $d -Filter '*.exe') {
        $v = $f.VersionInfo
        LogLine ("{0,-55} {1,-14} {2}" -f $f.Name, $v.FileVersion, $v.FileDescription)
    }
}

# --- 3. Archivos de configuracion de los exe clave ----------------------------
LogHead 'CONFIGURACION (.config / .json) DE EXE CLAVE'
$keys = 'PartImporter|ApiService|Massmutation|PartInfoUpdater|PreviewGenerator|BosBase|DwgConverter|BySoftCAM'
foreach ($d in $exeDirs) {
    foreach ($f in Get-ChildItem $d -File | Where-Object { $_.Name -match "(?i)($keys).*\.(config|json)$" -or $_.Name -match '(?i)^appsettings.*\.json$' }) {
        LogLine ''; LogLine "----- $($f.FullName)"
        Get-Content $f.FullName -TotalCount 300 | ForEach-Object { LogLine (Mask $_) }
    }
}

# --- 4. Servicios y puertos (ApiService) --------------------------------------
LogHead 'SERVICIOS WINDOWS BYSOFT/BYSTRONIC'
Get-CimInstance Win32_Service | Where-Object { $_.Name -match '(?i)bysoft|bystronic|bos' -or $_.PathName -match '(?i)bysoft|bystronic' } |
    ForEach-Object { LogLine ("{0} | {1} | {2} | {3}" -f $_.Name, $_.State, $_.StartMode, $_.PathName) }

LogHead 'PUERTOS EN ESCUCHA DE PROCESOS BYSOFT'
$procs = Get-Process | Where-Object { $_.Path -match '(?i)bysoft|bystronic' }
$ports = @()
foreach ($p in $procs) {
    foreach ($c in Get-NetTCPConnection -State Listen -OwningProcess $p.Id) {
        LogLine ("{0} (pid {1}) -> {2}:{3}" -f $p.ProcessName, $p.Id, $c.LocalAddress, $c.LocalPort)
        $ports += $c.LocalPort
    }
}

LogHead 'SONDEO HTTP LOCAL (solo GET a documentacion de API)'
foreach ($port in ($ports | Sort-Object -Unique)) {
    foreach ($path in '/', '/swagger', '/swagger/index.html', '/swagger/v1/swagger.json', '/api', '/help') {
        foreach ($scheme in 'http', 'https') {
            $u = "${scheme}://localhost:$port$path"
            try {
                $r = Invoke-WebRequest $u -UseBasicParsing -TimeoutSec 3
                LogLine ("{0} -> {1} ({2} bytes)" -f $u, $r.StatusCode, $r.RawContentLength)
                if ($path -like '*swagger.json') { LogLine ($r.Content.Substring(0, [Math]::Min(20000, $r.Content.Length))) }
            } catch { }
        }
    }
}

# --- 5. Pistas de linea de comandos dentro de los ensamblados -----------------
LogHead 'CADENAS TIPO ARGUMENTO EN EXE CLAVE (posible linea de comandos)'
foreach ($d in $exeDirs) {
    foreach ($name in 'PartImporter.exe', 'Massmutation.exe', 'PartInfoUpdater.exe', 'BySoftCAM.exe', 'Bystronic.BySoft.ApiService.exe') {
        $f = Join-Path $d $name
        if (-not (Test-Path $f)) { continue }
        $bytes = [IO.File]::ReadAllBytes($f)
        $txt = [Text.Encoding]::Unicode.GetString($bytes) + [Text.Encoding]::ASCII.GetString($bytes)
        $hits = [regex]::Matches($txt, '(?<![\w])(--?|/)[A-Za-z][A-Za-z]{2,30}(?=[\s:=\x00"])') |
            ForEach-Object { $_.Value } | Sort-Object -Unique | Select-Object -First 200
        LogLine ''; LogLine "----- $name"; LogLine ($hits -join '  ')
        $kw = [regex]::Matches($txt, '(?i)[\w\.]*(watch|hotfolder|hot folder|import|nest|order|job|excel|csv|xml|quantity|cantidad)[\w\.]*') |
            ForEach-Object { $_.Value } | Where-Object { $_.Length -lt 60 } | Sort-Object -Unique | Select-Object -First 300
        LogLine ''; LogLine "  palabras clave:"; LogLine ('  ' + ($kw -join '  '))
    }
}

# --- 6. Tipos .NET publicos (solo metadatos, no ejecuta codigo) ---------------
LogHead 'TIPOS .NET EN ENSAMBLADOS DE API/IMPORT/NEST'
foreach ($d in $exeDirs) {
    foreach ($f in Get-ChildItem $d -File | Where-Object { $_.Name -match '(?i)(Api|Import|Nest|Order|Job|Automation|Interface).*\.(dll|exe)$' }) {
        try {
            $asm = [Reflection.Assembly]::ReflectionOnlyLoadFrom($f.FullName)
            $types = $asm.GetExportedTypes() | ForEach-Object { $_.FullName } | Select-Object -First 150
            LogLine ''; LogLine "----- $($f.Name)"; $types | ForEach-Object { LogLine "  $_" }
        } catch {
            LogLine ''; LogLine "----- $($f.Name) (no .NET clasico / no se pudo leer metadatos)"
        }
    }
}

# --- 7. Listado de DLL (nombres) ---------------------------------------------
LogHead 'DLL INSTALADAS (nombres)'
foreach ($d in $exeDirs) { (Get-ChildItem $d -Filter '*.dll').Name -join '  ' | ForEach-Object { LogLine $_ } }

try {
    $log | Set-Content -Path $out -Encoding UTF8 -ErrorAction Stop
} catch {
    # Escritorio no escribible (politica / OneDrive): guardar junto al .bat o en Documentos
    $out = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'diagnostico-bysoft.txt'
    $log | Set-Content -Path $out -Encoding UTF8
}
Write-Host ''
Write-Host "LISTO. Archivo generado: $out"

