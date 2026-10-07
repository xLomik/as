"""Heuristicas simples contra el optimo conocido de instancias publicas (BPPLIB)."""
import glob, os, time
from collections import Counter, defaultdict
import openpyxl
from modelo import ffd, bfd, llenado_exacto, cota_l1, cota_l2

B = "../bpplib/Instances"
wb = openpyxl.load_workbook(f"{B}/Solutions/Solutions.xlsx", read_only=True)
optimo = {}
for hoja in ("Falkenauer", "Schoenfield Hard28"):
    for fila in wb[hoja].iter_rows(min_row=2, values_only=True):
        if fila[0] and fila[1] == fila[2]:
            optimo[fila[0]] = fila[2]

def leer(ruta):
    v = [int(x) for x in open(ruta).read().split()]
    n, C, w = v[0], v[1], v[2:]
    assert len(w) == n, ruta
    return w, C

familias = defaultdict(list)
for ruta in sorted(glob.glob(f"{B}/x/**/*.txt", recursive=True)):
    nombre = os.path.basename(ruta)
    if nombre not in optimo:
        continue
    fam = "Hard28" if "Hard28" in nombre else ("Falkenauer U" if "_u" in nombre else "Falkenauer T")
    familias[fam].append((nombre, ruta))

print(f"{'familia':<14}{'inst.':>6}{'piezas':>10} | {'metodo':<9}{'= optimo':>10}{'+1':>6}{'+2 o mas':>10}{'peor':>6}{'seg/inst':>10}")
for fam, lista in familias.items():
    ns = [len(leer(r)[0]) for _, r in lista]
    for nombre_m, metodo in (("FFD", ffd), ("BFD", bfd), ("llenado", llenado_exacto)):
        gaps = Counter(); t0 = time.time()
        for nombre, ruta in lista:
            w, C = leer(ruta)
            gaps[len(metodo(w, C)) - optimo[nombre]] += 1
        dt = (time.time() - t0) / len(lista)
        print(f"{fam:<14}{len(lista):>6}{f'{min(ns)}-{max(ns)}':>10} | {nombre_m:<9}"
              f"{gaps[0]:>10}{gaps[1]:>6}{sum(v for k, v in gaps.items() if k >= 2):>10}{max(gaps):>6}{dt:>10.3f}")
    # que tan buena es la cota frente al optimo real
    c1 = sum(cota_l1(*leer(r)) == optimo[n] for n, r in lista)
    c2 = sum(cota_l2(*leer(r)) == optimo[n] for n, r in lista)
    print(f"{'':<30} | cota L1 = optimo en {c1}/{len(lista)}, cota L2 en {c2}/{len(lista)}")
