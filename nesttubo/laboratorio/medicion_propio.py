"""Mide el metodo sin solver (simplex_propio.py) sobre los mismos trabajos de
medicion_final.py (misma semilla -> mismos trabajos).

Por cada trabajo compara contra la cota de la relajacion lineal:
  simples    la mejor de FFD, BFD y llenado por subset-sum
  residual   LP propio + parte entera + cierre con simples (sin clavado, una pasada)
  final      con clavado y hasta 5 arranques (lo que se propone para el programa)
  peor_orden una pasada con clavado y los largos en 2 ordenes al azar: el PEOR
             de los 2 (mide cuanto depende el resultado del vertice del LP)
y verifica que la cota del LP propio sea la misma que da HiGHS.

Uso:  python3 medicion_propio.py <familia> <trabajos> <semilla> [sin_highs]
Escribe propio_<familia>.json
"""
import json, random, sys, time
from collections import Counter
from modelo import transformar, ffd, bfd, llenado_exacto
from residual import lp_patrones, validar_pesos
from simplex_propio import lp_propio, resolver

FAMILIAS = {
    "chico":    (2, 6, 8,   120, 3000, 1, 1, 6000, 10, 230, 3),
    "medio":    (5, 15, 20, 120, 3000, 1, 1, 6000, 10, 230, 3),
    "grande":   (10, 30, 40, 120, 3000, 1, 1, 6000, 10, 230, 3),
    "largas":   (3, 10, 12, 1500, 3500, 1, 1, 6000, 10, 230, 3),
    "redondos": (4, 12, 24, 100, 3000, 5, 1, 6000, 10, 230, 3),
    "decimal":  (4, 10, 16, 1200, 30000, 5, 10, 6000, 10, 230, 3),
    "barra12":  (5, 15, 20, 300, 6000, 1, 1, 12000, 10, 230, 3),
    "sin_margen": (5, 15, 20, 120, 3000, 1, 1, 6000, 0, 0, 0),
    "variado":  (40, 120, 3, 120, 3000, 1, 1, 6000, 10, 230, 3),     # muchas piezas distintas, pocas de cada una
}
fam, n, semilla = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
con_highs = len(sys.argv) < 5
tmin, tmax, cmax, lmin, lmax, paso, esc, L, D, Z, S = FAMILIAS[fam]
random.seed(semilla)
otro = random.Random(semilla + 1)          # para los ordenes al azar, sin tocar el generador de trabajos
VAR = ("simples", "residual", "final", "peor_orden")
r = dict(familia=fam, trabajos=n, semilla=semilla,
         cota_igual_a_highs=0, cota_distinta=0, lp_no_convergio=0,
         brecha={v: Counter() for v in VAR}, barras={v: 0 for v in VAR}, barras_cota=0,
         arranques_usados=Counter(), tipos_max=0, piezas_max=0,
         seg_final=0.0, seg_final_max=0.0, n_lp_max=0, mochilas_max=0)
diag = {}
t0 = time.time()
for _ in range(n):
    largos = []
    for _ in range(random.randint(tmin, tmax)):
        l = random.randrange(lmin, lmax + 1, paso)
        largos += [l] * random.randint(1, cmax)
    w, C, util = transformar(largos, L * esc, D * esc, Z * esc, S * esc)
    cuenta = Counter(int(v) for v in w)
    pesos = sorted(cuenta, reverse=True)
    dem = [cuenta[p] for p in pesos]
    cota, _, _, conv, _ = lp_propio(pesos, dem, int(C), diagnostico=diag)
    r["lp_no_convergio"] += (not conv)
    if con_highs:
        cota_h, _, _ = lp_patrones(pesos, dem, int(C))
        r["cota_igual_a_highs"] += (cota == cota_h)
        r["cota_distinta"] += (cota != cota_h)
    r["tipos_max"] = max(r["tipos_max"], len(pesos)); r["piezas_max"] = max(r["piezas_max"], len(w))

    res = {}
    res["simples"] = min(len(ffd(w, C)), len(bfd(w, C)), len(llenado_exacto(w, C)))
    sol, c, info = resolver(w, C, arranques=1, clavar=False)
    ok, msg = validar_pesos(w, C, sol); assert ok, msg
    assert c == cota
    res["residual"] = len(sol)
    ta = time.time()
    sol, c, info = resolver(w, C, clavar=True, arranques=5, semilla=semilla)
    dt = time.time() - ta
    ok, msg = validar_pesos(w, C, sol); assert ok, msg
    res["final"] = len(sol)
    r["arranques_usados"][info["arranques"]] += 1
    r["n_lp_max"] = max(r["n_lp_max"], info["n_lp"])
    r["mochilas_max"] = max(r["mochilas_max"], info["mochilas"])
    r["seg_final"] += dt; r["seg_final_max"] = max(r["seg_final_max"], dt)
    peor = 0
    for _ in range(2):
        orden = list(range(len(pesos))); otro.shuffle(orden)
        sol, c, info = resolver(w, C, clavar=True, arranques=1, orden=orden)
        ok, msg = validar_pesos(w, C, sol); assert ok, msg
        peor = max(peor, len(sol))
    res["peor_orden"] = peor
    for v in VAR:
        assert res[v] >= cota, "una solucion no puede bajar de la cota"
        r["brecha"][v][res[v] - cota] += 1
        r["barras"][v] += res[v]
    r["barras_cota"] += cota
r["segundos_total"] = round(time.time() - t0, 1)
r["seg_final"] = round(r["seg_final"], 1); r["seg_final_max"] = round(r["seg_final_max"], 2)
r["brecha"] = {v: dict(sorted(c.items())) for v, c in r["brecha"].items()}
r["arranques_usados"] = dict(sorted(r["arranques_usados"].items()))
r["error_inversa_max"] = diag.get("error_inversa", 0.0)
r["vueltas_lp_max"] = diag.get("vueltas", 0); r["mochilas_lp_max"] = diag.get("mochilas", 0)
json.dump(r, open(f"propio_{fam}.json", "w"), indent=1)
print(json.dumps(r))
