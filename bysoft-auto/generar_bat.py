# Genera Ejecutar-diagnostico.bat con diagnostico-bysoft.ps1 incrustado (un solo archivo).
# Uso: python generar_bat.py
from pathlib import Path

here = Path(__file__).parent
ps1 = (here / "diagnostico-bysoft.ps1").read_text(encoding="ascii").replace("\r\n", "\n")

bat = r"""@echo off
REM Diagnostico BySoft (SOLO LECTURA). Archivo unico: el script PowerShell va incrustado al final.
REM Parte 1 usa solo comandos de cmd (funciona aunque PowerShell este restringido).
REM Parte 2 usa PowerShell para el detalle.
set "ME=%~f0"
set "PF86=%ProgramFiles(x86)%"
set "OUT=%~dp0diagnostico-bysoft-cmd.txt"

echo [1/2] Diagnostico basico (cmd)...
> "%OUT%" (
  echo ==== FECHA / WINDOWS
  date /t
  time /t
  ver
  echo.
  echo ==== CARPETAS BYSTRONIC / BYSOFT
  for %%R in ("%ProgramFiles%" "%PF86%" "%ProgramData%") do for /d %%D in ("%%~R\*Bystronic*" "%%~R\*BySoft*") do echo %%~fD
  echo.
  echo ==== EJECUTABLES Y CONFIGURACION
  for %%R in ("%ProgramFiles%" "%PF86%") do for /d %%D in ("%%~R\*Bystronic*" "%%~R\*BySoft*") do dir /s /b "%%~fD\*.exe" "%%~fD\*.exe.config" "%%~fD\appsettings*.json" 2>nul
  echo.
  echo ==== CONTENIDO DE CONFIGS CLAVE - lineas con password o token omitidas
  for %%R in ("%ProgramFiles%" "%PF86%") do for /d %%D in ("%%~R\*Bystronic*" "%%~R\*BySoft*") do for /f "delims=" %%F in ('dir /s /b "%%~fD\PartImporter.exe.config" "%%~fD\Bystronic.BySoft.ApiService*.config" "%%~fD\Massmutation.exe.config" "%%~fD\BySoftCAM.exe.config" "%%~fD\appsettings*.json" 2^>nul') do (
    echo ----- %%F
    findstr /v /i "password pwd secret token apikey" "%%F"
  )
  echo.
  echo ==== PROCESOS
  tasklist /fo csv /nh | findstr /i "bysoft bystronic partimporter"
  echo.
  echo ==== SERVICIOS
  sc query state= all | findstr /i "bysoft bystronic"
  echo.
  echo ==== PUERTOS EN ESCUCHA DE ESOS PROCESOS
  for /f "tokens=1,2 delims=," %%A in ('tasklist /fo csv /nh ^| findstr /i "bysoft bystronic partimporter"') do (
    echo -- %%~A pid %%~B
    netstat -ano | findstr /i "LISTENING" | findstr /e /c:" %%~B"
  )
)
echo       Guardado: %OUT%
echo.
echo [2/2] Diagnostico detallado (PowerShell)...
where powershell >nul 2>&1
if errorlevel 1 (
  echo ERROR: PowerShell no esta disponible en este PC.
  goto :fin
)
powershell -NoProfile -ExecutionPolicy Bypass -Command "Write-Output ('PowerShell ' + $PSVersionTable.PSVersion + ' / modo de lenguaje: ' + $ExecutionContext.SessionState.LanguageMode); try { $t=[IO.File]::ReadAllText($env:ME); $m='::'+' ===PS1==='; $i=$t.LastIndexOf($m, [StringComparison]::Ordinal); Write-Output ('Script incrustado: posicion ' + $i + ' de ' + $t.Length); if ($i -lt 0) { throw 'script incrustado no encontrado' }; $sb=[ScriptBlock]::Create($t.Substring($i + $m.Length)) } catch { Write-Output ('ERROR al preparar el script: ' + $_); exit 3 }; & $sb"
echo Codigo de salida PowerShell: %ERRORLEVEL%
:fin
echo.
echo Envia: %OUT%
echo y, si aparecio LISTO arriba, tambien diagnostico-bysoft.txt
echo Haz una captura de esta ventana.
pause
exit /b
:: ===PS1===
""" + ps1

(here / "Ejecutar-diagnostico.bat").write_bytes(bat.replace("\r\n", "\n").replace("\n", "\r\n").encode("ascii"))
print("OK")
