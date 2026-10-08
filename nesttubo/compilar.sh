#!/bin/bash
# Compila y prueba NestTubo en Linux (Ubuntu 24.04).
#   ./compilar.sh pruebas   nucleo nativo con sanitizadores + pruebas (deben terminar en "FALLOS: 0")
#   ./compilar.sh exe       NestTubo.exe para Windows con mingw-w64, en build/
#   ./compilar.sh datos     carpeta de datos bajo wine con una unidad de red N: de prueba
#   ./compilar.sh           pruebas y exe
# Necesita: apt-get install -y --no-install-recommends g++ g++-mingw-w64-x86-64
set -e
cd "$(dirname "$0")"
mkdir -p build
LIBS="-lcomctl32 -lshlwapi -lole32 -loleaut32 -luuid -lgdi32 -luser32 -lshell32 -ladvapi32 -lcomdlg32"
que=${1:-todo}
if [ "$que" = pruebas ] || [ "$que" = todo ]; then
  g++ -std=c++17 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -Wall -Wextra \
    src/nucleo.cpp src/trabajo.cpp src/salidas.cpp src/pruebas.cpp -o build/pruebas
  build/pruebas laboratorio/casos_oraculo.txt
fi
if [ "$que" = exe ] || [ "$que" = todo ]; then
  x86_64-w64-mingw32-windres src/app.rc -O coff -o build/app.res
  x86_64-w64-mingw32-g++ -std=c++17 -municode -O2 -Wall -Wextra src/main.cpp src/datos.cpp src/trabajo.cpp src/nucleo.cpp src/salidas.cpp build/app.res \
    -o build/NestTubo.exe -mwindows -static -static-libgcc -static-libstdc++ $LIBS
  x86_64-w64-mingw32-strip build/NestTubo.exe
  echo "build/NestTubo.exe: $(stat -c %s build/NestTubo.exe) bytes; DLL: $(x86_64-w64-mingw32-objdump -p build/NestTubo.exe | grep 'DLL Name' | awk '{print $3}' | tr '\n' ' ')"
fi
if [ "$que" = datos ]; then
  # N: es una unidad "de red" de wine que apunta a build/srv; las pruebas la desconectan moviendo esa carpeta
  x86_64-w64-mingw32-g++ -std=c++17 -municode -O1 -Wall -Wextra src/pruebas_datos.cpp src/datos.cpp src/trabajo.cpp src/nucleo.cpp \
    -o build/pruebas_datos.exe -static -static-libgcc -static-libstdc++ $LIBS
  # sin locale UTF-8, wine no crea archivos con tildes ("Sin conexión") y responde "no encontrado"
  # NESTTUBO_INTERVALO_S: sin pasadas solas cada 30 s, que se cruzarian con las que piden las pruebas
  export WINEPREFIX=${WINEPREFIX:-$HOME/.winep} WINEDEBUG=-all LANG=C.UTF-8 NESTTUBO_INTERVALO_S=3600
  srv="$PWD/build/srv"
  mkdir -p "$srv"
  ln -sfn "$srv" "$WINEPREFIX/dosdevices/n:"
  /usr/lib/wine/wine64 reg add 'HKLM\Software\Wine\Drives' /v n: /d network /f >/dev/null
  /usr/lib/wine/wine64 build/pruebas_datos.exe "Z:$(echo "$srv" | tr / '\\')"
fi
