@echo off
REM ===================================================================
REM  Catalogo BySoft - SOLO LECTURA
REM  Lista desde el indice de BySoft los materiales, maquinas, reglas de
REM  corte y gases (nombre + GUID). No modifica nada.
REM  Resultado: catalogo-bysoft.txt en esta carpeta.
REM ===================================================================
setlocal
if not defined BYSOFT_DIR set "BYSOFT_DIR=C:\Program Files\Bystronic\BySoft CAM\Programmer"
set "AQUI=%~dp0"
if "%AQUI:~-1%"=="\" set "AQUI=%AQUI:~0,-1%"
set "TOOL=%AQUI%\herramienta\BySoftCarpeta.exe"
set "OUT=%AQUI%\catalogo-bysoft.txt"

if not exist "%TOOL%" (
  echo ERROR: falta herramienta\BySoftCarpeta.exe
  goto fin
)
copy /y "%BYSOFT_DIR%\PartImporter.exe.config" "%TOOL%.config" >nul
if errorlevel 1 (
  echo ERROR: no se pudo copiar la configuracion de BySoft
  goto fin
)
echo Leyendo catalogo de BySoft...
> "%OUT%" echo ==== CATALOGO BYSOFT
>>"%OUT%" date /t
>>"%OUT%" time /t
>>"%OUT%" echo.
>>"%OUT%" echo ==== BASES
"%TOOL%" info >>"%OUT%" 2>&1
>>"%OUT%" echo.
>>"%OUT%" echo ==== SYSTEM
"%TOOL%" catalogo System >>"%OUT%" 2>&1
if errorlevel 1 (
  echo ERROR al leer el catalogo. Revisa catalogo-bysoft.txt
) else (
  echo Listo.
)

:fin
echo.
echo Sube este archivo: %OUT%
pause
exit /b
