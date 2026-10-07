"""Pruebas del metodo sin solver (simplex_propio.py). Deben dar FALLOS: 0."""
import random
from modelo import exacto_pequeno, transformar
from simplex_propio import resolver, validar_pesos

casos = [
    # (descripcion, largos, L, despunte, zona muerta, separacion, barras esperadas)
    ("dos mitades exactas, sin separacion",        [3000, 3000],        6000, 0, 0, 0, 1),
    ("dos mitades y 1 mm de mas",                  [3000, 3000, 1],     6000, 0, 0, 0, 2),
    ("separacion 3: 3000+2997+3 = 6000 justo",     [3000, 2997],        6000, 0, 0, 3, 1),
    ("separacion 3: 3000+2998+3 = 6001 no cabe",   [3000, 2998],        6000, 0, 0, 3, 2),
    ("zona muerta 230 + despunte 10: util 5760",   [2880, 2877],        6000, 10, 230, 3, 1),
    ("lo mismo con 1 mm mas",                      [2880, 2878],        6000, 10, 230, 3, 2),
    ("una pieza sola ocupa todo el util",          [5760],              6000, 10, 230, 3, 1),
    ("seis de 1000 en 6000 sin separacion",        [1000] * 6,          6000, 0, 0, 0, 1),
    ("seis de 1000 con separacion 3: 6015 > 6000", [1000] * 6,          6000, 0, 0, 3, 2),
    ("FFD da 3 aqui; el minimo es 2",              [3, 3, 2, 2, 2, 2],  7, 0, 0, 0, 2),
    # Limite conocido del metodo: aqui da 24 y el minimo real es 23 (lo demostro exacto_arcflow.py).
    # Se aceptan los dos valores para que una mejora futura no rompa la prueba.
    ("trabajo 289 de la familia medio (limite conocido)",
     [2694] * 4 + [2443] * 17 + [1868] * 4 + [1845] * 19 + [1611] * 19 + [445] * 7, 6000, 10, 230, 3, (23, 24)),
]
fallos = 0
for desc, largos, L, d, z, s, esperado in casos:
    w, C, util = transformar(largos, L, d, z, s)
    sol, cota, info = resolver(w, C, arranques=5)
    ok, msg = validar_pesos(w, C, sol)
    admitidos = esperado if isinstance(esperado, tuple) else (esperado,)
    bien = ok and len(sol) in admitidos and cota <= len(sol)
    fallos += (not bien)
    print(f"{'OK ' if bien else 'MAL'} {desc[:60]:<60} esperado {esperado} | barras {len(sol)} cota {cota} "
          f"{'(minimo demostrado)' if len(sol) == cota else '(no demostrado)'}")

try:                                            # una pieza que no cabe debe rechazarse, no colgar el calculo
    resolver([5764], 5763)
    print("MAL pieza que no cabe: no la rechazo"); fallos += 1
except ValueError as e:
    print("OK  pieza que no cabe ->", e)

random.seed(11)                                 # contra el branch and bound en trabajos chicos
malas = optimas = certificadas = n = 0
for _ in range(400):
    C = random.choice([150, 1000, 5763])
    w = [random.randint(C // 15, C * 3 // 5) for _ in range(random.randint(4, 13))]
    opt, probado = exacto_pequeno(w, C)
    if not probado:
        continue
    sol, cota, info = resolver(w, C, arranques=5)
    ok, msg = validar_pesos(w, C, sol)
    n += 1
    malas += (not ok) or not (cota <= opt <= len(sol))
    optimas += (len(sol) == opt); certificadas += (len(sol) == cota)
print(f"{'OK ' if malas == 0 else 'MAL'} {n} trabajos chicos contra branch and bound: cota <= minimo <= metodo en todos "
      f"(fallas: {malas}); metodo = minimo en {optimas}; minimo demostrado en {certificadas}")
print("\nFALLOS:", fallos + malas)
