"""Prototipo de laboratorio: corte 1D de tubo (solo largos).

No es el programa final. Sirve para comprobar el modelo y medir que tan
cerca del optimo quedan las heuristicas simples, antes de escribir el .exe.

Modelo de una barra:
    L  largo de la barra
    d  despunte inicial (saneado de la punta)
    z  zona muerta del mandril (tramo final que no se puede cortar)
    s  separacion entre piezas consecutivas (kerf + holgura)
    util = L - d - z
    n piezas de largos l_1..l_n caben si  sum(l_i) + (n-1)*s <= util

Transformacion a bin packing clasico (sumar s a ambos lados):
    sum(l_i + s) <= util + s
o sea pesos w_i = l_i + s y capacidad C = util + s.
"""
from __future__ import annotations

import math
from bisect import insort


def transformar(largos, L, d, z, s):
    util = L - d - z
    C = util + s
    w = [l + s for l in largos]
    return w, C, util


def cota_l1(w, C):
    return math.ceil(sum(w) / C - 1e-12)


def cota_l2(w, C):
    """Cota L2 de Martello y Toth (1990): domina a L1."""
    mejor = cota_l1(w, C)
    ws = sorted(w, reverse=True)
    candidatos = sorted({x for x in ws if x <= C / 2} | {0})
    for a in candidatos:
        j1 = [x for x in ws if x > C - a]            # no comparten barra con nada >= a
        j2 = [x for x in ws if C - a >= x > C / 2]   # una por barra
        j3 = [x for x in ws if C / 2 >= x >= a]
        libre = len(j2) * C - sum(j2)
        extra = max(0, math.ceil((sum(j3) - libre) / C - 1e-12))
        mejor = max(mejor, len(j1) + len(j2) + extra)
    return mejor


def ffd(w, C):
    """First-fit decreasing. Devuelve lista de barras (listas de indices)."""
    orden = sorted(range(len(w)), key=lambda i: -w[i])
    barras, resto = [], []
    for i in orden:
        for b in range(len(barras)):
            if resto[b] >= w[i] - 1e-9:
                barras[b].append(i)
                resto[b] -= w[i]
                break
        else:
            barras.append([i])
            resto.append(C - w[i])
    return barras


def bfd(w, C):
    """Best-fit decreasing: la barra donde queda menos hueco."""
    orden = sorted(range(len(w)), key=lambda i: -w[i])
    barras, resto = [], []
    for i in orden:
        mejor, mb = None, None
        for b in range(len(barras)):
            if resto[b] >= w[i] - 1e-9 and (mejor is None or resto[b] < mejor):
                mejor, mb = resto[b], b
        if mb is None:
            barras.append([i])
            resto.append(C - w[i])
        else:
            barras[mb].append(i)
            resto[mb] -= w[i]
    return barras


def llenado_exacto(w, C, resolucion=1):
    """Llena una barra a la vez con el subconjunto que deja menos hueco.

    Subset-sum exacto por programacion dinamica sobre enteros. Es la idea de
    "minimum bin slack": cada barra queda lo mas llena posible con lo que
    resta. No garantiza el minimo global de barras, pero suele cerrar el
    hueco que deja FFD.
    """
    cap = int(round(C / resolucion))
    pend = sorted(range(len(w)), key=lambda i: -w[i])
    wi = {i: int(math.ceil(w[i] / resolucion - 1e-9)) for i in pend}
    barras = []
    while pend:
        # la pieza mas larga pendiente va fija: evita dejar las grandes para el final
        fijo = pend[0]
        resto = cap - wi[fijo]
        otros = pend[1:]
        # dp[c] = indice del ultimo item usado para alcanzar exactamente c (o -1)
        alcanz = [None] * (resto + 1)
        alcanz[0] = (-1, -1)
        for k, i in enumerate(otros):
            p = wi[i]
            if p > resto:
                continue
            for c in range(resto, p - 1, -1):
                if alcanz[c] is None and alcanz[c - p] is not None and alcanz[c - p][0] < k:
                    alcanz[c] = (k, c - p)
        c = max(x for x in range(resto + 1) if alcanz[x] is not None)
        elegido = [fijo]
        while c > 0:
            k, ant = alcanz[c]
            elegido.append(otros[k])
            c = ant
        barras.append(elegido)
        usados = set(elegido)
        pend = [i for i in pend if i not in usados]
    return barras


def exacto_pequeno(w, C, limite_nodos=2_000_000):
    """Minimo de barras por branch and bound. Solo para instancias chicas."""
    n = len(w)
    orden = sorted(range(n), key=lambda i: -w[i])
    ws = [w[i] for i in orden]
    mejor = [len(ffd(w, C))]
    lb = cota_l2(w, C)
    if mejor[0] == lb:
        return mejor[0], True
    nodos = [0]
    resto = []

    def rec(k):
        if nodos[0] > limite_nodos:
            return
        nodos[0] += 1
        if len(resto) >= mejor[0]:
            return
        if k == n:
            mejor[0] = len(resto)
            return
        vistos = set()
        for b in range(len(resto)):
            r = round(resto[b], 6)
            if r in vistos or resto[b] < ws[k] - 1e-9:
                continue
            vistos.add(r)
            resto[b] -= ws[k]
            rec(k + 1)
            resto[b] += ws[k]
            if mejor[0] == lb:
                return
        if len(resto) + 1 < mejor[0]:
            resto.append(C - ws[k])
            rec(k + 1)
            resto.pop()

    rec(0)
    return mejor[0], nodos[0] <= limite_nodos


def validar(largos, barras, L, d, z, s):
    """Comprobacion independiente de la solucion, en unidades reales."""
    util = L - d - z
    usados = sorted(i for b in barras for i in b)
    if usados != list(range(len(largos))):
        return False, "las piezas colocadas no coinciden con las pedidas"
    for k, b in enumerate(barras):
        ocupado = sum(largos[i] for i in b) + (len(b) - 1) * s
        if ocupado > util + 1e-6:
            return False, f"la barra {k+1} ocupa {ocupado:.2f} y solo hay {util:.2f}"
    return True, "ok"
