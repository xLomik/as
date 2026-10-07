"""Referencia exacta para medir: generacion de columnas (Gilmore y Gomory).

Da dos cosas por trabajo:
  - cota_lp: la cota inferior de la relajacion lineal del modelo de patrones,
    mucho mas ajustada que la cota por volumen;
  - barras_mip: una solucion entera sobre los patrones generados.
Si barras_mip == cota_lp, ese numero es el optimo demostrado.

Solo se usa en el laboratorio (necesita scipy/HiGHS). No va en el programa.
"""
from __future__ import annotations

import math
from collections import Counter

import numpy as np
from scipy.optimize import Bounds, LinearConstraint, linprog, milp

from simplex_propio import mochila as _mochila


def resolver(largos_w, C, max_iter=3000):
    """largos_w: pesos ya transformados (enteros). C: capacidad entera."""
    cuenta = Counter(int(x) for x in largos_w)
    pesos = sorted(cuenta, reverse=True)
    dem = [cuenta[p] for p in pesos]
    m = len(pesos)
    cap = int(C)

    # patrones iniciales: uno homogeneo por tipo
    patrones = []
    for j in range(m):
        a = [0] * m
        a[j] = min(dem[j], cap // pesos[j])
        patrones.append(a)

    # El valor del LP restringido solo es cota inferior cuando ya no existe
    # ningun patron que lo mejore. Mientras tanto la cota valida es la de
    # Farley: z / v, con v el valor del mejor patron que encuentra la mochila.
    z, convergio, cota_farley = None, False, 0.0
    for _ in range(max_iter):
        A = np.array(patrones, dtype=float).T            # m x n_patrones
        r = linprog(np.ones(A.shape[1]), A_ub=-A, b_ub=-np.array(dem, dtype=float),
                    bounds=(0, None), method="highs")
        if r.status != 0:
            raise RuntimeError("el LP maestro no resolvio: " + r.message)
        z = r.fun
        duales = -np.asarray(r.ineqlin.marginals)        # precios de cada tipo
        valor, patron = _mochila(list(duales), pesos, dem, cap)
        assert sum(pj * aj for pj, aj in zip(pesos, patron)) <= cap, "patron que no cabe"
        cota_farley = max(cota_farley, z / max(valor, 1.0))
        if valor <= 1 + 1e-7:
            convergio = True
            break
        patrones.append(patron)

    cota_lp = math.ceil((z if convergio else cota_farley) - 1e-6)

    # solucion entera usando solo los patrones generados
    A = np.array(patrones, dtype=float).T
    n = A.shape[1]
    res = milp(c=np.ones(n),
               constraints=LinearConstraint(A, lb=np.array(dem, dtype=float), ub=np.inf),
               integrality=np.ones(n), bounds=Bounds(0, np.inf),
               options={"time_limit": 20.0})
    barras_mip = int(round(res.fun)) if res.success else None
    return cota_lp, barras_mip, len(patrones), convergio
