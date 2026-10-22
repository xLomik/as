#!/usr/bin/env bash
# Genera imagenes NTFS reales para las pruebas de lince-ntfs / lince-core.
#
# Requiere Linux con root, FUSE y ntfs-3g (mkntfs, ntfs-3g):
#   sudo apt-get install ntfs-3g
#   sudo lince/tools/gen-fixtures.sh
#
# Por cada escenario deja en lince/fixtures/:
#   <nombre>.img.gz        imagen del volumen comprimida
#   <nombre>.expected.tsv  listado esperado: ruta \t tamano \t D|F \t mtime (segundos unix)
set -euo pipefail

OUT="$(cd "$(dirname "$0")/.." && pwd)/fixtures"
WORK="$(mktemp -d)"
trap 'mountpoint -q "$WORK/mnt" && umount "$WORK/mnt"; rm -rf "$WORK"' EXIT
mkdir -p "$OUT" "$WORK/mnt"

relleno() { # relleno <bytes> <texto> : contenido repetitivo (comprime bien)
    # Sin tuberia: con pipefail, el SIGPIPE de `yes` haria fallar el script.
    head -c "$1" < <(yes "$2")
}

escenario_basico() {
    mkdir -p "Docs/Sub" "Fotos 2026" "a/b/c/d/e/f/g/h" "Vacía"
    printf 'hola\n' > Docs/a.txt
    printf 'ñandú\n' > "Docs/Sub/Canción ñ.txt"
    ln Docs/a.txt Docs/Sub/enlace.txt
    : > vacio.dat
    printf 'MAYUSCULAS\n' > MAYUS.TXT
    printf 'zorro\n' > "🦊 zorro.txt"
    printf 'largo\n' > "$(printf 'n%.0s' $(seq 1 200)).txt"
    printf 'profundo\n' > a/b/c/d/e/f/g/h/profundo.txt
    relleno 3000000 "foto grande lince" > "Fotos 2026/grande.bin"
    # Flujo alterno (ADS): no debe cambiar el tamano ni crear entradas.
    printf 'secreto\n' > "Docs/a.txt:oculto"
    # Registros borrados: dejan huecos en el bitmap de la MFT.
    mkdir tmp
    for i in $(seq 1 50); do printf 'x' > "tmp/borrar_$i"; done
    rm -r tmp
}

escenario_muchos() {
    mkdir pre
    for f in $(seq 0 299); do printf 'p%s\n' "$f" > "pre/f$f.txt"; done
    # Llenar casi todo el volumen y abrir huecos: la MFT tendra que crecer
    # en otra zona del disco y quedara fragmentada.
    for i in $(seq 0 40); do
        relleno 1048576 "relleno $i" > "lleno$i.bin" 2>/dev/null || { rm -f "lleno$i.bin"; break; }
    done
    rm -f lleno3.bin lleno9.bin lleno15.bin lleno21.bin
    for d in $(seq 0 14); do
        mkdir "dir_$d"
        for f in $(seq 0 199); do printf '%s-%s\n' "$d" "$f" > "dir_$d/archivo_$f.txt"; done
    done
    # Borrar la mitad de algunas carpetas (huecos en el bitmap).
    for d in 2 5 8; do rm -f "dir_$d"/archivo_1*.txt; done
    # Muchos hard links: los $FILE_NAME no caben en un registro, NTFS crea
    # una lista de atributos y registros de extension.
    mkdir enlaces
    printf 'origen\n' > enlaces/origen.txt
    for i in $(seq 1 300); do ln enlaces/origen.txt "enlaces/enlace_con_nombre_bastante_largo_$i.txt"; done
}

generar() { # generar <nombre> <tamano_MB>
    local nombre="$1" mb="$2" img="$WORK/$1.img"
    truncate -s "${mb}M" "$img"
    mkntfs -F -Q -q -L "LINCE" "$img" >/dev/null 2>&1
    ntfs-3g -o streams_interface=windows "$img" "$WORK/mnt"
    (cd "$WORK/mnt" && "escenario_$nombre")
    (cd "$WORK/mnt" && find . -mindepth 1 -printf '%P\t%s\t%y\t%T@\n') \
        | awk -F'\t' 'BEGIN{OFS="\t"} {t=($3=="d")?"D":"F"; s=(t=="D")?0:$2; split($4,m,"."); print $1,s,t,m[1]}' \
        | LC_ALL=C sort > "$OUT/$nombre.expected.tsv"
    umount "$WORK/mnt"
    gzip -9 -n -c "$img" > "$OUT/$nombre.img.gz"
    echo "$nombre: $(wc -l < "$OUT/$nombre.expected.tsv") entradas, $(du -h "$OUT/$nombre.img.gz" | cut -f1) comprimido"
}

generar basico 32
generar muchos 48
