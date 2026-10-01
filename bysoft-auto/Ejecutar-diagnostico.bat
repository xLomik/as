@echo off
REM Ejecuta el diagnostico (solo lectura). Muestra cada paso para saber donde falla.
cd /d "%~dp0"
echo Carpeta: %CD%
echo.
echo Archivos en esta carpeta:
dir /b
echo.
set "PS1="
for %%F in (*diagnostico-bysoft*.ps1 *diagnostico-bysoft*.ps1.txt *diagnostico-bysoft*.txt) do if not defined PS1 if /i not "%%~nxF"=="diagnostico-bysoft.txt" set "PS1=%%F"
if not defined PS1 (
  echo ERROR: no encuentro diagnostico-bysoft.ps1 en esta carpeta.
  echo Pon el .bat y el .ps1 juntos en la misma carpeta.
  goto fin
)
echo Usando script: %PS1%
copy /y "%PS1%" "%TEMP%\diagnostico-bysoft.ps1" >nul
where powershell >nul 2>&1 || (echo ERROR: PowerShell no esta disponible en este PC. & goto fin)
echo Ejecutando, puede tardar 1-2 minutos...
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%TEMP%\diagnostico-bysoft.ps1"
echo.
echo Codigo de salida de PowerShell: %ERRORLEVEL%
:fin
echo.
echo Haz una captura de esta ventana si algo salio mal.
pause
