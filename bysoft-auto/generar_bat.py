# Genera Ejecutar-diagnostico.bat con diagnostico-bysoft.ps1 incrustado (un solo archivo).
# Uso: python generar_bat.py
from pathlib import Path

here = Path(__file__).parent
ps1 = (here / "diagnostico-bysoft.ps1").read_text(encoding="ascii").replace("\r\n", "\n")

bat = r"""@echo off
REM Diagnostico BySoft (SOLO LECTURA). Archivo unico: el script PowerShell va incrustado al final.
set "ME=%~f0"
where powershell >nul 2>&1
if errorlevel 1 (
  echo ERROR: PowerShell no esta disponible en este PC.
  pause
  exit /b 1
)
echo Ejecutando diagnostico, puede tardar 1-2 minutos...
echo.
powershell -NoProfile -ExecutionPolicy Bypass -Command "$t=[IO.File]::ReadAllText($env:ME); $m='::'+' ===PS1==='; $i=$t.LastIndexOf($m); if($i -lt 0){Write-Host 'ERROR: script incrustado no encontrado'; exit 2}; & ([ScriptBlock]::Create($t.Substring($i+$m.Length)))"
echo.
echo Codigo de salida: %ERRORLEVEL%
echo Haz una captura de esta ventana si algo salio mal.
pause
exit /b
:: ===PS1===
""" + ps1

(here / "Ejecutar-diagnostico.bat").write_bytes(bat.replace("\r\n", "\n").replace("\n", "\r\n").encode("ascii"))
print("OK")
