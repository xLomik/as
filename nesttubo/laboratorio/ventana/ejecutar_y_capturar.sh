#!/bin/bash
# Ejecuta un .exe bajo wine en una pantalla virtual y deja una captura recortada.
# Uso: ./ejecutar_y_capturar.sh prueba.exe captura.png
# Necesita: wine64 xvfb imagemagick (apt) y Pillow (pip). wine NO queda en el PATH.
cd "$(dirname "$0")"
EXE=${1:-prueba.exe}; PNG=${2:-captura.png}
export DISPLAY=:99 WINEPREFIX=${WINEPREFIX:-$HOME/.winep} WINEDEBUG=-all
pidof Xvfb >/dev/null || (setsid nohup Xvfb :99 -screen 0 1400x1000x24 >/dev/null 2>&1 &)
sleep 2
# El prefijo se crea UNA vez y hay que esperar a que termine (tardo 45 s y ocupa unos 700 MB).
# Si se lanza el programa sin esto, la captura muestra el aviso "The Wine configuration is being
# updated", y matar wine a medias deja el prefijo roto ("could not load kernel32.dll"):
# en ese caso, borrar la carpeta del prefijo y repetir.
if [ ! -f "$WINEPREFIX/system.reg" ]; then
  WINEDLLOVERRIDES="mscoree,mshtml=" /usr/lib/wine/wine64 wineboot --init >/dev/null 2>&1
  /usr/lib/wine/wineserver -w
fi
/usr/lib/wine/wineserver -k 2>/dev/null; sleep 3          # instancias viejas contaminan la prueba
(setsid nohup /usr/lib/wine/wine64 "./$EXE" >/dev/null 2>&1 &)
sleep 8
import -window root "$PNG"
/usr/lib/wine/wineserver -k; sleep 3                       # nunca pkill -f: mata el propio shell
python3 - "$PNG" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
caja = im.point(lambda v: 255 if v > 20 else 0).getbbox()   # recorta el fondo negro
if caja:
    im.crop(caja).save(sys.argv[1])
print("captura:", sys.argv[1], "recorte:", caja)
PY
