@echo off
REM Registros BySoft - SOLO LECTURA.
REM Revisa UNICAMENTE la carpeta donde esta este .bat y sus subcarpetas.
REM Resultado: registros-bysoft.txt en esa misma carpeta.
setlocal
set "BASE=%~dp0"
if "%BASE:~-1%"=="\" set "BASE=%BASE:~0,-1%"
set "OUT=%BASE%\registros-bysoft.txt"
set "LISTA=%TEMP%\bysoft_lista.txt"
set /a MAX=500

echo Revisando: %BASE%
echo Si la carpeta es grande o esta en red puede tardar. Para cancelar: Ctrl+C
echo.
> "%OUT%" echo ==== REGISTROS DE CARPETA
>>"%OUT%" echo Carpeta: %BASE%
>>"%OUT%" date /t
>>"%OUT%" time /t

echo [1/3] Listando carpetas...
>>"%OUT%" echo.
>>"%OUT%" echo ==== 1. CARPETAS - maximo %MAX%
dir /s /b /ad "%BASE%" > "%LISTA%" 2>nul
call :volcar

echo [2/3] Listando archivos...
>>"%OUT%" echo.
>>"%OUT%" echo ==== 2. ARCHIVOS - sin .png, maximo %MAX%
dir /s /b /a-d "%BASE%" > "%LISTA%" 2>nul
call :volcar

echo [3/3] Leyendo archivos .pis...
>>"%OUT%" echo.
>>"%OUT%" echo ==== 3. CONTENIDO DE ARCHIVOS .pis
for /f "delims=" %%F in ('dir /s /b "%BASE%\*.pis" 2^>nul') do (
  >>"%OUT%" echo ----- %%F
  type "%%F" >>"%OUT%"
  >>"%OUT%" echo.
)

>>"%OUT%" echo.
>>"%OUT%" echo ==== FIN
if exist "%LISTA%" del "%LISTA%"
echo.
echo Listo: %OUT%
echo Revisalo y subelo.
pause
exit /b

:volcar
REM Copia LISTA al registro: maximo MAX lineas, sin .png, y cuenta por tipo
setlocal EnableDelayedExpansion
set /a T=0, CBOX=0, CLOCK=0, CDXF=0, CPIS=0, CDB=0
for /f "usebackq delims=" %%L in ("%LISTA%") do (
  if /i not "%%~xL"==".png" if /i not "%%~nxL"=="registros-bysoft.txt" (
    set /a T+=1
    if !T! LEQ %MAX% >>"%OUT%" echo %%L
    if /i "%%~xL"==".box" set /a CBOX+=1
    if /i "%%~xL"==".lock" set /a CLOCK+=1
    if /i "%%~xL"==".dxf" set /a CDXF+=1
    if /i "%%~xL"==".pis" set /a CPIS+=1
    if /i "%%~xL"==".db" set /a CDB+=1
  )
)
>>"%OUT%" echo Total: !T!   .box=!CBOX!  .lock=!CLOCK!  .dxf=!CDXF!  .pis=!CPIS!  .db=!CDB!
echo       Total: !T!
endlocal
exit /b
