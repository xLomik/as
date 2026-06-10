@echo off
REM Convierte todos los PDF (nombre LP*/LD*) que esten en ESTA carpeta.
REM Coloca este .bat junto a tus PDFs y haz doble clic.
setlocal
cd /d "%~dp0"

if exist "%~dp0PDF-a-Excel.exe" (
    "%~dp0PDF-a-Excel.exe" "%cd%"
) else if exist "%~dp0dist\PDF-a-Excel.exe" (
    "%~dp0dist\PDF-a-Excel.exe" "%cd%"
) else (
    python "%~dp0pdf_to_excel.py" "%cd%"
)
