"""Lo que SI se puede llevar a un .exe sin librerias: la relajacion lineal de
patrones (un simplex chico) mas redondeo, sin solver entero.

Idea (heuristica residual clasica del corte 1D):
  1. resolver el LP de patrones por generacion de columnas;
  2. usar cada patron la parte entera de las veces que pide el LP;
  3. repetir con lo que falta; cuando el LP ya no deja nada entero,
     cerrar el resto con las heuristicas simples.
"""
from __future__ import annotations

import math
from collections import Counter

import numpy as np
from scipy.optimize import linprog

from exacto import _mochila
from modelo import bfd, ffd, llenado_exacto


def lp_patrones(pesos, dem, cap, max_iter=3000):
    m = len(pesos)
    patrones = []
    for j in range(m):
        a = [0] * m
        a[j] = min(dem[j], cap // pesos[j])
        patrones.append(a)
    z, x, convergio, farley = None, None, False, 0.0
    for _ in range(max_iter):
        A = np.array(patrones, dtype=float).T
        r = linprog(np.ones(A.shape[1]), A_ub=-A, b_ub=-np.array(dem, dtype=float),
                    bounds=(0, None), method="highs")
        z, x = r.fun, r.x
        duales = -np.asarray(r.ineqlin.marginals)
        valor, patron = _mochila(list(duales), pesos, dem, cap)
        farley = max(farley, z / max(valor, 1.0))
        if valor <= 1 + 1e-7:
            convergio = True
            break
        patrones.append(patron)
    cota = math.ceil((z if convergio else farley) - 1e-6)
    return cota, patrones, x


def residual(w, C):
    """Devuelve (barras, cota_lp). barras = lista de listas de pesos. Sin solver entero."""
    cuenta = Counter(int(v) for v in w)
    pesos = sorted(cuenta, reverse=True)
    dem = [cuenta[p] for p in pesos]
    cap = int(C)
    barras, cota0 = [], None
    for _ in range(50):
        vivos = [j for j in range(len(pesos)) if dem[j] > 0]
        if not vivos:
            break
        ps = [pesos[j] for j in vivos]
        ds = [dem[j] for j in vivos]
        cota, patrones, x = lp_patrones(ps, ds, cap)
        if cota0 is None:
            cota0 = cota
        avance = 0
        for pat, veces in zip(patrones, x):
            k = int(math.floor(veces + 1e-9))
            for _ in range(k):
                # no producir de mas: el patron se recorta a lo que falta
                usa = [min(a, dem[vivos[i]]) for i, a in enumerate(pat)]
                if sum(usa) == 0:
                    break
                barra = []
                for i, a in enumerate(usa):
                    dem[vivos[i]] -= a
                    barra += [ps[i]] * a
                barras.append(barra)
                avance += 1
        if avance == 0:
            break
    resto = [pesos[j] for j in range(len(pesos)) for _ in range(dem[j])]
    if resto:
        cands = [ffd(resto, cap), bfd(resto, cap), llenado_exacto(resto, cap)]
        mejor = min(cands, key=len)
        barras += [[resto[i] for i in b] for b in mejor]
    return barras, cota0


def validar_pesos(w, C, barras):
    """Comprobacion independiente: mismas piezas, y ninguna barra pasada de capacidad."""
    pedido = Counter(int(v) for v in w)
    puesto = Counter(p for b in barras for p in b)
    if pedido != puesto:
        return False, "las piezas colocadas no son las pedidas"
    for k, b in enumerate(barras):
        if sum(b) > int(C):
            return False, f"barra {k+1} pasada: {sum(b)} > {int(C)}"
        if not b:
            return False, f"barra {k+1} vacia"
    return True, "ok"
