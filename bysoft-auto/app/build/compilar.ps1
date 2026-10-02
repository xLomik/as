# Compila AutoBySoft.exe (C# 5, .NET Framework 4.7.2, x64) con Roslyn.
# Requiere: PowerShell 7 (trae Microsoft.CodeAnalysis), las referencias de .NET Framework 4.7.2
# (paquete nuget Microsoft.NETFramework.ReferenceAssemblies.net472) y
# Bystronic.BySoft.Common.Persistence.dll (solo para compilar; no se distribuye).
param(
    [Parameter(Mandatory=$true)][string]$Ref472,      # carpeta build/.NETFramework/v4.7.2
    [Parameter(Mandatory=$true)][string]$Persistence, # ruta a Bystronic.BySoft.Common.Persistence.dll
    [string]$Salida = (Join-Path $PSScriptRoot "..\dist\AutoBySoft.exe"),
    [string]$Tipo = "WindowsApplication"
)
$pwshDir = Split-Path ([System.Diagnostics.Process]::GetCurrentProcess().MainModule.FileName)
Add-Type -Path (Join-Path $pwshDir "Microsoft.CodeAnalysis.dll")
Add-Type -Path (Join-Path $pwshDir "Microsoft.CodeAnalysis.CSharp.dll")
$refs = New-Object 'System.Collections.Generic.List[Microsoft.CodeAnalysis.MetadataReference]'
foreach ($n in 'mscorlib','System','System.Core','System.Configuration','System.Xml','System.Xml.Linq',
               'System.Windows.Forms','System.Drawing','System.IO.Compression','System.IO.Compression.FileSystem') {
    $refs.Add([Microsoft.CodeAnalysis.MetadataReference]::CreateFromFile((Join-Path $Ref472 "$n.dll")))
}
$refs.Add([Microsoft.CodeAnalysis.MetadataReference]::CreateFromFile($Persistence))
$opt = [Microsoft.CodeAnalysis.CSharp.CSharpParseOptions]::new([Microsoft.CodeAnalysis.CSharp.LanguageVersion]::CSharp5)
$arboles = foreach ($f in Get-ChildItem (Join-Path $PSScriptRoot "..\src") -Filter *.cs) {
    [Microsoft.CodeAnalysis.CSharp.CSharpSyntaxTree]::ParseText((Get-Content -Raw $f.FullName), $opt, $f.FullName)
}
$kind = [Microsoft.CodeAnalysis.OutputKind]::$Tipo
$copt = [Microsoft.CodeAnalysis.CSharp.CSharpCompilationOptions]::new($kind).WithPlatform([Microsoft.CodeAnalysis.Platform]::X64).WithOptimizationLevel([Microsoft.CodeAnalysis.OptimizationLevel]::Release)
$comp = [Microsoft.CodeAnalysis.CSharp.CSharpCompilation]::Create("AutoBySoft", [Microsoft.CodeAnalysis.SyntaxTree[]]$arboles, $refs, $copt)
New-Item -ItemType Directory -Force (Split-Path $Salida) | Out-Null
$fs = [IO.File]::Create($Salida); $r = $comp.Emit($fs); $fs.Close()
$r.Diagnostics | Where-Object { $_.Severity -eq 'Error' -or $_.Severity -eq 'Warning' } | ForEach-Object { $_.ToString() }
if (-not $r.Success) { Remove-Item $Salida -ErrorAction SilentlyContinue; "COMPILACION FALLIDA"; exit 1 }
"OK $Salida"
