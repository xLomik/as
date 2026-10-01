@echo off
REM ===================================================================
REM  Prueba 3 BySoft: carpetas creadas con la API de BySoft
REM  1. Prepara BySoftCarpeta.exe (lo compila con el csc de Windows).
REM  2. Crea DESARROLLO\PRUEBA_AUTO3 con FolderInfo.CreateSubfolder
REM     (igual que el boton "Crear nueva carpeta") en Parts y PartJobs,
REM     y registra en el indice PRUEBA_AUTO2 (creada antes con mkdir).
REM  3. Importa los DXF con el Part Importer en PRUEBA_AUTO3.
REM  Resultado: resultado.txt en esta carpeta.
REM ===================================================================
setlocal
if not defined PARTS set "PARTS=\\fnsrvnas\Planos\PLANOS_DIBUJO_FANALCA\BLANCOS\LASER\BystronicData\BySoftCam\Parts-FANALCA"
if not defined BYSOFT_DIR set "BYSOFT_DIR=C:\Program Files\Bystronic\BySoft CAM\Programmer"
if not defined PI set "PI=%BYSOFT_DIR%\PartImporter.exe"
set "REL=DESARROLLO\PRUEBA_AUTO3"
set "AQUI=%~dp0"
if "%AQUI:~-1%"=="\" set "AQUI=%AQUI:~0,-1%"
set "HERR=%AQUI%\herramienta"
set "TOOL=%HERR%\BySoftCarpeta.exe"
set "PIS=%AQUI%\config\prueba_auto3.pis"
set "DXF=%AQUI%\dxf"
set "LOG=%AQUI%\log-importacion.txt"
set "OUT=%AQUI%\resultado.txt"

> "%OUT%" echo ==== RESULTADO PRUEBA 3 BYSOFT
>>"%OUT%" date /t
>>"%OUT%" time /t
>>"%OUT%" echo Base de piezas esperada: %PARTS%
>>"%OUT%" echo Destino: %REL%

echo.
echo ===== PASO 1 de 6: comprobar archivos y rutas =====
set "FALLO="
if not exist "%PARTS%\" (echo ERROR: no hay acceso a la base de piezas& set "FALLO=1")
if not exist "%PI%" (echo ERROR: no se encuentra PartImporter.exe& set "FALLO=1")
if not exist "%PIS%" (echo ERROR: falta config\prueba_auto3.pis& set "FALLO=1")
set "NDXF=0"
for /f "delims=" %%F in ('dir /b /a-d "%DXF%\*.dxf" 2^>nul') do set /a NDXF+=1
if not "%NDXF%"=="2" (echo ERROR: deben haber 2 DXF en la carpeta dxf& set "FALLO=1")
if defined FALLO (
  >>"%OUT%" echo PASO 1: FALLO
  goto fin
)
echo OK

echo.
echo ===== PASO 2 de 6: preparar BySoftCarpeta.exe =====
set "CSC=%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe"
>>"%OUT%" echo.
>>"%OUT%" echo ==== HERRAMIENTA
if exist "%CSC%" (
  echo Compilando con el csc de Windows...
  "%CSC%" /nologo /platform:x64 /out:"%HERR%\BySoftCarpeta_local.exe" /r:System.Configuration.dll /r:"%BYSOFT_DIR%\Bystronic.BySoft.Common.Persistence.dll" "%HERR%\BySoftCarpeta.cs" >>"%OUT%" 2>&1
  if exist "%HERR%\BySoftCarpeta_local.exe" (
    set "TOOL=%HERR%\BySoftCarpeta_local.exe"
    >>"%OUT%" echo Compilada en este PC: BySoftCarpeta_local.exe
  ) else (
    >>"%OUT%" echo No se pudo compilar, se usa el exe incluido
  )
) else (
  >>"%OUT%" echo No hay csc.exe, se usa el exe incluido
)
copy /y "%BYSOFT_DIR%\PartImporter.exe.config" "%TOOL%.config" >nul
if errorlevel 1 (
  echo ERROR: no se pudo copiar la configuracion de BySoft
  >>"%OUT%" echo ERROR: no se pudo copiar PartImporter.exe.config
  goto fin
)
echo Herramienta: %TOOL%
>>"%OUT%" echo Herramienta: %TOOL%
>>"%OUT%" echo ---- info:
"%TOOL%" info >>"%OUT%" 2>&1
set "COD=%ERRORLEVEL%"
set "RAIZ="
for /f "tokens=1,* delims==" %%A in ('call "%TOOL%" info 2^>nul') do if /i "%%A"=="Parts" set "RAIZ=%%B"
if not "%COD%"=="0" (
  echo ERROR: la herramienta no pudo leer la configuracion de BySoft. Ver resultado.txt
  goto fin
)
echo Base de piezas segun BySoft: %RAIZ%
if /i not "%RAIZ%"=="%PARTS%" (
  echo ERROR: la base de BySoft no coincide con la esperada. No se toca nada.
  >>"%OUT%" echo ERROR: base distinta a la esperada
  goto fin
)
echo OK

echo.
echo ===== PASO 3 de 6: comprobar que AUTOTEST_C y D no existen =====
echo Buscando en toda la base, puede tardar 1-2 minutos...
>>"%OUT%" echo.
>>"%OUT%" echo ==== PIEZAS AUTOTEST_C/D EXISTENTES ANTES
set "NPREV=0"
for /f "delims=" %%F in ('dir /s /b "%PARTS%\AUTOTEST_C_R0.box" "%PARTS%\AUTOTEST_D_R0.box" 2^>nul') do (
  set /a NPREV+=1
  >>"%OUT%" echo %%F
)
>>"%OUT%" echo Total: %NPREV%
if not "%NPREV%"=="0" (
  echo ATENCION: ya existen. Borralas desde BySoft antes de repetir.
  goto fin
)
echo OK

echo.
echo ===== PASO 4 de 6: crear carpetas con la API de BySoft =====
>>"%OUT%" echo.
>>"%OUT%" echo ==== CREAR CARPETAS
call :carpeta Parts "%REL%"
if errorlevel 1 goto fin
call :carpeta PartJobs "%REL%"
if errorlevel 1 goto fin
echo Registrar en el indice PRUEBA_AUTO2, creada antes con mkdir:
call :carpeta Parts "DESARROLLO\PRUEBA_AUTO2"
if errorlevel 1 goto fin

echo.
echo ===== PASO 5 de 6: importar DXF con el Part Importer =====
echo Cierra el Part Importer si esta abierto y pulsa una tecla.
pause >nul
if exist "%LOG%" del "%LOG%"
echo Importando...
start "" /wait "%PI%" "-s=%PIS%" "-dir=%DXF%" "-log=%LOG%"
set "COD=%ERRORLEVEL%"
echo Codigo: %COD%
>>"%OUT%" echo.
>>"%OUT%" echo ==== IMPORTACION
>>"%OUT%" echo Codigo: %COD%
if exist "%LOG%" (type "%LOG%" >>"%OUT%") else (>>"%OUT%" echo No se genero log)

echo.
echo ===== PASO 6 de 6: verificar =====
>>"%OUT%" echo.
>>"%OUT%" echo ==== CARPETA DESTINO DESPUES
dir /b "%PARTS%\%REL%" >>"%OUT%" 2>&1
set "NBOX=0"
for /f "delims=" %%F in ('dir /b /a-d "%PARTS%\%REL%\AUTOTEST_*.box" 2^>nul') do set /a NBOX+=1
>>"%OUT%" echo Piezas en destino: %NBOX%
echo Piezas en %REL%: %NBOX%
if "%NBOX%"=="2" (
  echo RESULTADO: OK. Sigue los pasos del LEEME.txt en BySoft.
  >>"%OUT%" echo RESULTADO: OK
) else (
  echo RESULTADO: FALLO. Revisa resultado.txt
  >>"%OUT%" echo RESULTADO: FALLO
)

:fin
echo.
echo Sube este archivo: %OUT%
pause
exit /b

:carpeta
REM %1 = Parts o PartJobs, %2 = ruta relativa
echo   %~1: %~2
>>"%OUT%" echo ---- %~1 %~2
"%TOOL%" crear %~1 "%~2" >>"%OUT%" 2>&1
if errorlevel 1 (
  echo   ERROR creando la carpeta. Ver resultado.txt
  exit /b 1
)
echo   OK
exit /b 0
