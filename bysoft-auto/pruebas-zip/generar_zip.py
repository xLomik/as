# Genera PRUEBAS_BYSOFT.zip con todo lo necesario para la prueba completa:
# DXF de prueba (formato BySoft), .pis listo, Excel sin encabezado, BySoftCarpeta
# (crea carpetas con la API de BySoft), .bat y LEEME.
# Uso: python generar_zip.py
import re, zipfile
from pathlib import Path
from openpyxl import Workbook

HERE = Path(__file__).parent
SAMPLE = HERE / "plantilla_bysoft.dxf"      # DXF exportado por BySoft (hcmf75423_r0)
OUT = HERE / "PRUEBAS_BYSOFT.zip"
CARPETA = "DESARROLLO/PRUEBA_AUTO3"          # la crea BySoftCarpeta con la API de BySoft

# nombre -> (ancho, alto, [(cx, cy, diametro), ...], cantidad)
PIEZAS = {
    "AUTOTEST_C_R0": (120.0, 60.0, [(30.0, 30.0, 20.0)], "2"),
    "AUTOTEST_D_R0": (90.0, 45.0, [(20.0, 22.5, 10.0), (70.0, 22.5, 10.0)], "3"),
}

def lwpoly(handle, pts):
    """pts: lista de (x, y, bulge). Polilinea cerrada en capa GEOMETRY."""
    s = ["0", "LWPOLYLINE", "5", handle, "100", "AcDbEntity", "8", "GEOMETRY",
         "6", "CONTINUOUS", "100", "AcDbPolyline", "90", str(len(pts)), "70", "1"]
    for x, y, b in pts:
        s += ["10", f"{x:g}", "20", f"{y:g}"]
        if b:
            s += ["42", f"{b:g}"]
    return s

def dxf(w, h, agujeros):
    t = SAMPLE.read_text(encoding="cp1252").replace("\r\n", "\n")
    ents, hnd = [], 0x100
    for cx, cy, d in agujeros:               # circulo = 2 vertices con bulge 1 (como BySoft)
        r = d / 2
        ents += lwpoly(f"{hnd:X}", [(cx + r, cy, 1), (cx - r, cy, 1)]); hnd += 1
    ents += lwpoly(f"{hnd:X}", [(0, 0, 0), (0, h, 0), (w, h, 0), (w, 0, 0)])
    body = "\n".join(ents) + "\n"
    t = re.sub(r"(\n2\nENTITIES\n).*?(\n?0\nENDSEC\n)", lambda m: m.group(1) + body + "0\nENDSEC\n", t, count=1, flags=re.S)
    # Vista inicial centrada en la pieza
    t = re.sub(r"\n12\n[^\n]*\n22\n[^\n]*\n40\n[^\n]*\n41\n[^\n]*\n",
               f"\n12\n{w/2:g}\n22\n{h/2:g}\n40\n{max(w,h):g}\n41\n{w/h:.11g}\n", t, count=1)
    return t.replace("\n", "\r\n")

def excel(path):
    wb = Workbook(); ws = wb.active; ws.title = "Piezas"
    # SIN encabezado: BySoft procesa todas las filas y crearia una pieza vacia con el titulo.
    for nombre, (_, _, _, cant) in PIEZAS.items():
        ws.append([nombre, cant, "PRUEBA_AUTO3", "", "", ""])
    for row in ws.iter_rows():
        for c in row:
            c.number_format = "@"
            if c.value is not None:
                c.data_type = "s"          # todo texto: BySoft hace (string)Value2
    ws.column_dimensions["A"].width = 25
    wb.save(path)

def main():
    tmp = HERE / "_build"; (tmp / "dxf").mkdir(parents=True, exist_ok=True)
    for n, (w, h, ag, _) in PIEZAS.items():
        (tmp / "dxf" / f"{n}.dxf").write_bytes(dxf(w, h, ag).encode("cp1252"))
    excel(tmp / "importar_en_part_nester.xlsx")
    pis = (HERE / "prueba_auto3.pis").read_text(encoding="utf-8")
    files = {
        "PRUEBAS_BYSOFT/1-Ejecutar-prueba.bat": (HERE / "Ejecutar-prueba.bat").read_bytes(),
        "PRUEBAS_BYSOFT/LEEME.txt": (HERE / "LEEME.txt").read_bytes(),
        "PRUEBAS_BYSOFT/config/prueba_auto3.pis": pis.encode("utf-8"),
        "PRUEBAS_BYSOFT/herramienta/BySoftCarpeta.cs": (HERE / "BySoftCarpeta.cs").read_bytes(),
        "PRUEBAS_BYSOFT/herramienta/BySoftCarpeta.exe": (HERE / "BySoftCarpeta.exe").read_bytes(),
        "PRUEBAS_BYSOFT/2-importar_en_part_nester.xlsx": (tmp / "importar_en_part_nester.xlsx").read_bytes(),
    }
    for n in PIEZAS:
        files[f"PRUEBAS_BYSOFT/dxf/{n}.dxf"] = (tmp / "dxf" / f"{n}.dxf").read_bytes()
    with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in files.items():
            z.writestr(name, data)
    print("OK", OUT, len(files), "archivos")

if __name__ == "__main__":
    main()
