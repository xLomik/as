# NestTubo — cuántas barras de tubo enviar a corte láser

Documento de arranque (2026-10-06). Resume la conversación en la que Camilo definió el proyecto, lo que se investigó y lo que se midió ese día.
Etiquetas: **[C]** lo dijo Camilo · **[S]** supuesto sin confirmar · **[M]** ejecutado y medido ese día.
Mantén este archivo al día: cuando un [S] se confirme, corrígelo aquí, y borra lo que deje de ser cierto.

## Qué es
- Programa de escritorio para Windows. Camilo digita las piezas de tubo de un pedido y el programa calcula **cuántas barras de cada perfil hay que enviar al proveedor externo de corte láser**, con el plan de qué piezas salen de cada barra.
- No es un programa de corte. [C] "el nesteador es con el objetivo de saber cuanta tuberia para corte laser se debe usar, no para cortar, solo es para hacer el calculo de la tuberia que se debe enviar al provedor de corte laser".
- El número de barras por perfil es el dato principal en pantalla y en cada archivo de salida.
- "NestTubo" es un nombre provisional [S].

## Requisitos confirmados [C]
- Entrada: lista digitada a mano. Por pieza: perfil, largo, ángulo de cada extremo y cantidad.
- Cálculo: solo largos. Cada pieza es un tramo recto de su largo; los ángulos no se encajan entre sí y no hay corte común.
- Salidas: plan de corte en PDF y en Excel, y un DXF por barra.
- El DXF es un dibujo, no un archivo de corte: "Solo se ve la cara mas ancha, es algo visual, nada mas".
- Material: siempre barras completas. Sin inventario de sobrantes.
- La máquina del proveedor es un láser de tubo.
- Los datos se guardan en una carpeta que él elige. Se le propuso que pueda ser de red y que, si no responde, se guarde local y se sincronice después.
- Entrega: un `.exe` nativo en C++, compilado y probado en la nube. Él no instala nada.

## Fuera de alcance (no lo construyas si no lo pide)
- Importar DXF, STEP o Excel: los descartó como entrada. Su otro programa, TuboDXF, ya saca DXF y lista de piezas, y no pidió conectarlos.
- Encajar cortes en ángulo o compartir cortes entre piezas (sería una fase 2; ver Referencias).
- Inventario de retales, código de máquina, instalador, base de datos, servicio web.
- Pegar filas copiadas de Excel y multiplicar cantidades por número de conjuntos: son ideas de quien escribió este documento, Camilo no las pidió. Ofrécelas después de la primera versión.

## Por confirmar [S]
Pregunta los puntos 1 a 4 en una sola ronda antes de diseñar las salidas. No preguntes nada de "Requisitos confirmados".
1. Parámetros de la máquina del proveedor: largo de barra (provisional 6000 mm, editable por perfil); tramo final que el mandril no deja cortar, o "zona muerta" (provisional 230 mm: es el ejemplo del manual de TubesT para una máquina de dos mandriles; con tres mandriles puede ser 0, y otra fuente habla de 254 a 508 mm); separación entre piezas y despunte inicial (provisionales 3 y 10 mm: números de laboratorio, no del proveedor).
2. Qué mide "largo" en una pieza con extremos en ángulo (provisional: de punta a punta, que es lo que ocupa en la barra) y cómo expresa el ángulo (provisional: 90 = corte recto, 45 = inglete, visto sobre la cara más ancha).
3. Si el proveedor corta siguiendo este plan o nestea por su cuenta. En el segundo caso su programa puede necesitar más barras que el mínimo: ofrecer un margen de seguridad configurable.
4. Con qué programa abre el DXF (decide versión y entidades).
- Catálogo de perfiles (provisional): nombre libre + medida de la cara más ancha + largo de barra, guardado en la carpeta de datos.
- DXF repetidos (provisional): un archivo por distribución distinta, con "xN" en el nombre.
- Al digitar, aceptar coma o punto como separador decimal (provisional).
- Formato del PDF y del Excel: no lo preguntes en abstracto; enséñale pronto un borrador real y ajusta sobre eso.

## Entorno y entrega
- Equipo de Camilo [C]: el Windows del trabajo, de 64 bits (ahí compiló y corrió un binario x86_64). La red bloquea descargas de software (rustup.rs, python.org); github.com sí abre. No tiene Python ni otro equipo. Ya usa otro `.exe` suyo hecho en C++ Win32 (un cronómetro de nesteo).
- Se construye en un sandbox Linux en la nube y se le envía el resultado por el chat. Límite observado: 30 MB por archivo.
- Red del sandbox [M]: funcionan apt (archive.ubuntu.com), PyPI y `git clone` de GitHub público. Bloqueados por política del proxy (403; no reintentar ni rodear): static.rust-lang.org, python.org, nuget.org, zenodo.org. Por eso no C# y nada que él deba instalar.
- Si la skill `apps-windows-cpp` está disponible, cárgala antes de escribir código: es de Camilo y trae las recetas de Win32, Excel a mano y carpetas de red. Lo imprescindible está repetido aquí.

## Construir y probar [M]
```bash
apt-get update && apt-get install -y --no-install-recommends g++-mingw-w64-x86-64 wine64 xdotool   # Ubuntu 24.04: GCC 13, wine 9.0 (con sudo si hace falta)
# nucleo de calculo: nativo y con sanitizadores, para las pruebas
g++ -std=c++17 -O1 -g -fsanitize=address,undefined nucleo.cpp pruebas.cpp -o pruebas && ./pruebas
# programa para Windows
x86_64-w64-mingw32-windres app.rc -O coff -o app.res
x86_64-w64-mingw32-g++ -std=c++17 -municode -O2 main.cpp nucleo.cpp app.res -o NestTubo.exe -mwindows \
  -static -static-libgcc -static-libstdc++ -lcomctl32 -lshlwapi -lole32 -loleaut32 -luuid -lgdi32 -luser32 -lshell32 -ladvapi32
x86_64-w64-mingw32-strip NestTubo.exe
x86_64-w64-mingw32-objdump -p NestTubo.exe | grep "DLL Name"      # solo deben salir DLL de Windows
# ejecutarlo de verdad y mirar la captura (wine no queda en el PATH)
export DISPLAY=:99 WINEPREFIX=$HOME/.winep WINEDEBUG=-all
pidof Xvfb >/dev/null || (setsid nohup Xvfb :99 -screen 0 1400x1000x24 >/dev/null 2>&1 &)
[ -f $WINEPREFIX/system.reg ] || { WINEDLLOVERRIDES="mscoree,mshtml=" /usr/lib/wine/wine64 wineboot --init; /usr/lib/wine/wineserver -w; }
(setsid nohup /usr/lib/wine/wine64 ./NestTubo.exe >/dev/null 2>&1 &); sleep 8; import -window root captura.png
/usr/lib/wine/wineserver -k
```
- El prefijo de wine se crea una sola vez con `wineboot` y hay que esperarlo (45 s, unos 700 MB). Sin eso la captura muestra el aviso de wine, y matar wine a medias deja el prefijo roto ("could not load kernel32.dll"): se borra la carpeta y se repite.
- `app.rc` incrusta un manifiesto (comctl32 v6 + `dpiAware`); sin él los controles salen con aspecto de Windows 95.
- Sin gestor de ventanas no hay barra de título ni foco de teclado: con `xdotool` haz clic en el campo antes de escribir.
- Los literales `L"..."` con tildes funcionan si el fuente está en UTF-8. `std::thread` y `std::mutex` funcionan con `-static`.

## Modelo (un problema independiente por perfil)
- `util = largo_barra − despunte − zona_muerta`. Un grupo de n piezas cabe en una barra si `suma(largos) + (n−1)·separación ≤ util`.
- Sumando la separación a ambos lados queda un bin packing clásico: pesos `w = largo + separación`, capacidad `C = util + separación`.
- Todo en enteros: milímetros si los datos son enteros; décimas de milímetro si alguno trae decimal.
- Una pieza con `largo > util` no cabe en ninguna barra: avisar con su nombre y dejarla fuera del cálculo.
- Con los mismos parámetros, encajar ángulos solo puede pedir igual o menos barras que "solo largos". El riesgo real es el contrario: el punto 3 de "Por confirmar".

## Algoritmo
Siempre hay una respuesta válida en milisegundos (paso 1); los pasos 2 y 3 la certifican o la mejoran. La referencia es `laboratorio/simplex_propio.py`.
1. Tres heurísticas simples y quedarse con la mejor: FFD, BFD y "llenado" (para cada barra, fijar la pieza pendiente más larga y completarla con el subconjunto que deja menos hueco, por programación dinámica).
2. Cota inferior: relajación lineal del modelo de patrones (Gilmore y Gomory), `min Σx` con `Σ a·x ≥ demanda`, por generación de columnas. Simplex revisado propio con la inversa m×m explícita (m = largos distintos) y una mochila acotada entera que genera el patrón que entra. `cota = ceil(cota de Farley − 1e-6)`: es válida aunque el LP no llegue a converger.
3. Mejora: usar cada patrón del LP ⌊x⌋ veces y repetir el LP sobre lo que falta; cuando ya no queda parte entera, fijar una vez el patrón más usado y seguir; en cada paso cerrar lo pendiente con el paso 1 y guardar el mejor total; si no iguala la cota, repetir con los largos en otro orden (hasta 5 veces).
4. Si `barras == cota`, decir "mínimo demostrado". Si no, decir en pantalla y en el informe "no demostrado: podrían sobrar hasta N barras", con `N = barras − cota` (en lo medido con forma de taller, N nunca pasó de 1).

Tres piezas del paso 2 que se añadieron después de verlo fallar [M]; el port debe llevarlas:
- Guardar todos los patrones generados y buscar primero ahí uno que mejore, antes de llamar a la mochila (que es lo caro). Sin eso, con 79 largos distintos el LP llamó 2.521 veces a la mochila; con eso, 377.
- Perturbar la demanda (un extra distinto por largo, del orden de 1e-6) solo para decidir quién sale de la base. Sin eso el simplex cicló: 200.000 vueltas sin converger en un pedido de 75 largos distintos.
- Sembrar el LP con los patrones de las heurísticas del paso 1 (de 377 a 180 mochilas en el mismo caso).

Qué respalda esta elección [M] (trabajos sintéticos con forma de taller; no son pedidos reales de Camilo):
- 2.220 pedidos en 9 familias: de 3 a 647 piezas y hasta 30 largos distintos, más una familia con 40 a 117 largos distintos.
- Las heurísticas del paso 1, solas, igualan la cota en el 81,8 % de los pedidos y piden hasta 6 barras de más; en la familia de pedidos grandes (196 a 647 piezas) solo aciertan 13 de 60.
- El método completo da el mínimo demostrado en 2.218 de 2.220. En los otros 2 quedó 1 barra sobre la cota; un árbitro exacto mostró que en uno esa barra sobraba y en el otro era el mínimo real.
- La cota del simplex propio coincide con la de HiGHS en los 2.220, y el LP convergió en todos.
- En instancias públicas construidas para ser difíciles (BPPLIB) nunca quedó a más de 1 barra del óptimo publicado.
- Tiempos del prototipo (Python con numpy), por pedido: de 0,01 a 0,3 s de media con hasta 15 largos distintos; 1,2 s de media con hasta 30 largos y 647 piezas; 1,3 s de media y 9,6 s de máximo con 40 a 117 largos. En C++ no se ha medido: de ahí el tope de tiempo.

Reglas para el código:
- El cálculo vive en un `nucleo.cpp` sin nada de Win32, para compilarlo nativo y probarlo sin wine. La ventana solo lo llama.
- Determinista: la misma lista da siempre el mismo plan (generador propio con semilla fija para los reintentos).
- Corre en un hilo de trabajo con cancelación y tope de tiempo; al vencer entrega lo mejor que tenga.
- Toda solución pasa por un validador escrito aparte antes de mostrarse o exportarse: mismas piezas que las pedidas y ninguna barra pasada.
- El port se compara con `laboratorio/casos_oraculo.txt` (1.090 trabajos con su cota y sus barras): misma cota en todos los trabajos y mismas barras salvo casos sueltos que se miran a mano. No se comparan distribuciones, porque los empates se rompen distinto.

## Salidas
- Resumen (pantalla, primera hoja del Excel y primera página del PDF): perfil · largo de barra · **barras a enviar** · piezas · aprovechamiento · mínimo demostrado sí/no.
- Plan por barra: barras iguales agrupadas ("× N"), piezas en orden con su inicio y su fin, y sobrante.
- Excel: `.xlsx` escrito a mano (ZIP sin comprimir + XML, texto con `t="inlineStr"`); la receta está en la skill. [S] No se probó aquí.
- PDF: escrito a mano con Helvetica estándar y `WinAnsiEncoding` para las tildes. [M] Receta en `laboratorio/recetas/pdf_minimo.py`; la abren `pdftotext` y `pdftoppm`.
- DXF: ASCII R12 con solo `LINE` y `TEXT`, en mm, capas BARRA, PIEZAS, TEXTO y ZONA_MUERTA, sin tildes. [M] Receta en `laboratorio/recetas/dxf_minimo.py`; la lee `ezdxf`. Una barra de 6000 × 100 mm a escala real es una tira de 60:1: enséñasela antes de darla por buena.

## Antes de entregar algo
1. Pruebas del núcleo en verde: los casos calculables a mano de `laboratorio/pruebas_propio.py` y la comparación con el oráculo.
2. El `.exe` ejecutado bajo wine y las capturas miradas; compilar no cuenta como probar.
3. Cada PDF, Excel y DXF generado por el propio `.exe`, abierto con un lector independiente y mirado como imagen.
4. ZIP con el `.exe`, el código fuente, `build.bat` y un LEEME; listar su contenido y su tamaño (menos de 30 MB).
5. Decir qué se probó y qué no. wine no es Windows: lo que solo se vio en wine se dice así.

## Trampas ya pagadas
- El proyecto anterior (NestPro: motor en Rust + interfaz en Python) necesitó cuatro rondas solo para instalarse en su equipo: sin Rust, descargas bloqueadas, `dlltool.exe` ausente en el toolchain GNU y el "python" falso de la Microsoft Store. Por eso aquí todo va en un solo `.exe`. Prueba la ruta de entrega con un programa mínimo antes de escribir el resto.
- Prueba los scripts tal como los vas a entregar o documentar: el de captura falló la primera vez que se corrió desde cero, por el prefijo de wine.
- Cuando algo falle bajo wine, aísla si falla el programa o falla wine: pasó tres veces que era wine o el arnés de prueba.
- Nunca `pkill -f` ni `pgrep -f` con un patrón que aparezca en el propio comando: mata el shell (código 144; pasó dos veces). Guarda el PID al lanzar (`echo $! > x.pid`) y mata por número. wine se cierra con `wineserver -k`.
- Un comando en primer plano se corta a los 10 minutos: lo largo va con `setsid nohup ... &` y se consulta después.
- Antes de enviar un ZIP, lista su contenido: una vez salió de 298 MB por carpetas de compilación, y otra lo rechazó el límite de 30 MB.
- Si añades un `build.bat`: con expansión diferida un `!` dentro de `echo` desaparece, el lado izquierdo de un `|` no expande `!VAR!`, y `for /f` con una ruta entre comillas necesita `call`.
- Al añadir campos a un archivo de configuración o de trabajo, los archivos viejos deben seguir abriendo: rellenar lo que falte y probar con un archivo viejo.
- En la tabla de entrada no redibujes todo al teclear: se pierde el foco. ListView no edita subcolumnas; hay que superponer un EDIT a la celda y manejar Tab, Enter y flechas. Es la parte más laboriosa de la interfaz.
- Una referencia "exacta" también se equivoca: la primera generación de columnas se cortaba sin converger y daba una cota inválida; se vio al contrastarla con óptimos publicados. Y medir solo familias parecidas escondió el ciclado y la lentitud con muchos largos distintos. Toda cota nueva se contrasta contra algo independiente, y toda medición incluye el caso raro.

## Cómo trabaja Camilo
- En español, directo y concreto. Pide fuentes verificadas en lugar de afirmaciones de memoria.
- Responde bien a preguntas de opción múltiple y corrige los supuestos equivocados (lo hizo con el DXF). Reporta fallas con capturas de pantalla.
- Prefiere lo que abre con doble clic en su equipo. Cuando hubo dos rutas (compilar en GitHub Actions o en su equipo) usó la de su equipo.
- En TuboDXF pidió recibir solo los cambios del programa, un proceso proporcional al tamaño del cambio, y revisión independiente solo cuando cambia la geometría (aquí: cuando cambia el cálculo).

## Primer paso
1. Comprueba el entorno: `uname -a` y `which x86_64-w64-mingw32-g++`. Si no estás en un Linux donde `apt` instale mingw y wine, detente y díselo: todo el plan de entrega depende de eso.
2. Si existe `laboratorio/`, lee su `LEEME.md` y corre `python3 pruebas.py` (sin dependencias) y `python3 pruebas_propio.py` (necesita numpy).
3. Haz la ronda de "Por confirmar" y pídele dos o tres listas reales de pedidos: no se ha visto ninguna.
4. Orden de construcción: núcleo con sus pruebas → ventana con la tabla de entrada y el resumen de barras → primera entrega pequeña para confirmar que abre en su equipo (Windows mostrará el aviso de SmartScreen por no estar firmado) → PDF, Excel y DXF con borradores → pulido.

## Referencias
Consultadas el 2026-10-06.
- Parámetros de nesteo de tubo en un programa real (zona muerta, margen frontal, separación, corte común): manual de TubesT, https://www.bochu.com/tutorials/auto-nest/
- "Tackling tube nesting", Canadian Metalworking (2022): los mandriles pueden dejar sin cortar de 10 a 20 pulgadas al final de la barra. https://www.canadianmetalworking.com/canadianfabricatingandwelding/article/automationsoftware/tackling-tube-nesting
- Modelos y algoritmos de corte 1D: Delorme, Iori y Martello (2016), *European Journal of Operational Research* 255(1):1–20, DOI 10.1016/j.ejor.2016.04.030.
- Instancias con óptimo conocido: BPPLIB, https://github.com/mdelorme2/BPPLIB (licencia CC BY-NC-ND 4.0: sirve para medir, no para copiar su código).
- Fase 2, encajar cortes en ángulo: Lewis y Bonnet (2025), "Exact algorithms in bar nesting", *Computers & Industrial Engineering* 200, 110838, https://rhydlewis.eu/papers/CAIE2025.pdf. Algoritmo exacto O(n²) para ordenar piezas de extremos inclinados en una sola barra. Código en https://doi.org/10.5281/zenodo.11657149 (zenodo.org está bloqueado desde el sandbox).
- Cómo carga Claude Code este archivo y por qué conviene mantenerlo por debajo de 200 líneas: https://code.claude.com/docs/en/memory
