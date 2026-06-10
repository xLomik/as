@echo off
REM ============================================================
REM  Construye el ejecutable portable PDF-a-Excel.exe (Windows)
REM  Requiere Python 3.9+ instalado (solo para compilar; el .exe
REM  resultante NO necesita Python).
REM ============================================================
setlocal
cd /d "%~dp0"

echo [1/3] Instalando dependencias...
python -m pip install --upgrade pip >nul
python -m pip install -r requirements.txt pyinstaller || goto :error

echo [2/3] Compilando con PyInstaller...
python -m PyInstaller --noconfirm --clean pdf2excel.spec || goto :error

echo [3/3] Listo.
echo.
echo   Ejecutable: "%~dp0dist\PDF-a-Excel.exe"
echo.
echo Ese .exe es portable: copialo a cualquier PC Windows (sin instalar nada).
pause
exit /b 0

:error
echo.
echo *** Ocurrio un error en la compilacion. ***
pause
exit /b 1
