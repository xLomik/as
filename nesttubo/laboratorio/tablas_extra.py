"""Arma las tablas del arbitro exacto y de BPPLIB para el LEEME (uso interno del laboratorio)."""
import glob, json, os, re, sys
from collections import Counter

carpeta = sys.argv[1] if len(sys.argv) > 1 else "."
print("| Familia | Trabajo | Largos × cantidad | Cota | Método | Exacto | Veredicto |")
print("|---|---|---|---|---|---|---|")
n = Counter()
for f in sorted(glob.glob(os.path.join(carpeta, "arbitro_*.json"))):
    for r in json.load(open(f)):
        c = Counter(r["largos"])
        if len(c) <= 10:
            lista = ", ".join(f"{l}×{k}" for l, k in sorted(c.items(), reverse=True))
        else:
            lista = f"{len(c)} largos distintos, {r['piezas']} piezas"
        v = {"la barra de mas era evitable": "sobraba 1 barra", "cota+1 era el minimo": "era el mínimo: la cota era floja",
             "sin decidir": "sin decidir en el tiempo dado"}[r["veredicto"]]
        n[v] += 1
        print(f"| {r['familia']} | {r['trabajo']} | {lista} | {r['cota']} | {r['metodo_propio']} | {r['arcflow']} | {v} |")
print("\nveredictos:", dict(n), "\n")

print("| Instancias | Cuántas | Método: = óptimo | +1 | +2 o más | Mínimo demostrado | s por instancia (máx.) |")
print("|---|---|---|---|---|---|---|")
for f in sorted(glob.glob(os.path.join(carpeta, "bpp_propio_*.txt"))):
    t = open(f).read()
    m = re.search(r"\[(.+?)\] (\d+) instancias \| metodo propio \((\d+) arranques\): optimo en (\d+) \((\d+)%\), \+1 en (\d+), "
                  r"\+2 o mas en (\d+), peor \+(\d+) \| certifica el minimo en (\d+) \| la cota queda 1 por debajo del optimo en (\d+) \| "
                  r"([\d.]+) s por instancia \(max ([\d.]+)\)", t)
    if not m:
        print("NO SE PUDO LEER", f); continue
    g = m.groups()
    print(f"| {g[0].strip('_')} | {g[1]} | {g[3]} | {g[5]} | {g[6]} | {g[8]} | {g[10].replace('.', ',')} ({g[11].replace('.', ',')}) | "
          f"<!-- arranques {g[2]}, cota floja en {g[9]} -->")
