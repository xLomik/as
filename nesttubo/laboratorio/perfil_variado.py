"""Donde se va el tiempo cuando hay muchos largos distintos y pocas piezas de cada uno."""
import random, sys, time
from collections import Counter
from modelo import transformar, ffd, bfd, llenado_exacto
from simplex_propio import lp_propio, resolver, validar_pesos

tipos = int(sys.argv[1]); cmax = int(sys.argv[2]); semilla = int(sys.argv[3])
random.seed(semilla)
largos = []
for _ in range(tipos):
    largos += [random.randrange(120, 3001)] * random.randint(1, cmax)
w, C, util = transformar(largos, 6000, 10, 230, 3)
cuenta = Counter(w); pesos = sorted(cuenta, reverse=True); dem = [cuenta[p] for p in pesos]
t0 = time.time(); simples = min(len(ffd(w, C)), len(bfd(w, C)), len(llenado_exacto(w, C))); ts = time.time() - t0
d = {}
t0 = time.time(); cota, base, x, conv, inf = lp_propio(pesos, dem, int(C), diagnostico=d); tl = time.time() - t0
print(f"tipos {len(pesos)} piezas {len(w)} | simples {simples} ({ts:.2f} s) | LP solo: cota {cota} convergio {conv} "
      f"vueltas {inf['vueltas']} mochilas {inf['mochilas']} en {tl:.1f} s, error_inv {d['error_inversa']:.1e}", flush=True)
t0 = time.time(); sol, c, info = resolver(w, C, clavar=True, arranques=1); tr = time.time() - t0
ok, msg = validar_pesos(w, C, sol); assert ok, msg
print(f"   metodo 1 pasada: {len(sol)} barras (cota {c}) {info} en {tr:.1f} s", flush=True)
