"""Arma las tablas del LEEME a partir de los propio_*.json (para no copiar numeros a mano).

Uso:  python3 resumen.py [carpeta_con_los_json]
"""
import glob, json, os, sys
from collections import Counter

carpeta = sys.argv[1] if len(sys.argv) > 1 else "."
ORDEN = ["chico", "largas", "medio", "redondos", "barra12", "sin_margen", "decimal", "grande", "variado"]
NOTA = {"largas": "largas (1500–3500 mm)", "redondos": "redondos (múltiplos de 5 mm)", "barra12": "barra12 (barra de 12 m)",
        "sin_margen": "sin_margen (sin despunte, zona muerta ni separación)", "decimal": "decimal (décimas de mm)",
        "variado": "variado (40–120 largos distintos, 1–3 de cada uno)"}


def miles(n):
    return f"{n:,}".replace(",", ".")


def reparto(c):
    c = {int(k): v for k, v in c.items()}
    peor = max(c)
    mas = sum(v for k, v in c.items() if k >= 2)
    return f"{miles(c.get(0, 0))} / {miles(c.get(1, 0))} / {miles(mas)}" + (f" (+{peor})" if peor >= 2 else "")


datos = {}
for f in glob.glob(os.path.join(carpeta, "propio_*.json")):
    r = json.load(open(f))
    datos[r["familia"]] = r
tot = Counter(); br = {v: Counter() for v in ("simples", "residual", "final", "peor_orden")}; bar = Counter()
print("| Familia | Trabajos | Piezas (máx.) | Largos distintos (máx.) | Simples: brecha 0 / +1 / +2 o más (peor) | Método: brecha 0 / +1 / +2 o más | Barras: cota · simples · método | Método, s por trabajo: media (máx.) |")
print("|---|---|---|---|---|---|---|---|")
for fam in ORDEN:
    if fam not in datos:
        continue
    r = datos[fam]
    seg = f"{r['seg_final'] / r['trabajos']:.2f} ({r['seg_final_max']:.2f})".replace(".", ",")
    print(f"| {NOTA.get(fam, fam)} | {r['trabajos']} | {r['piezas_max']} | {r['tipos_max']} | {reparto(r['brecha']['simples'])} | "
          f"{reparto(r['brecha']['final'])} | {miles(r['barras_cota'])} · {miles(r['barras']['simples'])} · {miles(r['barras']['final'])} | "
          f"{seg} |")
    tot["trabajos"] += r["trabajos"]; tot["cota"] += r["barras_cota"]
    tot["cota_igual"] += r["cota_igual_a_highs"]; tot["cota_distinta"] += r["cota_distinta"]; tot["no_conv"] += r["lp_no_convergio"]
    tot["err"] = max(tot["err"], r["error_inversa_max"]); tot["vueltas"] = max(tot["vueltas"], r["vueltas_lp_max"])
    tot["mochilas"] = max(tot["mochilas"], r["mochilas_lp_max"])
    for v in br:
        for k, n in r["brecha"][v].items():
            br[v][int(k)] += n
        bar[v] += r["barras"][v]
    for k, n in r["arranques_usados"].items():
        tot[f"arranques_{k}"] += n
print(f"| **Total** | **{miles(tot['trabajos'])}** | | | **{reparto(br['simples'])}** | **{reparto(br['final'])}** | "
      f"**{miles(tot['cota'])} · {miles(bar['simples'])} · {miles(bar['final'])}** | |")
print()
n = tot["trabajos"]
print(f"trabajos {n} | cota propia = HiGHS en {tot['cota_igual']} (distinta en {tot['cota_distinta']}) | LP sin converger {tot['no_conv']}")
for v in br:
    extra = bar[v] - tot["cota"]
    print(f"{v:<11} brecha {dict(sorted(br[v].items()))} | barras {bar[v]} (+{extra} sobre la cota, {100 * extra / tot['cota']:.2f} %) | "
          f"brecha 0 en {100 * br[v][0] / n:.1f} %")
print("arranques usados:", {k: v for k, v in sorted(tot.items()) if k.startswith("arranques_")})
print(f"error maximo de la inversa {tot['err']:.1e} | vueltas max de un LP {tot['vueltas']} | mochilas max de un LP {tot['mochilas']}")
