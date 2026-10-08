"""Receta: un PDF escrito a mano, sin librerias (lo mismo que haria el .exe).

Solo usa: objetos numerados, dos fuentes estandar (Helvetica y Helvetica-Bold,
no se incrustan), codificacion WinAnsi para las tildes, y operadores de dibujo
basicos (re, S, B, f, g, RG, w, BT/Tf/Td/Tj/ET).

Comprobar siempre con:
    pdftoppm -r 70 -png salida.pdf pagina     y mirar el PNG
    pdftotext -layout salida.pdf -            (texto y tildes)
"""
import sys


def texto_pdf(s):
    """Cadena literal PDF en WinAnsi (cp1252): escapa \\ ( ) y lo no ASCII en octal."""
    out = []
    for b in s.encode("cp1252"):
        if b in (0x28, 0x29, 0x5C):
            out.append("\\" + chr(b))
        elif b < 32 or b > 126:
            out.append("\\%03o" % b)
        else:
            out.append(chr(b))
    return "(" + "".join(out) + ")"


def pagina_demo():
    c = []
    c.append("BT /F2 16 Tf 40 800 Td %s Tj ET" % texto_pdf("Plan de corte de tubería — pedido de ejemplo"))
    c.append("BT /F1 10 Tf 40 782 Td %s Tj ET"
             % texto_pdf("Perfil: Cuadrado 40x40x2    Barra: 6000 mm    Útil: 5760 mm    Separación: 3 mm"))
    # una barra: 515 pt de ancho representan 6000 mm
    x0, y0, ancho, alto, L = 40.0, 730.0, 515.0, 18.0, 6000.0
    esc = ancho / L
    c.append("0.6 w 0 0 0 RG %.2f %.2f %.2f %.2f re S" % (x0, y0, ancho, alto))
    pos = 10.0
    for i, largo in enumerate([2880, 1250, 1250, 360]):
        gris = 0.82 if i % 2 == 0 else 0.92
        c.append("%.2f g %.2f %.2f %.2f %.2f re B" % (gris, x0 + pos * esc, y0, largo * esc, alto))
        c.append("0 g BT /F1 8 Tf %.2f %.2f Td %s Tj ET" % (x0 + pos * esc + 3, y0 + 6, texto_pdf(str(largo))))
        pos += largo + 3
    # zona muerta en gris oscuro
    c.append("0.55 g %.2f %.2f %.2f %.2f re f 0 g" % (x0 + (L - 230) * esc, y0, 230 * esc, alto))
    c.append("BT /F1 9 Tf 40 712 Td %s Tj ET"
             % texto_pdf("Barra 1 de 3 · 4 piezas · sobrante 11 mm · zona muerta 230 mm"))
    return "\n".join(c).encode("ascii")


def escribir(ruta, contenidos):
    objs = []                                   # cuerpos de los objetos, en orden 1..n
    n_pag = len(contenidos)
    kids = " ".join("%d 0 R" % (5 + 2 * i) for i in range(n_pag))
    objs.append(b"<< /Type /Catalog /Pages 2 0 R >>")
    objs.append(("<< /Type /Pages /Kids [%s] /Count %d >>" % (kids, n_pag)).encode())
    objs.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    objs.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>")
    for i, flujo in enumerate(contenidos):
        objs.append(("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] "
                     "/Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents %d 0 R >>" % (6 + 2 * i)).encode())
        objs.append(b"<< /Length %d >>\nstream\n" % len(flujo) + flujo + b"\nendstream")
    out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    pos = []
    for k, cuerpo in enumerate(objs, 1):
        pos.append(len(out))                    # la tabla xref guarda el byte donde empieza cada objeto
        out += b"%d 0 obj\n" % k + cuerpo + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n" % (len(objs) + 1) + b"0000000000 65535 f \n"
    for p in pos:
        out += b"%010d 00000 n \n" % p          # cada linea mide exactamente 20 bytes
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, xref)
    open(ruta, "wb").write(out)


if __name__ == "__main__":
    escribir(sys.argv[1] if len(sys.argv) > 1 else "demo.pdf", [pagina_demo(), pagina_demo()])
