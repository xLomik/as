@echo off
REM ===================================================================
REM  Pruebas BySoft: carpeta de destino + importacion por linea de comandos
REM  Poner este .bat en la carpeta con los DXF de prueba (ej. prueba_dxf).
REM  Resultado: resultado-pruebas.txt en esa misma carpeta.
REM ===================================================================
setlocal
REM ---- CONFIGURACION -------------------------------------------------
if not defined PARTS set "PARTS=\\fnsrvnas\Planos\PLANOS_DIBUJO_FANALCA\BLANCOS\LASER\BystronicData\BySoftCam\Parts-FANALCA"
if not defined JOBS set "JOBS=\\fnsrvnas\Planos\PLANOS_DIBUJO_FANALCA\BLANCOS\LASER\BystronicData\BySoftCam\PartJobs-FANALCA"
set "REL=DESARROLLO\PRUEBA_AUTO"
if not defined PI set "PI=C:\Program Files\Bystronic\BySoft CAM\Programmer\PartImporter.exe"
REM --------------------------------------------------------------------
set "AQUI=%~dp0"
if "%AQUI:~-1%"=="\" set "AQUI=%AQUI:~0,-1%"
set "OUT=%AQUI%\resultado-pruebas.txt"
set "PIS2=%AQUI%\prueba2.pis"
set "LOG=%AQUI%\log-importacion.txt"

> "%OUT%" echo ==== RESULTADO PRUEBAS BYSOFT
>>"%OUT%" date /t
>>"%OUT%" time /t
>>"%OUT%" echo Carpeta DXF: %AQUI%
>>"%OUT%" echo Base piezas: %PARTS%
>>"%OUT%" echo Destino: %REL%

echo.
echo ================== PASO 1 de 5: comprobar rutas ==================
if not exist "%PARTS%\" (
  echo ERROR: no se puede acceder a la base de piezas:
  echo   %PARTS%
  >>"%OUT%" echo ERROR: base de piezas no accesible
  goto fin
)
if not exist "%PI%" (
  echo ERROR: no se encuentra PartImporter.exe en:
  echo   %PI%
  >>"%OUT%" echo ERROR: PartImporter.exe no encontrado
  goto fin
)
set "NDXF=0"
for /f "delims=" %%F in ('dir /b /a-d "%AQUI%\*.dxf" 2^>nul') do set /a NDXF+=1
echo DXF encontrados en esta carpeta: %NDXF%
>>"%OUT%" echo DXF en la carpeta: %NDXF%
if "%NDXF%"=="0" (
  echo ERROR: pon este .bat en la carpeta que tiene los DXF de prueba.
  goto fin
)
echo OK

echo.
echo ================== PASO 2 de 5: crear carpetas de prueba ==================
call :crear "%PARTS%\%REL%"
call :crear "%JOBS%\%REL%"

echo.
echo ================== PASO 3 de 5: preparar prueba2.pis ==================
if exist "%PIS2%" (
  echo Ya existe prueba2.pis, se usara ese.
) else (
  echo Ahora hazlo en el Part Importer:
  echo   1. Abrir: tu prueba.pis
  echo   2. Pon maquina, material, espesor y tecnologia de corte
  echo   3. Carpeta de destino: %REL%
  echo   4. Conflictos de archivo: Sobrescribir
  echo   5. Guardar como: %PIS2%
  echo   6. CIERRA el Part Importer
  echo.
  echo Cuando termines, pulsa una tecla aqui.
  pause >nul
)
if not exist "%PIS2%" (
  echo ERROR: no encuentro %PIS2%
  >>"%OUT%" echo ERROR: no se creo prueba2.pis
  goto fin
)
>>"%OUT%" echo.
>>"%OUT%" echo ==== CONTENIDO DE prueba2.pis
type "%PIS2%" >>"%OUT%"
>>"%OUT%" echo.
echo OK

echo.
echo ================== PASO 4 de 5: importar DXF por linea de comandos ==================
if exist "%LOG%" del "%LOG%"
echo Importando, espera...
start "" /wait "%PI%" "-s=%PIS2%" "-dir=%AQUI%" "-log=%LOG%"
set "COD=%ERRORLEVEL%"
call :significado %COD%
echo Codigo: %COD% - %SIG%
>>"%OUT%" echo.
>>"%OUT%" echo ==== IMPORTACION
>>"%OUT%" echo Codigo: %COD% - %SIG%
>>"%OUT%" echo ---- log:
if exist "%LOG%" (type "%LOG%" >>"%OUT%") else (>>"%OUT%" echo No se genero log)

echo.
echo ================== PASO 5 de 5: verificar resultado ==================
>>"%OUT%" echo.
>>"%OUT%" echo ==== CONTENIDO DE %REL% EN LA BASE DE PIEZAS
dir /b "%PARTS%\%REL%" >>"%OUT%" 2>&1
set "NBOX=0"
for /f "delims=" %%F in ('dir /b /a-d "%PARTS%\%REL%\*.box" 2^>nul') do set /a NBOX+=1
echo Piezas .box en %REL%: %NBOX%
>>"%OUT%" echo Piezas .box en destino: %NBOX%
>>"%OUT%" echo.
>>"%OUT%" echo ==== PIEZAS SUELTAS EN LA RAIZ DE LA BASE - prueba anterior
dir /b "%PARTS%\ESQUINERO_*" >>"%OUT%" 2>&1
if "%NBOX%"=="0" (
  echo RESULTADO: las piezas NO llegaron a %REL%. Revisa resultado-pruebas.txt
) else (
  echo RESULTADO: OK, las piezas se guardaron en %REL%
)

echo.
echo ================== LIMPIEZA OPCIONAL ==================
echo Borrar los archivos de diagnostico que quedaron en el servidor?
echo   Registros-BySoft*.bat, registros-bysoft.txt, diagnostico-bysoft.ps1, Ejecutar-diagnostico.bat
echo   - solo en la raiz de Parts-FANALCA y PartJobs-FANALCA -
choice /c SN /m "S = borrar, N = no tocar nada"
if errorlevel 2 goto fin
>>"%OUT%" echo.
>>"%OUT%" echo ==== LIMPIEZA
for %%R in ("%PARTS%" "%JOBS%") do (
  for %%A in ("Registros-BySoft.bat" "Registros-BySoft (1).bat" "Registros-BySoft (2).bat" "registros-bysoft.txt" "diagnostico-bysoft.ps1" "Ejecutar-diagnostico.bat" "diagnostico-bysoft-cmd.txt") do (
    if exist "%%~R\%%~A" (
      del "%%~R\%%~A" && >>"%OUT%" echo Borrado: %%~R\%%~A
    )
  )
)
echo Limpieza hecha.

:fin
echo.
echo Listo. Sube este archivo: %OUT%
pause
exit /b

:crear
if exist "%~1\" (
  echo Ya existe: %~1
  >>"%OUT%" echo Carpeta ya existia: %~1
) else (
  mkdir "%~1"
)
if exist "%~1\" (
  echo Lista: %~1
  >>"%OUT%" echo Carpeta lista: %~1
) else (
  echo ERROR creando: %~1
  >>"%OUT%" echo ERROR creando: %~1
)
exit /b

:significado
set "SIG=desconocido"
if "%1"=="0" set "SIG=OK, importacion correcta"
if "%1"=="1" set "SIG=sin licencia o parametros incorrectos"
if "%1"=="2" set "SIG=hubo errores al importar, ver log"
if "%1"=="3" set "SIG=no se pudo crear el importador"
if "%1"=="10" set "SIG=no existe el archivo .pis"
if "%1"=="11" set "SIG=no se pudo leer el archivo .pis"
if "%1"=="30" set "SIG=la carpeta de DXF no existe"
if "%1"=="31" set "SIG=no hay archivos importables en la carpeta"
exit /b
