"""Genera el archivo de casos con el que se compara el nucleo en C++.

Uso:  python3 oraculo.py [semilla] [trabajos_por_familia]   ->  casos_oraculo.txt

Una linea por trabajo, todo en enteros en la unidad interna (mm, o decimas de
mm en la familia "decimal"):

    L despunte zona_muerta separacion ; largo:cantidad largo:cantidad ... ; cota metodo simples

  cota     cota inferior de la relajacion lineal (el nucleo en C++ debe dar LA MISMA)
  metodo   barras del metodo completo de simplex_propio.py (5 arranques)
  simples  barras de la mejor de FFD, BFD y llenado

Que exigirle al nucleo en C++ con cada linea:
  - su cota == cota                                  (siempre)
  - cota <= sus barras <= simples                    (siempre)
  - sus barras == metodo                             (casi siempre: los empates se rompen distinto,
                                                      asi que una diferencia de 1 en algun caso
                                                      suelto se mira a mano; muchas, es un error)
Las lineas que empiezan por # son comentarios.
"""
import random, sys, time
from collections import Counter
from modelo import transformar, ffd, bfd, llenado_exacto
from simplex_propio import resolver, validar_pesos

# (tipos min, tipos max, cantidad max, largo min, largo max, paso, escala, L, despunte, zona muerta, separacion, cuantos)
FAMILIAS = {
    "chico":      (2, 6, 8,   120, 3000, 1, 1, 6000, 10, 230, 3, 1.0),
    "largas":     (3, 10, 12, 1500, 3500, 1, 1, 6000, 10, 230, 3, 1.0),
    "medio":      (5, 15, 20, 120, 3000, 1, 1, 6000, 10, 230, 3, 1.0),
    "redondos":   (4, 12, 24, 100, 3000, 5, 1, 6000, 10, 230, 3, 1.0),
    "barra12":    (5, 15, 20, 300, 6000, 1, 1, 12000, 10, 230, 3, 0.5),
    "sin_margen": (5, 15, 20, 120, 3000, 1, 1, 6000, 0, 0, 0, 0.5),
    "decimal":    (4, 10, 16, 1200, 30000, 5, 10, 6000, 10, 230, 3, 0.25),
    "grande":     (10, 30, 40, 120, 3000, 1, 1, 6000, 10, 230, 3, 0.1),
    "variado":    (40, 120, 3, 120, 3000, 1, 1, 6000, 10, 230, 3, 0.1),    # muchas piezas distintas, pocas de cada una
}
semilla = int(sys.argv[1]) if len(sys.argv) > 1 else 7
base = int(sys.argv[2]) if len(sys.argv) > 2 else 200
random.seed(semilla)
t0 = time.time()
lineas, total, cert = [], 0, 0
for fam, (tmin, tmax, cmax, lmin, lmax, paso, esc, L, D, Z, S, parte) in FAMILIAS.items():
    lineas.append(f"# familia {fam}")
    for _ in range(max(1, int(base * parte))):
        largos = []
        for _ in range(random.randint(tmin, tmax)):
            l = random.randrange(lmin, lmax + 1, paso)
            largos += [l] * random.randint(1, cmax)
        w, C, util = transformar(largos, L * esc, D * esc, Z * esc, S * esc)
        sol, cota, info = resolver(w, C, clavar=True, arranques=5, semilla=semilla)
        ok, msg = validar_pesos(w, C, sol); assert ok, msg
        simples = min(len(ffd(w, C)), len(bfd(w, C)), len(llenado_exacto(w, C)))
        assert cota <= len(sol) <= simples
        pares = " ".join(f"{l}:{n}" for l, n in sorted(Counter(largos).items(), reverse=True))
        lineas.append(f"{L*esc} {D*esc} {Z*esc} {S*esc} ; {pares} ; {cota} {len(sol)} {simples}")
        total += 1; cert += (len(sol) == cota)
open("casos_oraculo.txt", "w").write(
    f"# casos del oraculo: semilla {semilla}, {total} trabajos, minimo demostrado en {cert}\n"
    "# L despunte zona_muerta separacion ; largo:cantidad ... ; cota metodo simples\n" + "\n".join(lineas) + "\n")
print(f"casos_oraculo.txt: {total} trabajos, minimo demostrado en {cert}, {time.time()-t0:.0f} s")
