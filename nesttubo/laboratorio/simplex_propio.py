"""El metodo completo SIN solver: es la referencia para portar a C++.

No usa scipy ni ningun solver. numpy se usa solo como calculadora de vectores
(producto matriz-vector y la tabla de la mochila) para que el laboratorio
corra rapido; en C++ son bucles simples.

Piezas:
  lp_propio()   simplex revisado con la inversa de la base explicita y
                generacion de columnas (Gilmore y Gomory). Las columnas las
                genera una mochila acotada; nunca se enumeran todas. Las ya
                generadas se guardan en una "cantera" y se reutilizan.
  resolver()    1) usa la parte entera de la solucion del LP y repite sobre lo
                   que falta ("residual");
                2) cuando el LP ya no deja nada entero, CLAVA el patron mas
                   usado (lo usa una vez) y vuelve a resolver ("clavado");
                3) en cada paso prueba cerrar lo que falta con las heuristicas
                   simples y se queda con el mejor total;
                4) si no igualo la cota, reintenta con los largos en otro
                   orden (otro vertice del LP), hasta `arranques` veces.

Formulacion:  min sum(x_p)   sujeto a   sum_p a_jp * x_p >= demanda_j,  x >= 0
  a_jp = cuantas piezas del largo j lleva el patron p (0 <= a_jp <= demanda_j,
  y el patron cabe en la barra). Se usa la forma >= (cubrir) y no la forma =
  porque es la habitual y deja los precios >= 0. Al usar un patron se recorta
  a lo que falta, asi que nunca se produce de mas.
"""
from __future__ import annotations

import math
import random
from collections import Counter

import numpy as np

from modelo import bfd, ffd, llenado_exacto

EPS_COSTO = 1e-7      # un patron entra solo si vale mas que 1 + EPS_COSTO
EPS_PRECIO = 1e-9     # un precio por debajo de -EPS_PRECIO hace entrar la holgura de ese largo
EPS_PERTURBA = 1e-6   # tamano de la perturbacion de la demanda que evita el ciclado
EPS_PIVOTE = 1e-9     # componentes menores no sirven de pivote


def mochila(valores, pesos, tope, cap):
    """Mochila acotada entera: max sum(v_j a_j), sum(w_j a_j) <= cap, 0 <= a_j <= tope_j."""
    m = len(pesos)
    mejor = np.zeros(cap + 1)
    eleccion = []                       # por cada "paquete": (tipo, copias, tabla de decision)
    for j in range(m):
        resto, k = tope[j], 1
        while resto > 0:                # descomposicion binaria de la demanda
            c = min(k, resto)
            resto -= c
            k *= 2
            p, v = pesos[j] * c, valores[j] * c
            if p > cap or v <= 1e-12:
                continue
            cand = np.full(cap + 1, -np.inf)
            cand[p:] = mejor[:-p] + v if p > 0 else mejor + v
            tomar = cand > mejor + 1e-12
            mejor = np.where(tomar, cand, mejor)
            eleccion.append((j, c, p, tomar))
    c = int(np.argmax(mejor))
    valor = float(mejor[c])
    patron = [0] * m
    for j, copias, p, tomar in reversed(eleccion):
        if tomar[c]:
            patron[j] += copias
            c -= p
    return valor, patron


def lp_propio(pesos, dem, cap, cantera=None, max_iter=200000, diagnostico=None):
    """Relajacion lineal del modelo de patrones por generacion de columnas.

        min sum(x_p)   sujeto a   sum_p a_jp * x_p >= demanda_j,   x >= 0

    Devuelve (cota, patrones, veces, convergio, info). `patrones` y `veces` son
    solo los patrones que quedaron en la base (las holguras no se devuelven).

    Algebra densa de m x m (m = largos distintos). Aqui va con numpy; en C++
    son bucles sobre un vector<double> de m*m.

    Cada posicion de la base es un patron (cuesta 1 barra) o una holgura de
    exceso -e_j (cuesta 0: deja cubrir de mas el largo j).

    CANTERA: todos los patrones generados se guardan. En cada vuelta primero se
    busca en la cantera un patron que mejore (un producto matriz-vector, muy
    barato) y solo si no hay ninguno se llama a la mochila, que es lo caro.
    Sin cantera, un patron que sale de la base se olvida y hay que volver a
    generarlo: con 79 largos distintos eran 1.884 mochilas en vez de 368.
    `cantera` (lista de dicts peso->cantidad) trae patrones de un LP anterior y
    sale ampliada con los nuevos, para que el siguiente LP arranque adelantado.
    """
    m = len(pesos)
    d = np.array(dem, dtype=float)
    # ANTICICLADO. Este LP es muy degenerado (muchos empates en la prueba de la razon) y sin
    # proteccion el simplex puede dar vueltas sin fin entre las mismas bases: se vio en un
    # trabajo de 75 largos distintos (200.000 vueltas sin converger). El remedio clasico es
    # perturbar un poco la demanda SOLO para elegir quien sale de la base: cada largo recibe
    # un extra distinto, del orden de 1e-6, que deshace los empates. Los precios no dependen
    # de la demanda, asi que la cota no cambia; y la solucion final se calcula con la demanda real.
    dp = d + EPS_PERTURBA * (0.5 + np.modf(0.6180339887498949 * np.arange(1, m + 1))[0])
    base = []                                # base[i] = patron (lista) o None si es una holgura
    holgura = [-1] * m                       # holgura[i] = j si la posicion i es la holgura del largo j
    costo, inv = np.ones(m), np.zeros((m, m))
    cols, vistos = [], set()                 # la cantera, en el orden de `pesos`
    P = np.zeros((max(64, 4 * m), m))        # la misma cantera como matriz, para valorar todos de una vez

    def guardar(pat):
        nonlocal P
        t = tuple(pat)
        if t in vistos or not any(pat):
            return
        vistos.add(t)
        if len(cols) == P.shape[0]:
            P = np.vstack([P, np.zeros_like(P)])
        P[len(cols)] = pat
        cols.append(list(pat))

    for j in range(m):                       # base inicial: un patron homogeneo por largo
        a = [0] * m
        a[j] = min(dem[j], cap // pesos[j])
        base.append(a)
        inv[j, j] = 1.0 / a[j]
        guardar(a)
    for pd in (cantera or []):               # patrones heredados, recortados a la demanda de ahora
        guardar([min(pd.get(p, 0), dem[j]) for j, p in enumerate(pesos)])

    x, convergio, farley, it, mochilas = None, False, 0.0, 0, 0
    for it in range(1, max_iter + 1):
        # x = inv * demanda perturbada (se recalcula entero en cada vuelta: no acumula error)
        x = np.maximum(inv @ dp, 0.0)
        y = costo @ inv                      # precio de cada largo
        j = int(np.argmin(y))
        if y[j] < -EPS_PRECIO:
            # un precio negativo: entra la holgura de ese largo (columna -e_j, costo 0)
            entra, j_holgura, c_entra = None, j, 0.0
            col = np.zeros(m)
            col[j] = -1.0
        else:
            yp = np.maximum(y, 0.0)
            valores = P[:len(cols)] @ yp     # 1) lo barato: algun patron de la cantera mejora?
            k = int(np.argmax(valores))
            if valores[k] > 1.0 + EPS_COSTO:
                entra = cols[k]
            else:                            # 2) lo caro: la mochila busca entre TODOS los patrones
                mochilas += 1
                valor, entra = mochila(list(yp), pesos, dem, cap)
                # Cota de Farley: (demanda . precios) / (valor del mejor patron). Es una
                # cota inferior valida en CUALQUIER vuelta, haya convergido o no, y no
                # depende de las tolerancias. Por eso la cota sale de aqui.
                farley = max(farley, float(d @ yp) / max(valor, 1.0))
                if valor <= 1.0 + EPS_COSTO:
                    convergio = True
                    break
                guardar(entra)
            j_holgura, c_entra = -1, 1.0
            col = np.array(entra, dtype=float)
        u = inv @ col
        sirve = u > EPS_PIVOTE               # prueba de la razon: sale quien antes llega a cero
        if not sirve.any():
            raise RuntimeError("LP no acotado: no deberia pasar en este problema")
        sale = int(np.argmin(np.where(sirve, x / np.where(sirve, u, 1.0), np.inf)))   # empate -> indice menor
        fila = inv[sale] / u[sale]
        inv -= np.outer(u, fila)             # a cada fila i se le resta u[i] * fila
        inv[sale] = fila
        base[sale], holgura[sale], costo[sale] = entra, j_holgura, c_entra
    info = dict(vueltas=it, mochilas=mochilas, cantera=len(cols))
    if diagnostico is not None:              # cuanto se desvio la inversa de la base
        B = np.zeros((m, m))
        for i in range(m):
            if base[i] is None:
                B[holgura[i], i] = -1.0
            else:
                B[:, i] = base[i]
        diagnostico["error_inversa"] = max(diagnostico.get("error_inversa", 0.0),
                                           float(np.abs(B @ inv - np.eye(m)).max()))
        for k2 in ("vueltas", "mochilas"):
            diagnostico[k2] = max(diagnostico.get(k2, 0), info[k2])
    if cantera is not None:                  # devolver la cantera ampliada, en forma peso -> cantidad
        cantera[:] = [{pesos[j]: a for j, a in enumerate(c) if a} for c in cols]
    cota = math.ceil(farley - 1e-6)          # 1e-6 >> error de redondeo: nunca redondea de mas
    x = np.maximum(inv @ d, 0.0)             # la solucion, con la demanda real
    patrones = [b for b in base if b is not None]
    veces = [float(x[i]) for i, b in enumerate(base) if b is not None]
    return cota, patrones, veces, convergio, info


def _simples(pesos, dem, cap):
    """Las tres heuristicas simples sobre lo pendiente. Devuelve las tres soluciones (listas de barras)."""
    resto = [pesos[j] for j in range(len(pesos)) for _ in range(dem[j])]
    if not resto:
        return [[]]
    return [[[resto[i] for i in b] for b in sol] for sol in (ffd(resto, cap), bfd(resto, cap), llenado_exacto(resto, cap))]


def _una_pasada(pesos, dem, cap, clavar=True, cantera=None, max_pasos=100000):
    """Una pasada completa con los largos en el orden dado.

    Devuelve (barras, cota, convergio, info). cota es la del primer LP, o sea
    la del trabajo completo.
    """
    dem = list(dem)
    hechas = []                 # barras ya decididas
    mejor = None                # mejor solucion completa vista
    cantera = [] if cantera is None else cantera
    cota0, conv0 = None, True
    info = dict(n_lp=0, vueltas=0, mochilas=0)
    for _ in range(max_pasos):
        vivos = [j for j in range(len(pesos)) if dem[j] > 0]
        if not vivos:
            if mejor is None or len(hechas) < len(mejor):
                mejor = list(hechas)
            break
        ps = [pesos[j] for j in vivos]
        ds = [dem[j] for j in vivos]
        # candidato: lo decidido hasta aqui + cierre con heuristicas simples
        cierres = _simples(ps, ds, cap)
        cand = hechas + min(cierres, key=len)
        if mejor is None or len(cand) < len(mejor):
            mejor = cand
        if cota0 is not None and len(mejor) == cota0:
            break               # igualo la cota: es el minimo, no hay nada mejor
        for sol in cierres:     # los patrones de las heuristicas adelantan al LP
            for barra in sol:
                cantera.append(dict(Counter(barra)))
        cota, base, x, convergio, inf = lp_propio(ps, ds, cap, cantera=cantera)
        info["n_lp"] += 1
        info["vueltas"] += inf["vueltas"]
        info["mochilas"] += inf["mochilas"]
        if cota0 is None:
            cota0, conv0 = cota, convergio
        if len(mejor) == cota0:
            break
        if len(hechas) + cota >= len(mejor):
            break               # por este camino ya no se puede mejorar
        avance = 0
        for pat, veces in zip(base, x):
            for _ in range(int(math.floor(veces + 1e-9))):
                usa = [min(a, dem[vivos[i]]) for i, a in enumerate(pat)]   # nunca producir de mas
                if sum(usa) == 0:
                    break
                for i, a in enumerate(usa):
                    dem[vivos[i]] -= a
                hechas.append([ps[i] for i, a in enumerate(usa) for _ in range(a)])
                avance += 1
        if avance:
            continue
        if not clavar:
            break
        # nada entero: clavar el patron mas usado; empate -> el que llena mas la barra
        k = max(range(len(base)), key=lambda i: (round(x[i], 9), sum(a * p for a, p in zip(base[i], ps))))
        usa = [min(a, dem[vivos[i]]) for i, a in enumerate(base[k])]
        if sum(usa) == 0:
            break
        for i, a in enumerate(usa):
            dem[vivos[i]] -= a
        hechas.append([ps[i] for i, a in enumerate(usa) for _ in range(a)])
    return mejor, cota0, conv0, info


def resolver(w, C, clavar=True, arranques=1, semilla=0, orden=None):
    """Devuelve (barras, cota, info). barras = lista de listas de pesos.

    arranques > 1: si no se igualo la cota, reintenta con los largos en otro
    orden. `orden` fuerza un orden concreto de los largos (para pruebas).
    """
    cuenta = Counter(int(v) for v in w)
    pesos = sorted(cuenta, reverse=True)
    if pesos and pesos[0] > int(C):
        raise ValueError("hay una pieza que no cabe en la barra: quien llama debe apartarla antes")
    if orden is not None:
        pesos = [pesos[i] for i in orden]
    cap = int(C)
    rng = random.Random(semilla)
    mejor, cota = None, None
    info = dict(arranques=0, n_lp=0, vueltas=0, mochilas=0, convergio=True)
    for a in range(arranques):
        dem = [cuenta[p] for p in pesos]
        sol, c, conv, inf = _una_pasada(pesos, dem, cap, clavar=clavar)
        info["arranques"] += 1
        for k in ("n_lp", "vueltas", "mochilas"):
            info[k] += inf[k]
        info["convergio"] = info["convergio"] and conv
        cota = c if cota is None else max(cota, c)      # toda cota es valida: vale la mayor
        if mejor is None or len(sol) < len(mejor):
            mejor = sol
        if len(mejor) == cota:
            break
        rng.shuffle(pesos)
    return mejor, cota, info


def residual_propio(w, C):
    """Variante sin clavado ni reintentos (la primera que se midio)."""
    sol, cota, info = resolver(w, C, clavar=False, arranques=1)
    return sol, cota, info["convergio"]


def validar_pesos(w, C, barras):
    """Comprobacion independiente: mismas piezas que las pedidas y ninguna barra pasada."""
    pedido = Counter(int(v) for v in w)
    puesto = Counter(p for b in barras for p in b)
    if pedido != puesto:
        return False, "las piezas colocadas no son las pedidas"
    for k, b in enumerate(barras):
        if not b:
            return False, f"barra {k+1} vacia"
        if sum(b) > int(C):
            return False, f"barra {k+1} pasada: {sum(b)} > {int(C)}"
    return True, "ok"
