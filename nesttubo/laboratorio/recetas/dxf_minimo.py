"""Receta: un DXF ASCII escrito a mano, sin librerias (lo mismo que haria el .exe).

Formato R12 (AC1009): solo la seccion ENTITIES con LINE y TEXT. Es el DXF mas
simple que existe y lo abren los visores comunes. Sin tildes en los textos.

Dibuja UNA barra vista por la cara mas ancha: el rectangulo de la barra, cada
pieza como un cuadrilatero (los extremos inclinados segun el angulo) y la zona
muerta al final. Es solo un dibujo: no es un programa de corte.

Comprobar siempre con ezdxf (solo en el laboratorio):
    python3 dxf_minimo.py barra.dxf && python3 ver_dxf.py barra.dxf barra.png
"""
import math
import sys


def linea(x1, y1, x2, y2, capa):
    return ["0", "LINE", "8", capa, "10", f"{x1:.3f}", "20", f"{y1:.3f}", "11", f"{x2:.3f}", "21", f"{y2:.3f}"]


def texto(x, y, alto, s, capa):
    return ["0", "TEXT", "8", capa, "10", f"{x:.3f}", "20", f"{y:.3f}", "40", f"{alto:.3f}", "1", s]


def barra_dxf(L, cara, despunte, zona_muerta, sep, piezas):
    """piezas: lista de (nombre, largo punta a punta, angulo_izq, angulo_der).

    Convencion usada en esta demo (POR CONFIRMAR con Camilo): el angulo se mide
    respecto al eje del tubo; 90 = corte recto, 45 = inglete. El largo es de
    punta a punta, asi que la inclinacion queda DENTRO del largo de la pieza.
    """
    e = []
    for a, b in (((0, 0), (L, 0)), ((L, 0), (L, cara)), ((L, cara), (0, cara)), ((0, cara), (0, 0))):
        e += linea(*a, *b, "BARRA")
    x = despunte
    for nombre, largo, ang_i, ang_d in piezas:
        di = 0.0 if ang_i == 90 else cara / math.tan(math.radians(ang_i))   # retroceso del borde superior
        dd = 0.0 if ang_d == 90 else cara / math.tan(math.radians(ang_d))
        p = [(x, 0), (x + largo, 0), (x + largo - dd, cara), (x + di, cara)]
        for k in range(4):
            e += linea(*p[k], *p[(k + 1) % 4], "PIEZAS")
        e += texto(x + largo / 2 - 60, cara / 2 - 6, 12, f"{nombre} {largo:g}", "TEXTO")
        x += largo + sep
    zx = L - zona_muerta
    e += linea(zx, 0, zx, cara, "ZONA_MUERTA")
    e += linea(zx, 0, L, cara, "ZONA_MUERTA")
    e += linea(zx, cara, L, 0, "ZONA_MUERTA")
    return "\n".join(["0", "SECTION", "2", "ENTITIES"] + e + ["0", "ENDSEC", "0", "EOF"]) + "\n"


if __name__ == "__main__":
    piezas = [("Larguero", 2880, 90, 90), ("Diagonal", 1250, 45, 45), ("Diagonal", 1250, 45, 90), ("Tope", 360, 90, 90)]
    open(sys.argv[1] if len(sys.argv) > 1 else "barra.dxf", "w", newline="\r\n").write(
        barra_dxf(6000, 100, 10, 230, 3, piezas))
