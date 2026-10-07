"""El metodo propio (sin solver) contra instancias publicas dificiles con optimo conocido.

Necesita BPPLIB en ../bpplib (ver LEEME). Uso:
    python3 medir_propio_bpplib.py <filtro> [max_instancias] [arranques]
filtro: _u120, _u250, _t60, _t120, Hard28 ...
"""
import glob, os, sys, time
from collections import Counter
import openpyxl
from simplex_propio import resolver, validar_pesos

B = "../bpplib/Instances"
wb = openpyxl.load_workbook(f"{B}/Solutions/Solutions.xlsx", read_only=True)
optimo = {}
for hoja in ("Falkenauer", "Schoenfield Hard28"):
    for f in wb[hoja].iter_rows(min_row=2, values_only=True):
        if f[0] and f[1] == f[2]:
            optimo[f[0]] = f[2]
filtro = sys.argv[1]
tope = int(sys.argv[2]) if len(sys.argv) > 2 else 10**9
arranques = int(sys.argv[3]) if len(sys.argv) > 3 else 5
rutas = [r for r in sorted(glob.glob(f"{B}/x/**/*.txt", recursive=True))
         if filtro in os.path.basename(r) and os.path.basename(r) in optimo][:tope]
g = Counter(); cert = 0; t0 = time.time(); tmax = 0.0; cota_floja = 0
for r in rutas:
    v = [int(x) for x in open(r).read().split()]
    C, w = v[1], v[2:]
    ta = time.time()
    sol, cota, info = resolver(w, C, clavar=True, arranques=arranques, semilla=2026)
    tmax = max(tmax, time.time() - ta)
    ok, msg = validar_pesos(w, C, sol); assert ok, (r, msg)
    opt = optimo[os.path.basename(r)]
    assert cota <= opt <= len(sol), (r, cota, opt, len(sol))     # la cota nunca puede pasar del optimo publicado
    g[len(sol) - opt] += 1
    cert += (len(sol) == cota)
    cota_floja += (cota < opt)
    print(os.path.basename(r), "optimo", opt, "cota", cota, "metodo", len(sol), info, flush=True)
n = len(rutas)
linea = (f"[{filtro}] {n} instancias | metodo propio ({arranques} arranques): optimo en {g[0]} ({100*g[0]/n:.0f}%), "
         f"+1 en {g[1]}, +2 o mas en {sum(v for k, v in g.items() if k >= 2)}, peor +{max(g)} | "
         f"certifica el minimo en {cert} | la cota queda 1 por debajo del optimo en {cota_floja} | "
         f"{(time.time()-t0)/n:.1f} s por instancia (max {tmax:.1f})")
print(linea)
open(f"bpp_propio_{filtro.strip('_')}.txt", "w").write(linea + "\n")
