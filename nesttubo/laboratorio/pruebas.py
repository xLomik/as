"""Pruebas de cordura del modelo, con respuestas que se pueden calcular a mano."""
from modelo import *

def resolver(largos, L, d, z, s):
    w, C, util = transformar(largos, L, d, z, s)
    largas = [l for l in largos if l > util + 1e-9]
    if largas:
        return None, f"pieza de {max(largas)} no cabe en el tramo util de {util}"
    sols = {"ffd": ffd(w, C), "bfd": bfd(w, C), "llenado": llenado_exacto(w, C)}
    for nombre, b in sols.items():
        ok, msg = validar(largos, b, L, d, z, s)
        assert ok, (nombre, msg)
    opt, probado = exacto_pequeno(w, C)
    return {k: len(v) for k, v in sols.items()} | {"optimo": opt, "L1": cota_l1(w, C), "L2": cota_l2(w, C)}, None

casos = [
    # (descripcion, largos, L, d, z, s, barras esperadas)
    ("dos mitades exactas, sin separacion",        [3000, 3000],        6000, 0, 0, 0, 1),
    ("dos mitades y 1 mm de mas",                  [3000, 3000, 1],     6000, 0, 0, 0, 2),
    ("separacion 3: 3000+2997+3 = 6000 justo",     [3000, 2997],        6000, 0, 0, 3, 1),
    ("separacion 3: 3000+2998+3 = 6001 no cabe",   [3000, 2998],        6000, 0, 0, 3, 2),
    ("zona muerta 230 + despunte 10: util 5760",   [2880, 2877],        6000, 10, 230, 3, 1),
    ("lo mismo con 1 mm mas",                      [2880, 2878],        6000, 10, 230, 3, 2),
    ("una pieza sola ocupa todo el util",          [5760],              6000, 10, 230, 3, 1),
    ("seis de 1000 en 6000 sin separacion",        [1000]*6,            6000, 0, 0, 0, 1),
    ("seis de 1000 con separacion 3: 6015 > 6000", [1000]*6,            6000, 0, 0, 3, 2),
    ("FFD falla aqui: optimo 2, FFD da 3",         [3, 3, 2, 2, 2, 2],  7, 0, 0, 0, 2),
]
fallos = 0
for desc, largos, L, d, z, s, esperado in casos:
    r, err = resolver(largos, L, d, z, s)
    ok = r["optimo"] == esperado
    fallos += (not ok)
    print(f"{'OK ' if ok else 'MAL'} {desc:<46} esperado {esperado} | {r}")

r, err = resolver([5761], 6000, 10, 230, 3)
print(f"{'OK ' if err else 'MAL'} pieza mas larga que el util -> {err}")
fallos += (err is None)

# cota L2 >= L1 siempre, y ninguna cota supera al optimo
import random
random.seed(7)
malas = 0
for _ in range(400):
    n = random.randint(3, 11)
    C = random.choice([100, 150, 1000])
    w = [random.randint(max(1, C // 12), C * 3 // 5) for _ in range(n)]
    opt, probado = exacto_pequeno(w, C)
    l1, l2 = cota_l1(w, C), cota_l2(w, C)
    if not (l1 <= l2 <= opt) or not probado:
        malas += 1
print(f"{'OK ' if malas == 0 else 'MAL'} 400 instancias aleatorias: L1 <= L2 <= optimo en todas (fallas: {malas})")
print("\nFALLOS:", fallos + malas)
