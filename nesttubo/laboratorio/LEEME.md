# Laboratorio de corte 1D de tubo (solo largos)

Prototipo en Python hecho el 2026-10-06 para decidir el algoritmo del programa antes de escribirlo. **No es el programa.** Sirve para dos cosas:

1. **Referencia para portar a C++**: `simplex_propio.py` es el método completo sin ningún solver.
2. **Oráculo**: `casos_oraculo.txt` trae 1.090 trabajos con la cota y las barras esperadas, para comparar el núcleo en C++.

Todo lo que aquí dice "medido" se ejecutó ese día en un sandbox Ubuntu 24.04 con Python 3.11.15, numpy 2.4.4 y scipy 1.17.1. Los trabajos son **sintéticos** (largos al azar con forma de pedido de taller); no son pedidos reales de Camilo.

## Cómo correrlo

```bash
python3 pruebas.py                 # modelo, cotas y heurísticas simples. Sin dependencias. Debe terminar en "FALLOS: 0"
pip install numpy                  # (en Ubuntu: añadir --break-system-packages)
python3 pruebas_propio.py          # el método sin solver. Debe terminar en "FALLOS: 0"
python3 oraculo.py 7 200           # regenera casos_oraculo.txt (240 s aquí)
pip install scipy openpyxl         # solo para las mediciones y los árbitros
python3 medicion_propio.py medio 300 2026     # una familia de la medición -> propio_medio.json
python3 exacto_arcflow.py medio 300 2026      # árbitro exacto de los trabajos sin certificar
python3 resumen.py resultados                 # las tablas de este LEEME, armadas desde los json
```

## Archivos

| Archivo | Qué es | Necesita |
|---|---|---|
| `modelo.py` | Modelo de la barra, transformación a bin packing, cotas L1 y L2, FFD, BFD, llenado por subset-sum, branch and bound para trabajos chicos y un validador | nada |
| `pruebas.py` | 10 casos que se calculan a mano + 400 trabajos al azar contra el branch and bound | nada |
| **`simplex_propio.py`** | **El método propuesto, sin solver**: mochila acotada, simplex revisado con generación de columnas, cantera de patrones, anticiclado, cota de Farley, redondeo residual, clavado, reintentos y validador | numpy |
| `pruebas_propio.py` | Pruebas del método: los casos a mano, el límite conocido y 400 trabajos chicos contra el branch and bound | numpy |
| `oraculo.py`, `casos_oraculo.txt` | Generador y archivo de casos para comparar el port a C++ (el formato y qué exigir están en la cabecera de `oraculo.py`) | numpy |
| `perfil_variado.py` | Cuántas vueltas y mochilas cuesta el LP según el número de largos distintos | numpy |
| `exacto.py` | Referencia con HiGHS: relajación lineal por generación de columnas + programa entero sobre los patrones generados | scipy |
| `residual.py` | Primera versión del redondeo residual, con el LP de HiGHS | scipy |
| `exacto_arcflow.py` | Árbitro exacto (flujo en arcos de Valério de Carvalho, entero con HiGHS) para los trabajos donde nada iguala la cota | scipy |
| `medicion_final.py` | Primera medición, con HiGHS (8 familias) | scipy |
| `medicion_propio.py`, `resumen.py` | La medición del método sin solver, con la comparación de su cota con la de HiGHS, y el script que arma las tablas | scipy |
| `validar_exacto.py`, `medir_bpplib.py`, `medir_residual_bpplib.py`, `medir_propio_bpplib.py` | Contraste con instancias públicas de óptimo conocido | scipy, openpyxl, BPPLIB |
| `resultados/` | Las salidas de todo lo anterior, tal como salieron | |
| `recetas/` | PDF y DXF escritos a mano, sin librerías, con su comprobación | ezdxf y matplotlib solo para mirar el DXF |
| `ventana/` | Ventana Win32 mínima, compilada y ejecutada bajo wine, con sus scripts y capturas | mingw-w64, wine |

## El modelo

- `util = L − despunte − zona_muerta`. Un grupo de n piezas cabe en una barra si `suma(largos) + (n−1)·separación ≤ util`.
- Sumando la separación a ambos lados: pesos `w = largo + separación`, capacidad `C = util + separación`. Es un bin packing clásico.
- Todo en enteros (mm, o décimas de mm si hay decimales).

## El método (`simplex_propio.py`)

1. **Cota inferior.** Relajación lineal del modelo de patrones de Gilmore y Gomory: `min Σ x_p` con `Σ_p a_jp·x_p ≥ demanda_j`. Se resuelve con un simplex revisado que guarda la inversa de la base (m×m, m = largos distintos). Cada posición de la base es un patrón (cuesta 1) o una holgura de exceso (cuesta 0). La base inicial es un patrón homogéneo por largo. Los patrones nuevos los genera una mochila acotada entera (programación dinámica sobre la capacidad, con la demanda descompuesta en paquetes 1, 2, 4…). La cota es `ceil(cota de Farley − 1e-6)`, con `Farley = (demanda · precios) / max(1, valor del mejor patrón)`: vale en cualquier vuelta, aunque el LP no haya convergido.
2. **Residual.** Cada patrón de la base se usa ⌊x⌋ veces, recortado a lo que falta; se repite el LP sobre lo pendiente.
3. **Clavado.** Cuando el LP ya no deja ninguna parte entera, se usa una vez el patrón con mayor x (empate: el que más llena la barra) y se vuelve a resolver.
4. **Cierre.** En cada paso se prueba terminar lo pendiente con la mejor de FFD, BFD y llenado, y se guarda el mejor total. Si iguala la cota, se para: es el mínimo.
5. **Reintentos.** Si no igualó la cota, se repite todo con los largos en otro orden (otro vértice del LP), hasta 5 veces.

Tres piezas del LP que se añadieron después de verlo fallar, y que el port debe llevar:

- **Cantera de patrones.** Todos los patrones generados se guardan, y en cada vuelta se busca primero ahí uno que mejore (un producto matriz-vector) antes de llamar a la mochila. Sin cantera, un patrón que sale de la base se olvida y hay que regenerarlo: con 79 largos distintos el LP llamó 2.521 veces a la mochila; con cantera, 377. La cantera pasa de un LP al siguiente dentro de la misma pasada.
- **Anticiclado.** El LP es muy degenerado. Sin protección, un trabajo de 75 largos distintos dio 200.000 vueltas sin converger. El remedio es perturbar la demanda (cada largo recibe un extra distinto del orden de 1e-6) solo para la prueba de la razón; los precios y la cota no cambian, y la solución final se calcula con la demanda real.
- **Semilla.** Los patrones de las tres heurísticas simples entran a la cantera antes del primer LP. Con 79 largos distintos baja de 377 a 180 mochilas.

Cuánto cuesta el LP según los largos distintos (`resultados/perfil_variado_final.log`; 1 a 3 piezas de cada largo; tiempos de este Python con las otras mediciones corriendo a la vez):

| Largos distintos | Piezas | LP solo: vueltas / mochilas | Método, una pasada |
|---|---|---|---|
| 20 | 44 | 185 / 61 | 28 mochilas, menos de 0,1 s |
| 40 | 88 | 1.127 / 165 | 75 mochilas, 0,3 s |
| 79 | 166 | 5.222 / 377 | 180 mochilas, 1,1 s |
| 117 | 247 | 14.393 / 644 | 33 LP, 570 mochilas, 9,7 s (76 barras = cota; las simples daban 77) |

Notas para el port:
- La mochila es lo caro: `paquetes × capacidad` operaciones por llamada. En mm la capacidad ronda 5.763; en décimas, 57.630. Conviene trabajar en mm cuando todos los datos son enteros.
- Una vuelta sin mochila cuesta m² operaciones. Con muchos largos distintos hay miles de vueltas: por eso el tope de tiempo y la cancelación.
- El error de la inversa tras miles de pivotes fue despreciable en lo medido (máximo 1,5e-12; el LP más largo dio 17.454 vueltas). No se midió ese error con más de 120 largos distintos; ahí convendría recalcular la inversa cada cierto número de pivotes.
- `random.shuffle` de Python no se puede reproducir en C++: el port romperá los empates distinto. Se compara cota y número de barras, no la distribución.
- Una pieza con `w > C` hace dividir por cero en la base inicial: hay que apartarla antes (`resolver` lanza `ValueError`).

## Qué se midió

### 1. Las heurísticas simples no bastan

Contra óptimos publicados (BPPLIB), `resultados/salida_bpplib_simples.txt`:

| Familia | Instancias | Piezas | Método | = óptimo | +1 | +2 o más | Peor |
|---|---|---|---|---|---|---|---|
| Falkenauer U | 80 | 120–1000 | FFD | 6 | 26 | 48 | +7 |
| | | | BFD | 6 | 26 | 48 | +7 |
| | | | llenado | 61 | 17 | 2 | +2 |
| Falkenauer T | 80 | 60–501 | FFD | 0 | 0 | 80 | +24 |
| | | | BFD | 0 | 0 | 80 | +24 |
| | | | llenado | 0 | 56 | 24 | +4 |
| Hard28 | 28 | 160–200 | FFD | 5 | 23 | 0 | +1 |
| | | | BFD | 5 | 23 | 0 | +1 |
| | | | llenado | 3 | 14 | 11 | +3 |

La cota por volumen (L1) y la L2 de Martello y Toth coinciden con el óptimo en 79/80, 80/80 y 23/28.

### 2. El método propuesto en trabajos con forma de taller

`medicion_propio.py`, semilla 2026 → `resultados/propio_*.json`. Parámetros: barra 6000 (12000 en `barra12`), despunte 10, zona muerta 230, separación 3. "Brecha" es barras menos cota; brecha 0 significa **mínimo demostrado**. "Simples" es la mejor de FFD, BFD y llenado. Los tiempos son de este Python con numpy, con dos mediciones corriendo a la vez.

| Familia | Trabajos | Piezas (máx.) | Largos distintos (máx.) | Simples: brecha 0 / +1 / +2 o más (peor) | Método: brecha 0 / +1 / +2 o más | Barras: cota · simples · método | Método, s por trabajo: media (máx.) |
|---|---|---|---|---|---|---|---|
| chico | 600 | 41 | 6 | 593 / 7 / 0 | 600 / 0 / 0 | 3.447 · 3.454 · 3.447 | 0,01 (0,02) |
| largas (1500–3500 mm) | 400 | 82 | 10 | 392 / 8 / 0 | 400 / 0 / 0 | 8.367 · 8.375 · 8.367 | 0,01 (0,07) |
| medio | 300 | 208 | 15 | 197 / 89 / 14 (+4) | 298 / 2 / 0 | 8.766 · 8.886 · 8.768 | 0,10 (0,64) |
| redondos (múltiplos de 5 mm) | 300 | 203 | 12 | 211 / 81 / 8 (+2) | 300 / 0 / 0 | 8.429 · 8.526 · 8.429 | 0,09 (0,34) |
| barra12 (barra de 12 m) | 200 | 192 | 15 | 142 / 54 / 4 (+2) | 200 / 0 / 0 | 5.651 · 5.713 · 5.651 | 0,20 (0,70) |
| sin_margen (sin despunte, zona muerta ni separación) | 200 | 208 | 15 | 134 / 58 / 8 (+4) | 200 / 0 / 0 | 5.621 · 5.697 · 5.621 | 0,10 (0,35) |
| decimal (décimas de mm) | 120 | 121 | 10 | 107 / 13 / 0 | 120 / 0 / 0 | 2.035 · 2.048 · 2.035 | 0,32 (1,07) |
| grande | 60 | 647 | 30 | 13 / 18 / 29 (+6) | 60 / 0 / 0 | 6.662 · 6.757 · 6.662 | 1,16 (3,85) |
| variado (40–120 largos distintos, 1–3 de cada uno) | 40 | 242 | 117 | 27 / 13 / 0 | 40 / 0 / 0 | 1.734 · 1.747 · 1.734 | 1,27 (9,64) |
| **Total** | **2.220** | | | **1.816 / 341 / 63 (+6)** | **2.218 / 2 / 0** | **50.712 · 51.203 · 50.714** | |

- Las heurísticas simples igualan la cota en el 81,8 % de los trabajos y piden 491 barras de más en total (0,97 %). En la familia `grande` solo aciertan 13 de 60.
- El método da el mínimo demostrado en 2.218 de 2.220. Solo hicieron falta reintentos en 3 trabajos: uno se resolvió al segundo, y dos agotaron los cinco sin igualar la cota.
- Sin clavado ni reintentos ("residual", una pasada): 2.209 de 2.220.
- La cota del simplex propio fue igual a la de HiGHS en los 2.220 trabajos, y el LP convergió en todos.
- Sensibilidad al orden de los largos: una sola pasada con 2 órdenes al azar, quedándose con el peor, dio 2.217 de 2.220. El resultado depende poco del vértice del LP y los reintentos cubren la diferencia.
- Error máximo de la inversa de la base: 1,5e-12. El LP más largo dio 17.454 vueltas. Las dos cifras son de la familia `variado`.

### 3. Los trabajos sin certificar, resueltos de forma exacta

`exacto_arcflow.py` → `resultados/arbitro_*.json`. Las soluciones exactas pasaron el validador.

| Familia | Trabajo | Largos × cantidad | Cota | Método | Exacto | Veredicto |
|---|---|---|---|---|---|---|
| medio | 77 | 2610×18, 2564×19, 2125×9, 1474×18, 1419×8, 1217×6, 484×7, 444×15 | 30 | 31 | 31 | era el mínimo: la cota era floja |
| medio | 289 | 2694×4, 2443×17, 1868×4, 1845×19, 1611×19, 445×7 | 23 | 24 | 23 | sobraba 1 barra |

Cuando el método no iguala la cota, el programa tiene que decirlo ("no demostrado: podrían sobrar hasta N barras", con N = barras − cota), porque desde dentro no se distingue si la cota era floja o si de verdad sobra una barra.

### 4. El método contra instancias públicas hechas para ser difíciles

`medir_propio_bpplib.py`, hasta 5 reintentos → `resultados/bpp_propio_*.txt`. "Mínimo demostrado" cuenta las instancias donde el método iguala su propia cota.

| Instancias | Cuántas | Método: = óptimo | +1 | +2 o más | Mínimo demostrado | s por instancia (máx.) |
|---|---|---|---|---|---|---|
| Hard28 | 28 | 19 | 9 | 0 | 14 | 10,8 (31,2) |
| t120 | 20 | 18 | 2 | 0 | 18 | 1,6 (3,7) |
| t60 | 20 | 19 | 1 | 0 | 19 | 0,2 (1,0) |
| u120 | 20 | 20 | 0 | 0 | 20 | 0,1 (0,1) |
| u250 | 20 | 20 | 0 | 0 | 20 | 0,1 (0,2) |

- La primera versión del residual (`residual.py`: LP de HiGHS, sin clavado ni reintentos) acertaba 18/20 en u120, 18/20 en u250, 18/20 en t60, 2/20 en t120 y 5/28 en Hard28 (`resultados/bpp_residual_*.txt`).
- En Hard28 hay instancias cuyo óptimo está 1 barra por encima de la cota del LP (5 de las 28): ahí ningún método puede "demostrar" el mínimo con esta cota.
- Los pedidos de taller no se parecen a estas instancias (los "triplets" de Falkenauer están construidos para que cada barra lleve exactamente tres piezas sin sobrante). Sirven para saber cuánto puede fallar el método: en lo medido, nunca más de 1 barra.

### 5. La referencia exacta también se validó

`validar_exacto.py` → `resultados/salida_validar_exacto.txt`: 46 instancias de BPPLIB, 0 inconsistencias (cota ≤ óptimo publicado ≤ solución entera en todas), ninguna sin converger, y la solución entera coincide con el óptimo publicado en 32. Además, 150 trabajos chicos contra el branch and bound: 0 inconsistencias.

La primera versión de `exacto.py` cortaba la generación de columnas a las 400 vueltas sin avisar y devolvía una "cota" mayor que el óptimo publicado en Hard28 (69 contra 67). Se corrigió con la cota de Farley y un indicador de convergencia. Toda cota nueva se contrasta contra algo independiente antes de usarla.

### 6. Historia: lo que cambió durante el día

- La primera versión de `simplex_propio.py` usaba la forma `=` del LP, sin cantera ni anticiclado. En las 8 familias con pocos largos distintos (hasta 30) dio mínimo demostrado en 2.178 de 2.180 trabajos, igual que la versión final, pero al probar la familia `variado` la medición de 6 trabajos no terminó en 10 minutos. De ahí salieron la cantera y el anticiclado. Lección: medir solo familias parecidas escondió el problema.
- `medicion_final.py` (con HiGHS) queda como registro: `resultados/resultados_*.json`.

### 7. Recetas de salida y ventana

- `recetas/pdf_minimo.py`: PDF de 2 páginas escrito byte a byte (Helvetica y Helvetica-Bold sin incrustar, `WinAnsiEncoding`, rectángulos y texto). `pdfinfo` lo abre, `pdftotext` devuelve el texto con tildes y `pdftoppm` lo pinta bien (`recetas/pdf.png`).
- `recetas/dxf_minimo.py`: DXF ASCII R12 con solo `LINE` y `TEXT`. `ezdxf` 1.4.4 lo lee como AC1009, 0 errores de auditoría, y lo pinta (`recetas/dxf.png`). Ojo: una barra de 6000 × 100 mm a escala real es una tira de 60:1; hay que enseñársela a Camilo antes de dar el formato por bueno. La convención de ángulo de la receta (90 = recto, medido respecto al eje) es un supuesto.
- `ventana/compilar.sh` compila tres pruebas: `prueba.exe` (ListView y botón; 18.944 bytes; solo DLL de Windows), `acentos.exe` (los literales `L"..."` con tildes funcionan si el fuente está en UTF-8) e `hilo.exe` (`std::thread` y `std::mutex` con `-static`). `ventana/ejecutar_y_capturar.sh` las abre bajo wine 9.0 + Xvfb y deja la captura (`captura_tabla.png`, `captura_tildes.png`).
- El prefijo de wine hay que crearlo una vez con `wineboot --init` y esperar con `wineserver -w` (45 s, unos 700 MB). La primera versión del script no lo hacía: la captura mostraba el aviso "The Wine configuration is being updated" y, al matar wine a medias, el prefijo quedó roto ("could not load kernel32.dll"). Se arregla borrando la carpeta del prefijo.
- `prueba.exe` va incluido: abrirlo en el equipo de Camilo es la forma más barata de confirmar que la ruta de entrega funciona en su Windows.

## Lo que NO está probado

- El port a C++: no existe todavía. Ni su corrección ni su velocidad.
- Pedidos reales de Camilo: no se ha visto ninguno. Tamaños, número de perfiles y largos típicos son supuestos.
- Los valores de despunte (10), separación (3) y zona muerta (230) son de laboratorio. 230 mm es el ejemplo que da el manual de TubesT para una máquina de dos mandriles; el valor real es el de la máquina del proveedor.
- Nada se ha ejecutado en Windows real: solo bajo wine.
- Que Excel abra el `.xlsx` escrito a mano no se probó aquí (la receta viene de la skill `apps-windows-cpp`, usada en otro programa de Camilo).
- Que el visor de DXF que use Camilo abra un R12 sin cabecera.
- Trabajos con más de 120 largos distintos.

## Ideas sin medir

- Para los casos sin certificar: una búsqueda exacta acotada. El árbitro resolvió los que aparecieron en décimas de segundo porque tenían pocos largos distintos.
- Atajo: si las heurísticas simples ya igualan la cota L2, no hace falta el LP.
- Estabilizar los precios del LP para bajar el número de vueltas con muchos largos distintos.
- Fase 2 (encajar cortes en ángulo): Lewis y Bonnet (2025), "Exact algorithms in bar nesting", https://rhydlewis.eu/papers/CAIE2025.pdf; y Garraffa y otros (2016), "The one-dimensional cutting stock problem with sequence-dependent cut losses", DOI 10.1111/itor.12095.

## BPPLIB (no va incluido)

Pesa unos 85 MB y su licencia es CC BY-NC-ND 4.0: sirve para medir, no para copiar su código al programa. Los scripts lo buscan en `../bpplib`, al lado de esta carpeta:

```bash
git clone --depth 1 https://github.com/mdelorme2/BPPLIB.git bpplib     # fuera de laboratorio/, a su lado
apt-get install -y libarchive-tools                                     # bsdtar abre los .rar
mkdir -p bpplib/Instances/x
bsdtar -xf bpplib/Instances/Benchmarks/1_Falkenauer.rar -C bpplib/Instances/x
bsdtar -xf bpplib/Instances/Benchmarks/5_Hard28.rar -C bpplib/Instances/x
```

Los óptimos están en `bpplib/Instances/Solutions/Solutions.xlsx`.
