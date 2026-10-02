@echo off
REM ===================================================================
REM  Lista de parametros de corte (.PAR) - SOLO LECTURA
REM  Resultado: parametros-bysoft.txt en esta carpeta.
REM ===================================================================
setlocal
set "BASE=C:\BystronicData\BySoftCam\8109C_BySprint_Fiber_4020_6000_BIMO2_POWERCUT"
set "OUT=%~dp0parametros-bysoft.txt"
if not exist "%BASE%\" (
  echo ERROR: no existe %BASE%
  pause
  exit /b 1
)
echo Listando parametros...
> "%OUT%" echo ==== PARAMETROS DE CORTE BYSOFT
>>"%OUT%" date /t
>>"%OUT%" echo Carpeta: %BASE%
>>"%OUT%" echo.
>>"%OUT%" echo ==== SUBCARPETAS
dir /s /b /ad "%BASE%" >>"%OUT%" 2>nul
>>"%OUT%" echo.
>>"%OUT%" echo ==== ARCHIVOS .PAR
dir /s /b "%BASE%\*.PAR" >>"%OUT%" 2>nul
>>"%OUT%" echo.
>>"%OUT%" echo ==== OTROS ARCHIVOS - no .PAR
dir /s /b /a-d "%BASE%" 2>nul | findstr /v /i /e ".PAR" >>"%OUT%"
echo Listo: %OUT%
echo Subelo.
pause
