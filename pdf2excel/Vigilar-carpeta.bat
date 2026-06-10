@echo off
REM Modo AUTOMATICO: vigila ESTA carpeta y convierte cada PDF LP*/LD*
REM en cuanto aparece o cambia. Deja esta ventana abierta. Ctrl+C para salir.
setlocal
cd /d "%~dp0"

if exist "%~dp0PDF-a-Excel.exe" (
    "%~dp0PDF-a-Excel.exe" --watch "%cd%"
) else if exist "%~dp0dist\PDF-a-Excel.exe" (
    "%~dp0dist\PDF-a-Excel.exe" --watch "%cd%"
) else (
    python "%~dp0pdf_to_excel.py" --watch "%cd%"
)
pause
