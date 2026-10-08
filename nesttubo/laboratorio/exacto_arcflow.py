"""Arbitro exacto para los trabajos en los que ningun metodo iguala la cota.

Modelo de flujo en arcos (Valerio de Carvalho, 1999) resuelto como programa
entero con HiGHS. A diferencia de exacto.py, aqui estan TODOS los patrones
posibles, asi que si encuentra una solucion con `cota` barras, esa barra de mas
era evitable; y si demuestra que no existe, el resultado cota+1 era el minimo.

Uso:  python3 exacto_arcflow.py <familia> <trabajos> <semilla> [limite_seg]
Busca en esa familia los trabajos donde el metodo propio no iguala la cota y
los resuelve de forma exacta. Solo para el laboratorio (necesita scipy).
"""
import json, random, sys, time
from collections import Counter

import numpy as np
from scipy.optimize import Bounds, LinearConstraint, milp
from scipy.sparse import coo_matrix

from modelo import transformar
from simplex_propio import resolver, validar_pesos

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


def arcflow(w, C, tope_barras, limite_seg=600.0):
    """Minimo de barras por flujo en arcos. Devuelve (barras o None, estado, cota_dual)."""
    cuenta = Counter(int(v) for v in w)
    pesos = sorted(cuenta, reverse=True)
    dem = [cuenta[p] for p in pesos]
    C = int(C)
    # Posiciones alcanzables colocando las piezas de mayor a menor (rompe simetrias):
    # alcanz[j] = posiciones donde puede EMPEZAR una pieza del tipo j.
    alcanz = [set() for _ in pesos]
    actuales = {0}
    arcos = []                                  # (desde, hasta, tipo)
    for j, p in enumerate(pesos):
        nuevas = set(actuales)
        for inicio in sorted(actuales):
            pos = inicio
            for _ in range(dem[j]):
                if pos + p > C:
                    break
                alcanz[j].add(pos)
                pos += p
                nuevas.add(pos)
        for pos in alcanz[j]:
            arcos.append((pos, pos + p, j))
        actuales = nuevas
    nodos = sorted({0, C} | {a for a, _, _ in arcos} | {b for _, b, _ in arcos})
    idx = {n: i for i, n in enumerate(nodos)}
    perdida = [(n, C) for n in nodos if 0 < n < C]          # arco de sobrante hasta el final
    nv = len(arcos) + len(perdida) + 1                       # +1: z = numero de barras
    iz = nv - 1
    filas, cols, vals = [], [], []
    for k, (a, b, _) in enumerate(arcos):                    # conservacion: sale - entra
        filas += [idx[a], idx[b]]; cols += [k, k]; vals += [1.0, -1.0]
    for k, (a, b) in enumerate(perdida, len(arcos)):
        filas += [idx[a], idx[b]]; cols += [k, k]; vals += [1.0, -1.0]
    filas += [idx[0], idx[C]]; cols += [iz, iz]; vals += [-1.0, 1.0]   # de 0 salen z, a C llegan z
    A_flujo = coo_matrix((vals, (filas, cols)), shape=(len(nodos), nv)).tocsr()
    fd, cd, vd = [], [], []
    for k, (_, _, j) in enumerate(arcos):
        fd.append(j); cd.append(k); vd.append(1.0)
    A_dem = coo_matrix((vd, (fd, cd)), shape=(len(pesos), nv)).tocsr()
    c = np.zeros(nv); c[iz] = 1.0
    ub = np.full(nv, np.inf); ub[iz] = tope_barras
    t0 = time.time()
    res = milp(c, constraints=[LinearConstraint(A_flujo, 0.0, 0.0),
                               LinearConstraint(A_dem, np.array(dem, float), np.array(dem, float))],
               integrality=np.ones(nv), bounds=Bounds(0, ub),
               options={"time_limit": limite_seg, "mip_rel_gap": 0.0})
    barras = int(round(res.fun)) if res.x is not None else None
    dual = getattr(res, "mip_dual_bound", None)
    solucion = None
    if res.x is not None:
        # Descomponer el flujo en caminos de 0 a C: cada camino es una barra.
        # Asi la solucion se puede comprobar pieza por pieza, sin fiarse del modelo.
        flujo = [int(round(v)) for v in res.x[:len(arcos) + len(perdida)]]
        sale = {}
        for k, (a, b2, j) in enumerate(arcos):
            sale.setdefault(a, []).append((k, b2, j))
        for k, (a, b2) in enumerate(perdida, len(arcos)):
            sale.setdefault(a, []).append((k, b2, None))
        solucion = []
        for _ in range(barras):
            nodo, barra = 0, []
            while nodo != C:
                k, sig, j = next(t for t in sale[nodo] if flujo[t[0]] > 0)
                flujo[k] -= 1
                if j is not None:
                    barra.append(pesos[j])
                nodo = sig
            solucion.append(barra)
        assert not any(flujo), "quedo flujo sin asignar a ninguna barra"
    return barras, res.status, dual, len(arcos), round(time.time() - t0, 1), solucion


if __name__ == "__main__":
    fam, n, semilla = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    limite = float(sys.argv[4]) if len(sys.argv) > 4 else 600.0
    tmin, tmax, cmax, lmin, lmax, paso, esc, L, D, Z, S = FAMILIAS[fam]
    random.seed(semilla)
    salida = []
    for k in range(n):
        largos = []
        for _ in range(random.randint(tmin, tmax)):
            l = random.randrange(lmin, lmax + 1, paso)
            largos += [l] * random.randint(1, cmax)
        w, C, util = transformar(largos, L * esc, D * esc, Z * esc, S * esc)
        sol, cota, info = resolver(w, C, clavar=True, arranques=5, semilla=semilla)
        if len(sol) == cota:
            continue
        barras, estado, dual, n_arcos, seg, sol_exacta = arcflow(w, C, tope_barras=len(sol), limite_seg=limite)
        if sol_exacta is not None:              # la solucion del arbitro tambien pasa por el validador
            ok, msg = validar_pesos(w, C, sol_exacta); assert ok, msg
            assert len(sol_exacta) == barras
        # estado 0 = optimo demostrado; 1 = se acabo el tiempo
        veredicto = ("la barra de mas era evitable" if barras == cota else
                     "cota+1 era el minimo" if (estado == 0 and barras == len(sol)) else "sin decidir")
        fila = dict(familia=fam, trabajo=k, piezas=len(w), tipos=len(set(w)), cota=cota, metodo_propio=len(sol),
                    arcflow=barras, estado_milp=int(estado), cota_dual=dual, arcos=n_arcos, seg=seg,
                    solucion_validada=sol_exacta is not None, veredicto=veredicto, largos=sorted(largos, reverse=True),
                    parametros=dict(L=L * esc, despunte=D * esc, zona_muerta=Z * esc, separacion=S * esc))
        salida.append(fila)
        print(json.dumps(fila), flush=True)
    json.dump(salida, open(f"arbitro_{fam}.json", "w"), indent=1)
    print("trabajos sin certificar en", fam, ":", len(salida))
