#!/usr/bin/env bash
# Genera los documentos de prueba de la búsqueda en contenido
# (lince/fixtures/contenido): Word, Excel, OpenDocument y PDF reales, y
# textos en UTF-8, UTF-16 y ANSI (Windows-1252).
#
# Requiere LibreOffice (soffice), Python 3 y Chromium:
#   sudo apt-get install libreoffice-writer libreoffice-calc python3
#   CHROME=/ruta/a/chrome lince/tools/gen-fixtures-contenido.sh
#
# Cada archivo lleva la frase común «orden de compra 4512», palabras con
# acentos y un marcador propio («lince-<formato>») para probar cada lector
# por separado.
set -euo pipefail

OUT="$(cd "$(dirname "$0")/.." && pwd)/fixtures/contenido"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT"
export HOME="$WORK/home" # perfil temporal de LibreOffice
mkdir -p "$HOME"

documento() { # documento <marcador>: HTML con formato que parte palabras en trozos
    cat <<EOF
<html><head><meta charset="utf-8"><title>Documento</title></head><body>
<h1>Señalización del camión</h1>
<p>Esta es la orden de compra 4512 para FANALCA.</p>
<p>Un contrato <b>marco</b> de 2026 con texto <i>par</i>tido en formatos.</p>
<p>Marcador único: $1</p>
</body></html>
EOF
}

hoja() { # hoja <marcador>
    cat <<EOF
<html><head><meta charset="utf-8"></head><body><table>
<tr><td>Pieza</td><td>Cantidad</td><td>Precio</td></tr>
<tr><td>Lámina de acero inoxidable</td><td>12</td><td>4512.75</td></tr>
<tr><td>Marcador único: $1</td><td>3</td><td>99</td></tr>
</table></body></html>
EOF
}

convertir() { # convertir <html> <filtro|-> <extensión> <salida> [opciones de soffice]
    local destino="$3"
    [ "$2" != "-" ] && destino="$3:$2"
    soffice --headless "${@:5}" --convert-to "$destino" --outdir "$WORK/conv" "$1" >/dev/null
    mv "$WORK/conv/$(basename "${1%.*}").$3" "$OUT/$4"
}

cd "$WORK"
documento lince-docx > docx.html && convertir docx.html "MS Word 2007 XML" docx documento.docx
documento lince-doc > doc.html && convertir doc.html "MS Word 97" doc documento-97.doc
documento lince-odt > odt.html && convertir odt.html - odt documento.odt
documento lince-pdf > pdf.html && convertir pdf.html "writer_web_pdf_Export" pdf documento.pdf
hoja lince-xlsx > xlsx.html && convertir xlsx.html "Calc MS Excel 2007 XML" xlsx hoja.xlsx --calc
hoja lince-xls > xls.html && convertir xls.html "MS Excel 97" xls hoja-97.xls --calc
hoja lince-ods > ods.html && convertir ods.html - ods hoja.ods --calc

# PDF de navegador («imprimir a PDF»), con otra forma de guardar las fuentes.
documento lince-navegador > web.html
"${CHROME:-chromium}" --headless --no-sandbox --disable-gpu --no-pdf-header-footer \
    --print-to-pdf="$OUT/navegador.pdf" web.html 2>/dev/null

python3 - "$OUT" <<'EOF'
import sys
out = sys.argv[1]
texto = "Señalización del camión\r\nEsta es la orden de\r\ncompra 4512.\r\nMarcador único: {}\r\n"
open(f"{out}/texto-utf8.txt", "wb").write(texto.format("lince-utf8").encode("utf-8"))
open(f"{out}/texto-utf16.txt", "wb").write(b"\xff\xfe" + texto.format("lince-utf16").encode("utf-16-le"))
open(f"{out}/texto-ansi.txt", "wb").write(texto.format("lince-ansi").encode("cp1252"))
# Binario sin texto: no debe coincidir con nada.
open(f"{out}/binario.dat", "wb").write(bytes(range(256)) * 16)
EOF
ls -la "$OUT"
