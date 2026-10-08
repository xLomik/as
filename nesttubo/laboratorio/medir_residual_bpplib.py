"""El metodo residual contra las instancias publicas mas dificiles, con optimo conocido."""
import glob, os, sys, time
from collections import Counter
import openpyxl
from residual import residual, validar_pesos
from modelo import ffd, bfd, llenado_exacto

B = "../bpplib/Instances"
wb = openpyxl.load_workbook(f"{B}/Solutions/Solutions.xlsx", read_only=True)
optimo = {}
for hoja in ("Falkenauer", "Schoenfield Hard28"):
    for f in wb[hoja].iter_rows(min_row=2, values_only=True):
        if f[0] and f[1] == f[2]:
            optimo[f[0]] = f[2]
filtro = sys.argv[1]
rutas = [r for r in sorted(glob.glob(f"{B}/x/**/*.txt", recursive=True))
         if filtro in os.path.basename(r) and os.path.basename(r) in optimo]
g = Counter(); g3 = Counter(); t0 = time.time(); cert = 0
for r in rutas:
    v = [int(x) for x in open(r).read().split()]
    C, w = v[1], v[2:]
    sol, cota = residual(w, C)
    ok, msg = validar_pesos(w, C, sol); assert ok, (r, msg)
    opt = optimo[os.path.basename(r)]
    assert cota <= opt <= len(sol), (r, cota, opt, len(sol))
    g[len(sol) - opt] += 1
    cert += (len(sol) == cota)
    g3[min(len(ffd(w, C)), len(bfd(w, C)), len(llenado_exacto(w, C))) - opt] += 1
n = len(rutas)
print(f"[{filtro}] {n} instancias | residual: optimo en {g[0]} ({100*g[0]/n:.0f}%), +1 en {g[1]}, +2 o mas en {sum(v for k,v in g.items() if k>=2)}, peor +{max(g)}"
      f" | certifica el minimo en {cert} | mejor de 3 simples: optimo en {g3[0]}, peor +{max(g3)} | {(time.time()-t0)/n:.1f} s por instancia")
