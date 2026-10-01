@echo off
REM Registros BySoft - SOLO LECTURA.
REM Revisa UNICAMENTE la carpeta donde esta este .bat y sus subcarpetas.
REM Resultado: registros-bysoft.txt en esa misma carpeta.
setlocal
set "BASE=%~dp0"
if "%BASE:~-1%"=="\" set "BASE=%BASE:~0,-1%"
set "OUT=%BASE%\registros-bysoft.txt"
set /a N=0
set /a MAX=500

echo Revisando: %BASE%
echo Espera...
> "%OUT%" echo ==== REGISTROS DE CARPETA
>>"%OUT%" echo Carpeta: %BASE%
>>"%OUT%" date /t
>>"%OUT%" time /t

>>"%OUT%" echo.
>>"%OUT%" echo ==== 1. CARPETAS
dir /s /b /ad "%BASE%" >>"%OUT%" 2>nul

>>"%OUT%" echo.
>>"%OUT%" echo ==== 2. ARCHIVOS - sin .png, maximo %MAX%
for /f "delims=" %%F in ('dir /s /b /a-d "%BASE%" 2^>nul') do call :archivo "%%F"
>>"%OUT%" echo Total listados: %N%

>>"%OUT%" echo.
>>"%OUT%" echo ==== 3. CONTENIDO DE ARCHIVOS .pis
for /f "delims=" %%F in ('dir /s /b "%BASE%\*.pis" 2^>nul') do (
  >>"%OUT%" echo ----- %%F
  type "%%F" >>"%OUT%"
  >>"%OUT%" echo.
)

>>"%OUT%" echo.
>>"%OUT%" echo ==== FIN
echo.
echo Listo: %OUT%
echo Revisalo y subelo.
pause
exit /b

:archivo
if /i "%~x1"==".png" exit /b
if /i "%~f1"=="%OUT%" exit /b
if %N% GEQ %MAX% exit /b
set /a N+=1
>>"%OUT%" echo %~1    %~z1 bytes    %~t1
exit /b
