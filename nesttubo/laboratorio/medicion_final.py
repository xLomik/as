"""Medicion de referencia para el proyecto de corte de tubo (solo largos).

Compara, en trabajos sinteticos con forma de taller:
  simples   -> la mejor de FFD, BFD y llenado por subset-sum
  residual  -> relajacion lineal de patrones + redondeo + cierre con simples
contra el minimo de barras, que se da por conocido cuando una solucion
iguala la cota de la relajacion lineal.

Uso:  python3 medicion_final.py <familia> <trabajos> <semilla>
Escribe resultados_<familia>.json
"""
import json, random, sys, time
from collections import Counter
from modelo import transformar, ffd, bfd, llenado_exacto, validar
from exacto import resolver
from residual import residual, validar_pesos

# (tipos min, tipos max, cantidad max, largo min, largo max, paso, escala, L, despunte, zona muerta, separacion)
FAMILIAS = {
    "chico":    (2, 6, 8,   120, 3000, 1, 1, 6000, 10, 230, 3),
    "medio":    (5, 15, 20, 120, 3000, 1, 1, 6000, 10, 230, 3),
    "grande":   (10, 30, 40, 120, 3000, 1, 1, 6000, 10, 230, 3),
    "largas":   (3, 10, 12, 1500, 3500, 1, 1, 6000, 10, 230, 3),
    "redondos": (4, 12, 24, 100, 3000, 5, 1, 6000, 10, 230, 3),      # largos multiplos de 5 mm
    "decimal":  (4, 10, 16, 1200, 30000, 5, 10, 6000, 10, 230, 3),   # largos con 1 decimal (x0.1 mm)
    "barra12":  (5, 15, 20, 300, 6000, 1, 1, 12000, 10, 230, 3),     # barra de 12 m
    "sin_margen": (5, 15, 20, 120, 3000, 1, 1, 6000, 0, 0, 0),       # sin despunte, zona muerta ni separacion
}
fam, n, semilla = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
tmin, tmax, cmax, lmin, lmax, paso, esc, L, D, Z, S = FAMILIAS[fam]
random.seed(semilla)
r = dict(familia=fam, trabajos=n, semilla=semilla, parametros=dict(L=L, despunte=D, zona_muerta=Z, separacion=S, escala=esc),
         minimo_conocido=0, simples=Counter(), residual=Counter(), residual_certifica=0,
         barras_minimas=0, barras_simples=0, barras_residual=0, piezas_min=10**9, piezas_max=0, barras_max=0)
t0 = time.time()
for _ in range(n):
    largos = []
    for _ in range(random.randint(tmin, tmax)):
        l = random.randrange(lmin, lmax + 1, paso)
        largos += [l] * random.randint(1, cmax)
    # en la familia decimal los largos ya vienen en decimas de mm
    w, C, util = transformar(largos, L * esc, D * esc, Z * esc, S * esc)
    cota, mip, _, conv = resolver(w, C)
    sols = [ffd(w, C), bfd(w, C), llenado_exacto(w, C)]
    for b in sols:
        ok, msg = validar(largos, b, L * esc, D * esc, Z * esc, S * esc); assert ok, msg
    simples = min(len(b) for b in sols)
    sol, cota_r = residual(w, C)
    ok, msg = validar_pesos(w, C, sol); assert ok, msg
    res = len(sol)
    assert cota_r == cota, "las dos generaciones de columnas deben dar la misma cota"
    r["residual_certifica"] += (min(res, simples) == cota)
    minimo = None
    for cand in (mip, res, simples):
        if cand is not None and cand == cota:
            minimo = cand
    if minimo is None:
        continue
    r["minimo_conocido"] += 1
    r["simples"][simples - minimo] += 1
    r["residual"][res - minimo] += 1
    r["barras_minimas"] += minimo; r["barras_simples"] += simples; r["barras_residual"] += res
    r["piezas_min"] = min(r["piezas_min"], len(largos)); r["piezas_max"] = max(r["piezas_max"], len(largos))
    r["barras_max"] = max(r["barras_max"], minimo)
r["segundos"] = round(time.time() - t0, 1)
r["simples"] = dict(sorted(r["simples"].items())); r["residual"] = dict(sorted(r["residual"].items()))
json.dump(r, open(f"resultados_{fam}.json", "w"), indent=1)
print(json.dumps(r))
