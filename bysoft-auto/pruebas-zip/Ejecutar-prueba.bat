@echo off
REM ===================================================================
REM  Prueba completa BySoft - importacion automatica de DXF
REM  NO crea carpetas a mano: deja que el Part Importer cree
REM  DESARROLLO\PRUEBA_AUTO2 y registre las piezas en BySoft.
REM  Resultado: resultado.txt en esta carpeta.
REM ===================================================================
setlocal
if not defined PARTS set "PARTS=\\fnsrvnas\Planos\PLANOS_DIBUJO_FANALCA\BLANCOS\LASER\BystronicData\BySoftCam\Parts-FANALCA"
if not defined PI set "PI=C:\Program Files\Bystronic\BySoft CAM\Programmer\PartImporter.exe"
set "REL=DESARROLLO\PRUEBA_AUTO2"
set "AQUI=%~dp0"
if "%AQUI:~-1%"=="\" set "AQUI=%AQUI:~0,-1%"
set "PIS=%AQUI%\config\prueba_auto2.pis"
set "DXF=%AQUI%\dxf"
set "LOG=%AQUI%\log-importacion.txt"
set "OUT=%AQUI%\resultado.txt"

> "%OUT%" echo ==== RESULTADO PRUEBA BYSOFT
>>"%OUT%" date /t
>>"%OUT%" time /t
>>"%OUT%" echo Carpeta de la prueba: %AQUI%
>>"%OUT%" echo Base de piezas: %PARTS%
>>"%OUT%" echo Destino: %REL%

echo.
echo ===== PASO 1 de 5: comprobar archivos y rutas =====
set "FALLO="
if not exist "%PARTS%\" (echo ERROR: no hay acceso a la base de piezas& set "FALLO=1")
if not exist "%PI%" (echo ERROR: no se encuentra PartImporter.exe& set "FALLO=1")
if not exist "%PIS%" (echo ERROR: falta config\prueba_auto2.pis& set "FALLO=1")
set "NDXF=0"
for /f "delims=" %%F in ('dir /b /a-d "%DXF%\*.dxf" 2^>nul') do set /a NDXF+=1
echo DXF de prueba: %NDXF%
>>"%OUT%" echo DXF de prueba: %NDXF%
if not "%NDXF%"=="2" (echo ERROR: deben haber 2 DXF en la carpeta dxf& set "FALLO=1")
if defined FALLO (
  >>"%OUT%" echo PASO 1: FALLO
  goto fin
)
echo OK

echo.
echo ===== PASO 2 de 5: comprobar que las piezas NO existen ya =====
echo Buscando AUTOTEST_* en toda la base, puede tardar 1-2 minutos...
>>"%OUT%" echo.
>>"%OUT%" echo ==== PIEZAS AUTOTEST EXISTENTES ANTES DE IMPORTAR
set "NPREV=0"
for /f "delims=" %%F in ('dir /s /b "%PARTS%\AUTOTEST_*.box" 2^>nul') do (
  set /a NPREV+=1
  >>"%OUT%" echo %%F
)
>>"%OUT%" echo Total: %NPREV%
if not "%NPREV%"=="0" (
  echo ATENCION: ya existen %NPREV% piezas AUTOTEST en la base.
  echo Borralas desde BySoft antes de repetir la prueba. Detalle en resultado.txt
  goto fin
)
echo OK, no existen

echo.
echo ===== PASO 3 de 5: estado de la carpeta destino ANTES =====
if exist "%PARTS%\%REL%\" (
  echo La carpeta %REL% YA existe
  >>"%OUT%" echo Carpeta destino antes: EXISTIA
) else (
  echo La carpeta %REL% NO existe - la debe crear el Part Importer
  >>"%OUT%" echo Carpeta destino antes: NO EXISTIA
)

echo.
echo ===== PASO 4 de 5: importar DXF con el Part Importer =====
echo Cierra el Part Importer si esta abierto y pulsa una tecla.
pause >nul
if exist "%LOG%" del "%LOG%"
echo Importando...
start "" /wait "%PI%" "-s=%PIS%" "-dir=%DXF%" "-log=%LOG%"
set "COD=%ERRORLEVEL%"
call :significado %COD%
echo Codigo: %COD% - %SIG%
>>"%OUT%" echo.
>>"%OUT%" echo ==== IMPORTACION
>>"%OUT%" echo Codigo: %COD% - %SIG%
>>"%OUT%" echo ---- log:
if exist "%LOG%" (type "%LOG%" >>"%OUT%") else (>>"%OUT%" echo No se genero log)

echo.
echo ===== PASO 5 de 5: verificar resultado =====
>>"%OUT%" echo.
>>"%OUT%" echo ==== CARPETA DESTINO DESPUES
if exist "%PARTS%\%REL%\" (
  >>"%OUT%" echo Carpeta destino despues: EXISTE
  dir /b "%PARTS%\%REL%" >>"%OUT%" 2>&1
) else (
  >>"%OUT%" echo Carpeta destino despues: NO EXISTE
)
set "NBOX=0"
for /f "delims=" %%F in ('dir /b /a-d "%PARTS%\%REL%\AUTOTEST_*.box" 2^>nul') do set /a NBOX+=1
>>"%OUT%" echo Piezas AUTOTEST en destino: %NBOX%
echo Piezas AUTOTEST en %REL%: %NBOX%
if "%NBOX%"=="2" (
  echo RESULTADO: OK. Ahora sigue los pasos del LEEME.txt en BySoft.
  >>"%OUT%" echo RESULTADO: OK
) else (
  echo RESULTADO: NO se importaron las 2 piezas. Revisa resultado.txt
  >>"%OUT%" echo RESULTADO: FALLO
)

:fin
echo.
echo Sube este archivo: %OUT%
pause
exit /b

:significado
set "SIG=desconocido"
if "%1"=="0" set "SIG=OK"
if "%1"=="1" set "SIG=sin licencia o parametros incorrectos"
if "%1"=="2" set "SIG=errores al importar, ver log"
if "%1"=="3" set "SIG=no se pudo crear el importador"
if "%1"=="10" set "SIG=no existe el .pis"
if "%1"=="11" set "SIG=no se pudo leer el .pis"
if "%1"=="30" set "SIG=no existe la carpeta de DXF"
if "%1"=="31" set "SIG=no hay DXF en la carpeta"
exit /b
