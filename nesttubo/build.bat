@echo off
rem Compila NestTubo.exe en Windows con MinGW-w64 (g++ y windres en el PATH).
setlocal
cd /d "%~dp0src"
where g++ >nul 2>nul
if errorlevel 1 goto sin_gcc
windres app.rc -O coff -o app.res
if errorlevel 1 goto error
g++ -std=c++17 -municode -O2 main.cpp datos.cpp trabajo.cpp nucleo.cpp salidas.cpp app.res -o ..\NestTubo.exe -mwindows -static -static-libgcc -static-libstdc++ -lcomctl32 -lshlwapi -lole32 -loleaut32 -luuid -lgdi32 -luser32 -lshell32 -ladvapi32 -lcomdlg32
if errorlevel 1 goto error
del app.res
echo Listo: NestTubo.exe quedo junto a este archivo.
pause
exit /b 0
:sin_gcc
echo No se encontro g++ de MinGW-w64 en el PATH.
pause
exit /b 1
:error
echo Fallo la compilacion.
pause
exit /b 1
