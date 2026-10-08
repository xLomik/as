"""La referencia exacta debe reproducir optimos ya publicados antes de usarla para medir."""
import glob, os, random, time
import openpyxl
from exacto import resolver
from modelo import exacto_pequeno

B = "../bpplib/Instances"
wb = openpyxl.load_workbook(f"{B}/Solutions/Solutions.xlsx", read_only=True)
optimo = {}
for hoja in ("Falkenauer", "Schoenfield Hard28"):
    for f in wb[hoja].iter_rows(min_row=2, values_only=True):
        if f[0] and f[1] == f[2]:
            optimo[f[0]] = f[2]

random.seed(1)
rutas = sorted(glob.glob(f"{B}/x/**/*.txt", recursive=True))
muestra = [r for r in rutas if os.path.basename(r) in optimo]
muestra = ([r for r in muestra if "_u120" in r][:12] + [r for r in muestra if "_u250" in r][:8] +
           [r for r in muestra if "_t60" in r][:10] + [r for r in muestra if "_t120" in r][:6] +
           [r for r in muestra if "Hard28" in r][:10])
mal = 0; iguales = 0; sin_converger = 0; t0 = time.time()
for r in muestra:
    v = [int(x) for x in open(r).read().split()]
    C, w = v[1], v[2:]
    cota, mip, npat, conv = resolver(w, C)
    sin_converger += (not conv)
    opt = optimo[os.path.basename(r)]
    ok = cota <= opt and (mip is None or mip >= opt)
    mal += (not ok); iguales += (mip == opt)
    if not ok: print("INCONSISTENTE", os.path.basename(r), "cota", cota, "mip", mip, "optimo publicado", opt)
print(f"BPPLIB: {len(muestra)} instancias, inconsistencias {mal}; cota LP <= optimo <= solucion entera en todas las demas")
print(f"        sin converger: {sin_converger}")
print(f"        la solucion entera coincide con el optimo publicado en {iguales}/{len(muestra)}  ({time.time()-t0:.0f} s)")

# contra el branch and bound propio en instancias chicas
mal = 0
for _ in range(150):
    C = random.choice([150, 1000, 5763])
    w = [random.randint(C // 15, C * 3 // 5) for _ in range(random.randint(4, 13))]
    opt, probado = exacto_pequeno(w, C)
    cota, mip, _, _ = resolver(w, C)
    if probado and not (cota <= opt <= mip):
        mal += 1; print("INCONSISTENTE", w, C, cota, opt, mip)
print(f"150 instancias chicas contra branch and bound: inconsistencias {mal}")
