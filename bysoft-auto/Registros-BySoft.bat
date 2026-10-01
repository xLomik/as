@echo off
REM Registros BySoft - SOLO LECTURA. Solo usa comandos de cmd.
REM Recoge: rutas de la base de piezas, configs de usuario, archivos .pis
REM y la estructura de carpetas de piezas. Resultado: registros-bysoft.txt junto a este .bat
setlocal
set "OUT=%~dp0registros-bysoft.txt"
set "TMPL=%TEMP%\bysoft_rutas.txt"
if exist "%TMPL%" del "%TMPL%"

echo Recogiendo registros, espera...
> "%OUT%" echo ==== REGISTROS BYSOFT
>>"%OUT%" date /t
>>"%OUT%" time /t
>>"%OUT%" echo Usuario: %USERNAME%  Equipo: %COMPUTERNAME%

REM ---- 1. Archivos de configuracion de usuario y comunes
>>"%OUT%" echo.
>>"%OUT%" echo ==== 1. ARCHIVOS .config DE BYSTRONIC EN PERFIL Y PROGRAMDATA
for %%R in ("%APPDATA%" "%LOCALAPPDATA%" "%ProgramData%") do (
  if exist "%%~R\Bystronic" dir /s /b "%%~R\Bystronic\*.config" >>"%OUT%" 2>nul
)

REM ---- 2. Common.config: contenido y rutas
>>"%OUT%" echo.
>>"%OUT%" echo ==== 2. CONTENIDO DE Common.config - lineas con password o secret omitidas
for %%R in ("%APPDATA%" "%LOCALAPPDATA%" "%ProgramData%") do (
  if exist "%%~R\Bystronic" for /f "delims=" %%F in ('dir /s /b "%%~R\Bystronic\Common.config" 2^>nul') do (
    >>"%OUT%" echo ----- %%F
    findstr /v /i "password pwd secret" "%%F" >>"%OUT%"
    >>"%OUT%" echo.
    call :extraer "%%F"
  )
)

REM ---- 3. user.config de BySoft
>>"%OUT%" echo.
>>"%OUT%" echo ==== 3. CONFIGS DE USUARIO DE BYSOFT - user.config, lineas con rutas o carpetas
for %%R in ("%APPDATA%" "%LOCALAPPDATA%") do (
  for /f "delims=" %%F in ('dir /s /b "%%~R\user.config" 2^>nul ^| findstr /i "bystronic bysoft partimporter"') do (
    >>"%OUT%" echo ----- %%F
    findstr /i "path folder dir name=" "%%F" | findstr /v /i "password pwd secret" >>"%OUT%"
  )
)

REM ---- 4. Archivos .pis
>>"%OUT%" echo.
>>"%OUT%" echo ==== 4. ARCHIVOS .pis EN ESCRITORIO, DOCUMENTOS Y BYSTRONICDATA
for %%R in ("%USERPROFILE%\Desktop" "%USERPROFILE%\Documents" "%USERPROFILE%\OneDrive\Desktop" "C:\BystronicData") do (
  if exist "%%~R" for /f "delims=" %%F in ('dir /s /b "%%~R\*.pis" 2^>nul') do (
    >>"%OUT%" echo ----- %%F
    type "%%F" >>"%OUT%"
    >>"%OUT%" echo.
  )
)

REM ---- 5. Estructura de cada ruta encontrada
>>"%OUT%" echo.
>>"%OUT%" echo ==== 5. RUTAS ENCONTRADAS EN LAS CONFIGS Y SU ESTRUCTURA
>>"%TMPL%" echo C:\BystronicData\BySoftCam
for /f "usebackq delims=" %%P in ("%TMPL%") do call :ruta "%%P"

>>"%OUT%" echo.
>>"%OUT%" echo ==== FIN
if exist "%TMPL%" del "%TMPL%"
echo.
echo Listo: %OUT%
echo Revisalo y subelo.
pause
exit /b

:extraer
REM Guarda en TMPL el valor de cada atributo ...path="valor" del config
setlocal EnableDelayedExpansion
for /f "usebackq delims=" %%L in ("%~1") do (
  set "L=%%L"
  set "L=!L:"=#!"
  set "V=!L:*path=!"
  if not "!V!"=="!L!" call :valor "!V!"
)
endlocal
exit /b

:valor
for /f "tokens=2 delims=#" %%v in ("%~1") do >>"%TMPL%" echo %%v
exit /b

:ruta
REM Ignora valores que no son carpetas existentes
if "%~1"=="" exit /b
if not exist "%~1\" exit /b
>>"%OUT%" echo.
>>"%OUT%" echo ----- RUTA: %~1
>>"%OUT%" echo   Subcarpetas - hasta 2 niveles:
for /f "delims=" %%D in ('dir /ad /b "%~1" 2^>nul') do (
  >>"%OUT%" echo     %%D
  for /f "delims=" %%E in ('dir /ad /b "%~1\%%D" 2^>nul') do >>"%OUT%" echo       %%D\%%E
)
>>"%OUT%" echo   Ejemplos de piezas .box - maximo 10:
set /a N=0
for /f "delims=" %%F in ('dir /s /b "%~1\*.box" 2^>nul') do call :ejemplo "%%F"
exit /b

:ejemplo
if %N% GEQ 10 exit /b
set /a N+=1
>>"%OUT%" echo     %~1
exit /b
