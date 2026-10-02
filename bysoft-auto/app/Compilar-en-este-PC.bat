@echo off
REM Compila AutoBySoft.exe en este PC con el csc de .NET Framework (C# 5).
setlocal
set "BYSOFT_DIR=C:\Program Files\Bystronic\BySoft CAM\Programmer"
set "CSC=%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe"
set "AQUI=%~dp0"
if not exist "%CSC%" (
  echo ERROR: no se encuentra %CSC%
  pause
  exit /b 1
)
"%CSC%" /nologo /target:winexe /platform:x64 /optimize+ /out:"%AQUI%AutoBySoft.exe" ^
  /r:System.Configuration.dll /r:System.Xml.Linq.dll /r:System.IO.Compression.dll /r:System.IO.Compression.FileSystem.dll ^
  /r:System.Windows.Forms.dll /r:System.Drawing.dll ^
  /r:"%BYSOFT_DIR%\Bystronic.BySoft.Common.Persistence.dll" /r:"%BYSOFT_DIR%\Bystronic.BySoft.Common.dll" ^
  "%AQUI%src\*.cs"
if errorlevel 1 (
  echo.
  echo ERROR de compilacion. Haz una captura de esta ventana.
) else (
  echo.
  echo OK: AutoBySoft.exe compilado.
)
pause
