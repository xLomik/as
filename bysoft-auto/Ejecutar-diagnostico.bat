@echo off
REM Ejecuta el diagnostico (solo lectura) sin cambiar la politica de ejecucion del sistema.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0diagnostico-bysoft.ps1"
pause
