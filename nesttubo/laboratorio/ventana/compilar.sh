#!/bin/bash
# Compila las tres pruebas de la ruta de entrega. Verificado el 2026-10-06 en Ubuntu 24.04
# con g++-mingw-w64-x86-64 (GCC 13) instalado por apt.
set -e
cd "$(dirname "$0")"
LIBS="-lcomctl32 -lshlwapi -lole32 -loleaut32 -luuid -lgdi32 -luser32 -lshell32 -ladvapi32"
x86_64-w64-mingw32-windres app.rc -O coff -o app.res
x86_64-w64-mingw32-g++ -std=c++17 -municode -O2 main.cpp app.res -o prueba.exe -mwindows -static -static-libgcc -static-libstdc++ $LIBS
x86_64-w64-mingw32-g++ -std=c++17 -municode -O2 acentos.cpp app.res -o acentos.exe -mwindows -static -static-libgcc -static-libstdc++ $LIBS
x86_64-w64-mingw32-g++ -std=c++17 -O2 hilo.cpp -o hilo.exe -static -static-libgcc -static-libstdc++
x86_64-w64-mingw32-strip prueba.exe acentos.exe hilo.exe
for e in prueba.exe acentos.exe hilo.exe; do
  echo "$e: $(stat -c %s $e) bytes; DLL: $(x86_64-w64-mingw32-objdump -p $e | grep 'DLL Name' | awk '{print $3}' | sort -u | tr '\n' ' ')"
done
