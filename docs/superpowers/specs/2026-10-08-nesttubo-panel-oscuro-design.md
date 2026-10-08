# NestTubo 0.4: panel oscuro (camino C). Especificación

Integra `borrador_pantallas.md`, `borrador_arquitectura.md` y `borrador_riesgos_pruebas_entrega.md`, y resuelve las críticas de las dos revisiones (lista al final, §10).
Citas: `main.cpp:N` es `/home/user/as/nesttubo/src/main.cpp` **de 1462 líneas** (versión actual); `proto.cpp:N` es el prototipo desechable `sondeo_direct2d/src/proto.cpp`. Las citas de main.cpp tomadas del inventario se hicieron sobre la versión de 1394 líneas: antes de codificar se vuelven a comprobar todas y se inventaría lo añadido (instancia única, `WM_COPYDATA`, `main.cpp:1364-1373` y `1410-1432`).
Opción usada para lo pendiente de Camilo: **las medidas de cada perfil se editan solo en la pantalla Perfiles**. Lo que cambiaría con la otra opción está en §2.9.

---

## 1. Objetivo y criterios de éxito

### 1.1 Lo que dijo Camilo [C]
- Quiere la ventana como un panel oscuro "práctico pero también vistoso", con la referencia de panel azul noche, menú lateral, tarjetas, anillo, curva y acentos violeta y naranja.
- Eligió menú lateral con tres pantallas separadas: **Pedido, Resultado y Perfiles**.
- Eligió el **camino C**: Win32 + Direct2D + DirectWrite, todo dibujado, con cuadrícula propia y un EDIT nativo superpuesto para escribir en la celda. El camino A (GDI+ conservando la ListView) es el respaldo si C falla en su Windows.
- Aprobó la maqueta (`maqueta/project/*.dc.html`) como dirección visual.
- Tiene un solo equipo (CLAUDE.md, requisitos).
- Lo de siempre: un `.exe` que abre con doble clic, sin instalar nada (CLAUDE.md, requisitos y "Cómo trabaja Camilo").

### 1.2 Lo supuesto [S] (se le dice al entregar)
- Que el panel debe hacer todo lo que hace la 0.3: nada de lo que hoy funciona se pierde (paridad). Es la condición de éxito principal.
- Que debe caber en una pantalla de 1366×768 con escala al 150 %. No se sabe qué pantalla tiene; es el peor caso razonable de un portátil de oficina.
- Que los atajos de teclado nuevos le sirven.
- Que la curva de la referencia no hace falta (la maqueta aprobada no la tiene).

### 1.3 Criterios de éxito (cada uno se comprueba en §6)
1. **Paridad**: cada elemento del inventario de la ventana actual tiene su lugar en el panel (tabla §2.10) y se comporta igual salvo lo marcado como cambio en §2.
2. **Digitación sin pérdidas**: el guion fijo de tecleo (§6.2) a 5, 12 y 20 ms por tecla deja un `.ntb` idéntico al esperado; la ñ y las tildes de un `.ntb` abierto sobreviven a editar y guardar.
3. **Salidas idénticas**: Exportar desde la ventana deja PDF, XLSX y DXF iguales byte a byte a `build/pruebas --salidas` (CLAUDE.md, Salidas).
4. **Nada oculto**: ningún perfil, tarjeta o grupo de barras deja de verse por falta de espacio; lo que no cabe se desplaza o dice "y N más".
5. **Cabe**: la ventana cabe en 900×440 DIP útiles y la tabla de Pedido muestra ahí al menos 8 filas.
6. **Foco**: después de Calcular, Cancelar, Cambiar carpeta, un cuadro o Alt+Tab, el teclado responde sin hacer clic.
7. **Resultado viejo visible**: un resultado cuyos datos cambiaron se distingue siempre de uno vigente, en cualquier pantalla.
8. **Motor intacto**: `nucleo.cpp`, `trabajo.cpp`, `salidas.cpp` y `datos.cpp` no cambian; `./compilar.sh pruebas` y `./compilar.sh datos` terminan en "FALLOS: 0".
9. **En su Windows**: la sonda (§7.1) abre, no pierde teclas y Camilo la encuentra al menos tan cómoda como la 0.3.

---

## 2. Pantallas y uso

### 2.1 Estructura común

```
+-----------+------------------------------------------------------+
|  MENÚ     |  CABECERA: título · subtítulo · botones · [Calcular F5]
|  Pedido   |------------------------------------------------------|
|  Resultado|              CONTENIDO DE LA PANTALLA                |
|  Perfiles |                                                      |
|  TRABAJO  |------------------------------------------------------|
|  CARPETA  |  LÍNEA DE ESTADO (último aviso)                      |
+-----------+------------------------------------------------------+
```

- Menú, cabecera y línea de estado están en las tres pantallas: Calcular/Cancelar, el estado de la carpeta y los avisos se ven siempre.
- Colores de la maqueta (Main.dc.html:14-26): fondo #10152b, tarjeta #19203d, borde #262f55, violeta #6c4cf5 para lo seleccionado, naranja #ffa64d para la acción principal y para "no cabe" y "cálculo viejo". Rojo solo para errores (carpeta sin conexión, ERROR del validador).
- Los cuadros de aviso y los diálogos de Abrir, Guardar y Elegir carpeta son los de Windows y **salen en tema claro**. Se acepta y se dice al entregar.

**Tamaño**
- Mínimo de seguimiento: 860×420 DIP de área cliente (hoy 1110×560, main.cpp, WM_GETMINMAXINFO; el prototipo pide 1100×680). Se calcula con el DPI de la ventana.
- Arranque: 1280×800 DIP recortados al área de trabajo del monitor; si 860×420 no cabe sin recortar, arranca maximizada. Nunca en (0,0) con 1400×900 como el prototipo.
- **Modo estrecho**, con menos de 1100 DIP de ancho: menú solo con iconos (64 DIP en vez de 228); en Pedido las tarjetas de la derecha pasan a una franja de cifras sobre la tabla; en Resultado las cifras van arriba y la lista de perfiles y el plan quedan en dos columnas, cada una con su desplazamiento.
- Nada se omite: lo que no cabe se desplaza o dice "y N más" (corrige proto.cpp:1137 y 1218).
- **Columnas**: ancho por peso con un mínimo por columna; no se pueden arrastrar (hoy sí, HDN_BEGINTRACK de la ListView: **se pierde y se dice**). Un texto que no cabe se corta con "…" y, al marcar la celda, el texto completo sale en la línea de estado.

### 2.2 Menú lateral
1. Logo y "NestTubo".
2. Entradas **Pedido** (Ctrl+1), **Resultado** (Ctrl+2) y **Perfiles** (Ctrl+3), la activa en violeta.
   - Resultado: insignia con el total de barras si el cálculo está vigente; "…" mientras calcula; **punto naranja** si quedó viejo (proto.cpp:755-758, con la posición calculada tras el texto y no en x fija).
   - Perfiles: un **punto** cuando se acaba de crear o traer un perfil al escribirlo en Pedido; se apaga al visitar Perfiles.
3. **Tarjeta Trabajo**: el nombre del archivo o "Trabajo nuevo", y debajo "Guardado" o "Con cambios sin guardar" (proto.cpp:761-769, sin el rótulo "D2D · DCRenderTarget"). La barra de título de Windows sigue con "NestTubo - x.ntb *" (función `titulo` de main.cpp).
4. **Tarjeta Carpeta de datos**: primero el estado y luego la ruta, cortada con "…"; el texto completo en un globo al pasar el ratón; un clic lleva a Perfiles. Textos (los de `etiqueta_datos` de hoy, más uno nuevo):

| Situación | Punto | Texto |
|---|---|---|
| Sin carpeta elegida | gris | Carpeta de datos: este equipo |
| Nada esperando | verde | **Al día · X** (texto nuevo) |
| Copiando | verde, fijo | Carpeta de datos: X (copiando …) |
| Revisando una carpeta elegida | gris | Revisando la carpeta elegida... |
| Comprobando | gris | Comprobando X... |
| Trabajos esperando | naranja | N trabajos esperando conexión con X (guardados en este equipo) |
| Sin conexión | rojo, negrita | Sin conexión con X: N trabajos y cambios de perfiles esperando en este equipo |
| Error al copiar | rojo, negrita | Error al copiar: N trabajos sin copiar |
| Error del catálogo | rojo, negrita | el texto del error |

   El punto no gira: no hay animación en la tarjeta.
5. Sin número de versión en el menú (la maqueta pone "Versión 0.4"; la versión está en el LEEME).

### 2.3 Cabecera y botones
- Izquierda: título de la pantalla y subtítulo: Pedido "x.ntb · carpeta" o "Trabajo nuevo"; Resultado el estado del cálculo (§2.6); Perfiles el texto de §2.7.
- Derecha, en las tres pantallas: **Calcular F5** en naranja; mientras calcula pasa a **Cancelar** con borde rosa en el mismo sitio (proto.cpp:781-787).
- **Nuevo, Abrir…, Guardar y Guardar como…** en las tres pantallas (editar Perfiles también deja el trabajo con cambios). En modo estrecho, solo icono, con el nombre en un globo.
- **Exportar PDF, Excel y DXF…** solo en Resultado. **Cambiar carpeta…** solo en Perfiles. **Quitar fila** al pie de la tabla de Pedido; **Quitar perfil** en la tarjeta de la tabla de Perfiles.
- Los botones se alcanzan con el ratón y con su atajo; no hay Tab entre botones (hoy tampoco: no hay IsDialogMessage).

**Atajos (lista única; todos nuevos salvo lo dicho)**

| Acción | Tecla | Funciona con la celda abierta | Se apaga cuando |
|---|---|---|---|
| Pantalla Pedido / Resultado / Perfiles | Ctrl+1 / Ctrl+2 / Ctrl+3 | sí, confirma antes | nunca |
| Calcular | F5 | sí, confirma antes | calculando (F5 pasa a Cancelar) |
| Cancelar | F5 o el botón (no Esc) | sí | no calculando |
| Nuevo | Ctrl+N | sí, confirma antes | calculando |
| Abrir | Ctrl+O | sí, confirma antes | calculando |
| Guardar | Ctrl+S | sí, confirma antes | calculando |
| Guardar como | Ctrl+Mayús+S | sí, confirma antes | calculando |
| Exportar | Ctrl+E | sí, confirma antes | calculando, sin resultado o resultado viejo |
| Quitar filas marcadas | Supr (y Ctrl+Supr, igual) | no (dentro de la celda Supr borra texto) | calculando |

- Un botón apagado se ve apagado, no responde al clic y su atajo no hace nada. Las reglas son las de `botones()` de hoy; Cambiar carpeta se apaga además mientras se revisa una carpeta.
- **Mientras calcula** no se puede editar ni quitar filas en ninguna tabla: el clic marca la celda pero no la abre. Se puede navegar, cambiar de pantalla y cerrar.
- El globo de Exportar apagado dice el motivo con los textos de hoy: "Primero calcula las barras: se exporta el último cálculo." o "Los datos cambiaron desde el último cálculo. Calcula de nuevo antes de exportar." (main.cpp:777, 781).

### 2.4 Pedido

**Tarjeta "Piezas del pedido"** (izquierda)
- Ayuda: "Ángulo 0 = corte recto, 45 = inglete · largo de punta a punta".
- Columnas: #, Perfil, Pieza, Largo (mm), Ángulo 1, Ángulo 2, Cantidad. Números a la derecha. Los ángulos se muestran con "°"; la celda abierta muestra solo el número.
- Punto del color del perfil en cada fila. El color va atado al nombre del perfil (orden en la tabla de perfiles), igual en las tres pantallas.
- Fila vacía final con "+ escribe una pieza nueva…".
- Etiqueta naranja **"no cabe"** en la fila cuya pieza mide más que el útil de su perfil (barra − despunte − zona muerta), con la misma lectura de decimales que el cálculo (Main.dc.html:114-116). Si algún dato de la fila o del perfil no se puede leer, no se pone etiqueta.
- Al pie: botón Quitar fila (apagado sin filas marcadas), la ayuda "Tab → siguiente · Enter ↓ · Esc deshace · Supr quita filas" y "N filas".
- No hay botón "Agregar pieza": la fila vacía cumple esa función.

**Columna derecha**, de arriba abajo:
1. **Barras a enviar** (proto.cpp:910-924): sin cálculo "— Aún sin calcular. Pulsa F5."; calculando "Calculando perfil k de n…"; vigente el número, "mínimo demostrado en k de n perfiles" y el anillo; **viejo** el número en gris con "Cálculo viejo: los datos cambiaron. F5 para recalcular." en naranja.
2. **Pieza seleccionada** (Main.dc.html:144-170): nombre y perfil, "Ángulo 1: 0° · Ángulo 2: 45°", el trapecio con el lado largo abajo y cada extremo de arriba recortado alto × tan(ángulo), con tope del 40 % del ancho; "N mm de punta a punta", cantidad y cara ancha; rótulo "dibujo sin escala". Sin cara ancha: "cara ancha sin dato". Con un ángulo ilegible o de 90 o más: no se dibuja el trapecio y dice "Ángulo no válido". En la fila vacía: "Escribe una pieza".
3. **Por perfil** (Main.dc.html:172-189): una barra por perfil con "N piezas · X m" y el total "N piezas · X m en piezas" (corrige "m de tubo"). Más de 5 perfiles: los 5 con más metros y "y N más".

En modo estrecho: franja de cifras (barras a enviar, piezas, metros); Pieza seleccionada plegada, se abre con un clic.

### 2.5 Digitación en la cuadrícula (Pedido y Perfiles)
Dos modos: **navegación** (celda marcada con borde violeta) y **edición** (EDIT nativo abierto sobre la celda). Lo marcado **(nuevo)** se le dice a Camilo.

**Entrar a escribir**
- Clic en una celda: la abre en edición (como hoy). Doble clic, F2 o Enter: abren la celda marcada (hoy F2/Enter abrían la columna 0).
- **(nuevo)** Escribir una letra o número abre la celda marcada y reemplaza su contenido (proto.cpp:1511-1518). La línea de estado muestra el valor anterior ("Largo antes: 2450") y Esc lo devuelve mientras la celda siga abierta.
- Al abrir, el texto queda seleccionado: la primera tecla lo reemplaza. Para corregir sin borrar: F2 y luego Fin.

**Dentro de la celda** (como hoy, `EditProc`)
- Tab: siguiente columna; desde la última, primera de la fila de abajo. Mayús+Tab: al revés.
- Enter o flecha abajo: baja; flecha arriba: sube. Esc: descarta.
- Izquierda, derecha, Inicio y Fin mueven el cursor en el texto. Ctrl+C/V/X/Z, ñ, tildes y coma decimal: los del EDIT.
- **(nuevo)** RePág/AvPág: confirman y abren la misma columna una página más abajo o arriba.
- **(nuevo)** Los atajos de la tabla §2.3 marcados "sí" confirman la celda y luego actúan.
- En la fila vacía: si se escribió algo, al confirmar nace otra fila vacía y Enter/Tab siguen; si no, Enter sale de la edición y deja la celda marcada.
- Subir desde la primera fila deja la celda marcada (hoy pasaba el foco a la tabla).
- En Perfiles, Tab salta la columna Útil (solo lectura).

**Al confirmar** (reglas de main.cpp, no del prototipo)
- Una pieza nueva en la que se escribe primero otra columna copia el perfil de la fila de arriba.
- Un nombre de perfil que no está en la tabla se trae del catálogo o se crea (§2.5.1).
- Texto igual al anterior: nada cambia. Distinto: el trabajo queda "con cambios" y un resultado vigente pasa a viejo.

**En navegación**
- Flechas, Tab y Mayús+Tab mueven la celda marcada (corrige el error de Tab en la última celda, proto.cpp:557-563: desde la última columna de la fila vacía no se mueve).
- **(nuevo en la cuadrícula; hoy lo daba la ListView en parte)** Inicio/Fin: primera y última columna. Ctrl+Inicio/Ctrl+Fin: primera fila y fila vacía final. RePág/AvPág: una página.
- **Varias filas**: Mayús+flechas, Mayús+RePág/AvPág. **(nuevo)** Clic en el número de fila (#) marca la fila sin abrir la edición; Mayús+clic en # extiende; Ctrl+clic en # suma o quita. Filas marcadas con fondo violeta tenue.
- **Supr** (o Ctrl+Supr): quita las filas marcadas, o la fila de la celda marcada si no hay filas marcadas, **siempre con la pregunta de hoy** ("¿Quitar N fila(s) de piezas?" / "¿Quitar el perfil? También se quita del catálogo…"). La fila vacía final nunca se quita. Supr no vacía celdas: para vaciar una, se abre y se borra.
- Esc en navegación quita la marca de varias filas; sin marca, no hace nada.
- Rueda: 3 filas, acumulando el resto (ratones de desplazamiento fino). Barra de desplazamiento propia a la derecha: zona de clic de 12 DIP, pulgar de 4 DIP que se ensancha a 8 al pasar el ratón, se arrastra, y un clic en el riel avanza una página.
- **Desplazar no cierra la celda abierta (mejora)**: el EDIT se lleva fuera de la vista sin perder el foco y, al seguir tecleando, la tabla vuelve a mostrar la fila.
- Cambiar de pantalla, Nuevo, Abrir, cerrar la ventana y perder el foco (Alt+Tab) **confirman y cierran** la celda. Cambiar el tamaño de la ventana o pasar al modo estrecho **no** la cierran: el EDIT se recoloca.

**Foco**
- Al arrancar, la celda Perfil de la fila 1 de Pedido está marcada y el teclado responde sin clic.
- Al volver con Alt+Tab, la celda sigue marcada y el teclado responde.
- El foco del teclado siempre está en la ventana (o en el EDIT abierto). Ningún botón apagado se queda con él.

#### 2.5.1 Perfil que se trae o se crea al escribirlo (main.cpp, `perfil_si_falta`)
- Está en el catálogo: se trae. Línea de estado: "Perfil X traído del catálogo. Sus medidas están en la pantalla Perfiles."
- No está: se crea con 6000 · 10 · 230 · 3 · 0, se anota en el catálogo. Línea de estado: "Perfil X agregado con los valores iniciales (barra 6000, despunte 10, zona muerta 230, separación 3, margen 0). Revísalos en la pantalla Perfiles (Ctrl+3)."
- En los dos casos se enciende el punto de Perfiles. La edición sigue en Pedido; no se cambia de pantalla.

### 2.6 Resultado

**Calcular y cambio de pantalla (regla única)**
- F5 o el botón **no** cambian de pantalla.
- Al terminar: si Camilo no tocó tecla ni ratón desde F5 y no hay celda abierta, se pasa a Resultado y el foco queda en la lista de perfiles con el primero elegido. Si tocó algo, se queda donde está, con su celda marcada; se avisa con la insignia del menú, la mini tarjeta de Pedido y la línea de estado.
- Ese mismo criterio vale para Cancelar.

**Con un resultado vigente**
- Cabecera: "Resultado", y debajo "Barras a enviar al proveedor de corte láser". A la derecha, Calcular F5 y Exportar.
- **Cuatro cifras**: (1) Barras a enviar, "N mínimo + M de margen"; (2) Aprovechamiento con anillo, "largo de las piezas sobre el largo de las barras mínimas"; (3) Piezas en el plan "76 de 77" con "1 no cabe" en naranja; (4) Mínimo demostrado: "En los 3 perfiles · no hay forma de usar menos barras con estos datos" o, en naranja, "En 2 de 3 perfiles · en X podrían sobrar hasta N barras".
- **Lista de perfiles** (con desplazamiento propio): una tarjeta por perfil calculado, en el orden de la tabla de perfiles, con su color: **N barras**, "de 6000 mm", "X mínimo + Y de margen", "N piezas" con "(+K no caben)", anillo de aprovechamiento y "✓ mínimo demostrado" o "no demostrado: podrían sobrar hasta N". Cubre las 8 columnas del resumen de hoy. Los perfiles sin piezas **no salen** (preparar los salta, como hoy). Modo estrecho: una línea por perfil (color, nombre, barras, aprovechamiento, ✓). Se elige con clic o con flechas arriba/abajo.
- **Plan de corte del perfil elegido**: título "Plan de corte · <perfil>" y "Barra 6000 mm · útil 5760 mm · separación 3 mm"; leyenda (color y nombre de cada pieza con su largo, Despunte, Zona muerta rayada, Sobrante); una tira por grupo de barras iguales con "Barras 1–2 × 2 · 5 piezas", despunte, piezas en proporción como trapecios según sus ángulos, zona muerta rayada, "sobrante 337,5 mm (con zona muerta)" y debajo "4 × Larguero · 1 × Travesaño"; una fila por barra de margen "Barra 15 · margen" con recuadro a trazos "Barra extra de margen, sin cortes"; al final el recuadro naranja "No cabe: Larguero (× 8), 6300 mm, más largo que el útil (6160). Quedó fuera del cálculo." (texto armado como hoy, trabajo.cpp:279-283).
- **Inicio y fin de cada pieza**: globo al pasar el ratón por una pieza ("Larguero · 1450 mm · de 10 a 1460") y botón **Ver detalle** bajo el plan que muestra la tabla de hoy (Barra, Pieza, Largo, Inicio, Fin, Sobrante) con los textos de `filas_plan()`.
- El plan se desplaza de verdad: barra arrastrable, rueda y, con el foco en el plan, flechas, RePág/AvPág, Inicio/Fin. Tab alterna entre lista y plan.
- Colores de pieza: dos piezas vecinas en una barra nunca comparten color; el nombre escrito es la referencia.
- **Fuera**: la curva "Sobrante por barra" del prototipo.

**Estados**

| Estado | Qué se ve | Menú | Exportar |
|---|---|---|---|
| Sin cálculo (arranque, Nuevo, Abrir) | anillo vacío, "Aún no hay cálculo. Escribe las piezas en Pedido y pulsa F5." | sin insignia | apagado |
| Datos por corregir al pulsar F5 | sigue el estado anterior; cuadro "Corrige esto antes de calcular" (hasta 15 líneas y "... y N más."); línea de estado "N datos por corregir." | igual | igual |
| Calculando | indicador, "Calculando perfil k de n…", botón Cancelar | "…" | apagado |
| Cancelado | el resultado con lo mejor que había; etiqueta naranja "cancelado" en los perfiles cortados | insignia | encendido |
| Tiempo agotado en un perfil | etiqueta "tiempo agotado" y "no demostrado" | insignia | encendido |
| ERROR del validador en un perfil | tarjeta con borde rojo, "ERROR" y el motivo; al elegirla, el plan muestra el motivo en lugar de las tiras | insignia | **encendido** (hoy Exportar solo mira que haya resultado y que no sea viejo, main.cpp:775-782) |
| Vigente | todo lo anterior | insignia con las barras | encendido |
| Viejo | todo a la vista, cifras atenuadas y **franja naranja fija** arriba: "Los datos cambiaron desde este cálculo. Pulsa F5 para recalcular." | punto naranja | apagado |

- Si se edita durante el cálculo no es posible (§2.3); además, si los datos cambiaron entre el inicio y el fin (contador de versión), el resultado nace viejo.
- Mensaje final en la línea de estado: el de hoy (`fin_calculo`, main.cpp): "Listo: N barras a enviar en K perfil(es)." más "E con error.", "Mínimo no demostrado en X." o "Mínimo demostrado en todos.", "C cortado(s) por tiempo o cancelación." y "P pieza(s) no caben: ver el plan.".
- **Indicador de cálculo**: gira con un temporizador **solo** si la pantalla visible es Resultado, no hay sesión remota y Windows tiene las animaciones activadas; si no, queda fijo y se actualiza con cada aviso de progreso.
- **Animación al terminar** (0,7 s: números que cuentan, anillos y tiras que crecen): solo en Resultado y con las mismas dos condiciones; si no, se pinta el estado final directo.

**Exportar**: el flujo de hoy sin cambios (main.cpp, `exportar`). Antes de escribir, la línea de estado dice "Exportando…" y se fuerza el repintado.

### 2.7 Perfiles
- Título "Perfiles del trabajo". Subtítulo: "Medidas en mm · lo que cambies aquí queda también en el catálogo para los trabajos nuevos" (corrige "Catálogo compartido · los cambios llegan solos a los demás equipos").
- **Tarjeta "Medidas por perfil (mm)"**: columnas Perfil, Cara ancha, Barra, Despunte, Zona muerta, Separación, Margen y **Útil** (gris, solo lectura, barra − despunte − zona muerta; vacío si algún dato no se lee). Comportamiento de §2.5. Fila vacía "+ perfil nuevo…" (no hay "Agregar perfil"). Ayuda: "Margen = barras extra que se envían por perfil · Un perfil nuevo toma 6000 · 10 · 230 · 3 · 0". Botón Quitar perfil.
- **Tarjeta "Cómo se usa la barra · <perfil marcado>"**: dibujo de la maqueta con rótulo "dibujo sin escala" y la lista de valores (despunte, útil, zona muerta, separación, margen, cara ancha). Sin la palabra "mandril".
- **Tarjeta "Carpeta de datos"**: ruta, estado (tabla §2.2) y botón **Cambiar carpeta…**. Texto: "Los trabajos y los cambios de perfiles se guardan primero en este equipo y se copian solos a la carpeta mientras NestTubo está abierto. Si la red se cae, se ve aquí y en el menú, y se copian al volver." El botón hace lo de hoy (`elegir_carpeta_datos`, examinar, pregunta de mezcla con hasta 15 diferencias).
- **Reglas al editar** (main.cpp, `editar_perfil`): nombre repetido → pitido, "Ya hay un perfil X en la fila N. No se cambió el nombre." y la celda vuelve a su valor; nombre nuevo en fila vacía → se trae del catálogo o se rellenan con los valores iniciales las medidas vacías (no la cara); otra medida de un perfil con nombre → se anota solo ese campo. Quitar perfil pregunta lo de hoy y, **(mejora)**, si alguna pieza lo usa añade "Lo usan N piezas del pedido; esas piezas darán error al calcular."

### 2.8 Línea de estado, avisos y cierre
- Punto (verde, naranja o rojo) y el último aviso. Se conservan todos los textos de hoy, cambiando "tabla de perfiles" por "pantalla Perfiles" y "Calcular barras" por "F5".
- Texto inicial: "Escribe las piezas del pedido y pulsa F5 para calcular las barras. Los perfiles nuevos se crean solos; sus medidas están en Perfiles (Ctrl+3)."
- Avisos del hilo de red: como hoy, **esperan** a que no haya celda en edición ni otro cuadro abierto. Mientras esperan, el punto se pone naranja con "Aviso pendiente: se muestra al terminar la celda".
- "¿Guardar los cambios del trabajo actual?" antes de Nuevo, Abrir, abrir por `WM_COPYDATA` y cerrar, como hoy.
- **Cerrar**: como hoy (cancela el cálculo, pregunta, espera hasta 3 s la copia), pero "Copiando a la carpeta de datos antes de cerrar..." se dibuja antes de la espera.
- **Cerrar sesión o apagar Windows (mejora)**: ver §3.10.
- **Contraste alto**: si Windows lo tiene activado al arrancar, un cuadro una vez por sesión: "Windows tiene activado el contraste alto. El panel de NestTubo 0.4 no lo sigue; si no se lee bien, usa NestTubo 0.3, que sigue funcionando con los mismos archivos."

### 2.9 Si Camilo elige también editar medidas en Pedido
- Pedido gana sobre la tabla una **franja** con las medidas del perfil de la fila marcada (Cara ancha, Barra, Despunte, Zona muerta, Separación, Margen y Útil de solo lectura), con el mismo EDIT y las mismas reglas que Perfiles, anotando en el catálogo. No edita el nombre. Nota: "también cambia el catálogo para los trabajos nuevos". La cara ancha sale de Pieza seleccionada.
- Tab no pasa de la tabla a la franja: se entra con clic o con F6.
- La franja quita unos 70 DIP: a 440 DIP se ven unas 6 filas en vez de 8 (criterio 5 pasa a 6 filas).
- El punto de Perfiles en el menú se mantiene igual.
- Código: una tercera `Rejilla` de una fila que apunta al perfil de la pieza marcada; `main` sabe qué rejilla tiene el foco; `edicion` no cambia; 100-150 líneas más.

### 2.10 Dónde queda cada cosa de hoy

| Hoy (main.cpp) | En el panel |
|---|---|
| Arranque, argv[1], instancia única con `WM_COPYDATA` | igual (§3.10) |
| Tamaño inicial y mínimo | 1280×800 recortado / 860×420 DIP |
| 4 ListView | Pedido (piezas), Perfiles (perfiles), Resultado (resumen y plan) |
| Etiquetas de ayuda | ayuda bajo el título de cada tarjeta |
| Nuevo, Abrir, Guardar, Guardar como | cabecera de las 3 pantallas + atajos |
| Calcular / Cancelar | cabecera de las 3 pantallas, F5 |
| Exportar | Resultado, Ctrl+E |
| Carpeta de datos... | "Cambiar carpeta…" en Perfiles; estado en el menú |
| Quitar fila / Quitar perfil | botones + Supr, misma pregunta |
| Título con * | igual + tarjeta Trabajo |
| Resultado viejo (un mensaje) | franja fija, punto en menú, mini tarjeta |
| Progreso | Resultado, mini tarjeta, línea de estado |
| "Corrige esto antes de calcular" | igual |
| Resumen (8 columnas) | tarjetas de perfil |
| ERROR, tiempo agotado, cancelado, no demostrado | etiquetas en tarjetas y cifra "Mínimo demostrado" |
| Plan (Barra, Pieza, Largo, Inicio, Fin, Sobrante) | tiras + globo + "Ver detalle" con la tabla de hoy |
| Fila NO CABE | recuadro en el plan + etiqueta en Pedido |
| Fila vacía, copiar perfil de arriba, traer/crear perfil, nombre repetido | igual |
| Teclas de la celda | iguales + RePág/AvPág + atajos |
| Clic/doble clic/F2/Enter | iguales (F2/Enter abren la celda marcada) + teclear reemplaza |
| Supr quita filas con pregunta | igual (también sin filas marcadas: la fila de la celda) |
| Mayús+flechas | igual + clic en # |
| Inicio/Fin/RePág/AvPág de la ListView | escritos en la cuadrícula |
| Búsqueda por primera letra de la ListView | **se pierde** (teclear reemplaza) |
| Arrastrar el ancho de columna | **se pierde** (texto completo en la línea de estado) |
| Rueda/barra cierra la edición | ya no la cierra |
| Barra de estado izquierda / derecha | línea de estado / tarjeta Carpeta |
| Avisos de red diferidos | igual + punto "aviso pendiente" |
| Cierre (cancelar, preguntar, 3 s) | igual, con el mensaje visible |
| Diálogos nativos y "Sin conexión" | iguales (tema claro) |
| Lector de pantalla de la ListView | **se pierde** |

---

## 3. Arquitectura del código

### 3.1 Principios
1. `nucleo.cpp`, `trabajo.cpp`, `salidas.cpp` y `datos.cpp` no cambian (§3.11).
2. Lo que se decide sin HWND va a unidades sin `windows.h`, que compilan con g++ nativo.
3. Un solo dueño del estado: `App`, tocado solo por el hilo de la ventana; los hilos de cálculo y de red hablan por `PostMessage`.
4. YAGNI: sin capa de widgets, sin sistema de eventos, sin temas. Tres pantallas fijas y una cuadrícula usada dos veces.
5. Un archivo, un propósito, de 150 a 500 líneas; funciones libres y structs simples (como `nt::`, `datos::`).

### 3.2 Unidades
[N] = se prueba nativo con g++; [W] = solo bajo wine o Windows.

| # | Archivo | Propósito | Depende de | Prueba |
|---|---|---|---|---|
| 1 | `edicion.h/.cpp` | Reglas del modelo al confirmar una celda o quitar filas | trabajo.h | N |
| 2 | `vista.h/.cpp` | Datos derivados: resúmenes, textos, colores, «no cabe», útil, texto de fin de cálculo, `etiqueta_datos` | trabajo.h, nucleo.h | N |
| 3 | `rejilla.h/.cpp` | Modelo de cuadrícula: selección, rango, desplazamiento, teclado de navegación | — | N |
| 4 | `distribucion.h/.cpp` | Rectángulos en DIP de cada zona y zonas clicables; modo estrecho | rejilla.h | N |
| 5 | `acciones_habilitadas.h/.cpp` | `habilitado(Accion, Estado)`: la regla de `botones()` como función pura | — | N |
| 6 | `lienzo.h/.cpp` | Fábricas D2D/DWrite, destino DC, recreación, caché, fuentes, primitivas | d2d1, dwrite | W |
| 7 | `dibujo_marco.cpp`, `dibujo_rejilla.cpp`, `dibujo_graficos.cpp`, `pedido.cpp`, `resultado.cpp`, `perfiles.cpp` | Dibujo | 2, 3, 4, 6 | W |
| 8 | `celda_edit.cpp` | EDIT superpuesto y `EditProc` | 3, 6 | W |
| 9 | `acciones.cpp` | Archivo, exportar, carpeta de datos, `cuadro()` y pendientes | datos.h, salidas.h, 1 | W |
| 10 | `calculo.cpp` | Hilo de cálculo y fin de cálculo | trabajo.h, 2 | W |
| 11 | `main.cpp` | `wWinMain`, `Proc`, despacho, atajos, instancia única | todo | W |

### 3.3 `edicion` [N]
Toma de main.cpp (no del prototipo): `editar_perfil`, `perfil_si_falta`, copiar el perfil de arriba, `asegurar_fila_vacia`, `fila_con_perfil` y la parte de modelo de `quitar_filas`.
```cpp
struct Efecto {
  bool cambio = false;          // -> datos_cambiaron (modificado, res viejo, version_datos++)
  bool pitido = false;
  std::string estado;           // aviso para la línea de estado
  std::vector<nt::CambiosCatalogo> anotar;   // -> acciones::anotar
  bool perfil_creado = false;   // -> punto de Perfiles en el menú
};
Efecto confirmar_pieza (nt::Trabajo&, int fila, int col, const std::string& texto,
                        const std::vector<nt::PerfilTxt>& catalogo);
Efecto confirmar_perfil(nt::Trabajo&, int fila, int col, const std::string& texto,
                        const std::vector<nt::PerfilTxt>& catalogo);
Efecto quitar(nt::Trabajo&, bool perfiles, const std::vector<int>& filas);  // sin la fila vacía
int    piezas_que_usan(const nt::Trabajo&, const std::string& perfil);     // para la pregunta
```
El catálogo llega leído: la ventana lo relee de disco con `datos::` antes de llamar, como hoy. Los nombres exactos de los tipos de `trabajo.h`/`datos.h` se ajustan al codificar; la forma (entra el modelo, sale un `Efecto`) es la que se fija aquí.

### 3.4 `vista` [N]
- `resumen_pedido(Trabajo)`, `resumen_res(resultados)` (proto.cpp:664-707), con aprovechamiento "sobre las barras mínimas".
- `texto_fin_calculo(resultados)`, copiado de `fin_calculo` de main.cpp.
- `etiqueta_datos(entrada) -> {texto, nivel}`: sacada de main.cpp; recibe lo leído de config y de `App` como parámetro.
- `color_perfil(trabajo, clave)`: índice en `trabajo.perfiles`, paleta de 12; si hay más de 12, tono por ángulo áureo.
- `color_pieza(barra, i)`: tono por ángulo áureo, saltando al siguiente si coincide con la vecina.
- `util_perfil(perfil) -> optional<décimas>`, `no_cabe(trabajo, fila) -> bool`: con `leer_decimas` y la regla de `preparar()`.
- `grupos_plan(ResultadoPerfil)`: grupos de barras iguales + filas de margen; inicio y fin con `filas_plan()`.

### 3.5 `rejilla` [N]
- Base: `Grid` del prototipo (proto.cpp:102-116, 332-344): columnas con título, peso, **ancho mínimo**, numérica, **solo lectura**; acceso a celda por función.
- Estado: `top`, celda (`f`, `c`), conjunto de filas marcadas con fila ancla, `editando`, resto de rueda.
```cpp
enum class Orden { Nada, Abrir, AbrirReemplazando, Quitar, Pagina };
Orden tecla(Rejilla&, unsigned vk, bool ctrl, bool mayus);
void  clic(Rejilla&, int fila, int col, bool mayus, bool ctrl, bool en_numero);
void  rueda(Rejilla&, int delta);
void  asegurar_visible(Rejilla&, int fila);
struct Desplazable { int top, total, visibles; };   // lista de perfiles y plan de Resultado
```
- Supr y Ctrl+Supr → `Orden::Quitar` (con filas marcadas o la de la celda). No hay orden de vaciar.

### 3.6 `distribucion` [N]
- `Zonas acomodar(Pantalla, float ancho_dip, float alto_dip, const Estado&)`: rectángulos de menú, cabecera, tarjetas, cuadrículas, botones, línea de estado, barras de desplazamiento y la lista de zonas clicables con su acción.
- Se llama en WM_SIZE y en los cambios de estado que cambian la forma (pantalla, calculando, resultado, modo estrecho). **Nunca en WM_PAINT.**

### 3.7 `lienzo` [W]
- Del prototipo: fábricas (proto.cpp:1286-1292), destino DC con `BindDC` (1240-1266, 1343-1347), primitivas (203-261), arco Bezier y anillo (263-310), tarjeta (312-321).
- Cadena de fuentes: **la que fija §4.6**, en un solo sitio (`lienzo`).
- Correcciones: `soltar_destino()` libera también la caché y llama a `InvalidateRect`; degradados en caché por par de colores; fondo en un `ID2D1BitmapRenderTarget`.
- Dibujo de texto de celdas con `DWRITE_MEASURING_MODE_GDI_CLASSIC` si en las capturas de Windows se ve el salto de grosor al abrir el EDIT; si no, el modo por defecto.

### 3.8 `celda_edit` [W]
- `abrir(Rejilla&, rect_px, numerica, texto_inicial)`, `posicionar()`, `cerrar(bool confirmar) -> optional<string>`.
- `EditProc`: el de main.cpp (Tab, Enter, flechas, Esc por `WM_APP_MOVER`, `DLGC_WANTALLKEYS`, se tragan los WM_CHAR de Tab/Enter/Esc) más: RePág/AvPág, la rueda y **todos los atajos de la tabla §2.3 marcados "sí"**, reenviados a una función común `ejecutar(Accion)` que comprueba `habilitado()`. Supr y Ctrl+Supr no se reenvían (son del EDIT).
- Colores con WM_CTLCOLOREDIT (proto.cpp:1519-1522); fuente GDI de la misma familia y tamaño, creada con el DPI de la ventana.
- Fila fuera de la vista: el EDIT se mueve fuera del área cliente con `SetWindowPos`, sin `SW_HIDE`. WM_SIZE lo recoloca.
- Al cerrar devuelve solo el texto; `main` llama a `edicion::confirmar_*`, aplica el `Efecto`, invalida y llama a `revisar_pendientes()`.

### 3.9 `acciones` y `calculo` [W]
- Se mueven sin cambios de main.cpp: `cuadro`, `ocupado`, `revisar_pendientes`, `atender_pendientes`, `dialogo_archivo`, `guardar`, `puede_descartar`, `elegir_carpeta`, `exportar`, `nuevo`, `abrir`, carpeta de datos, `pasada_hecha`, `anotar`, `titulo`. Lo único que cambia: `llenar_tabla` pasa a "rejilla a la fila 0 + invalidar".
- Regla: **todo** MessageBox pasa por `cuadro()` y toda capa modal nueva sube `app.modal`. Excepción única: el fallo de Direct2D al arrancar (§5.1).
- `calculo`: `calcular()`, el hilo (cancelación, 20 s por perfil) y `fin_calculo` con el texto de `vista`. Novedades: `version_datos` comparado al terminar; foco devuelto a la ventana; regla de cambio de pantalla de §2.6.

### 3.10 `main.cpp` [W]
- `wWinMain`: **instancia única de hoy** (mutex `NestTubo-ventana-unica`, `FindWindowW` sobre la clase **`NestTuboVentana`**, que se conserva, y `WM_COPYDATA` con la ruta); `OleInitialize`; argv[1]; registro de clase sin `hbrBackground`, ventana con `WS_CLIPCHILDREN`; bucle de mensajes. Con el mismo mutex y la misma clase, la 0.3 y la 0.4 no pueden estar abiertas a la vez: abrir una con la otra abierta le pasa el archivo a la abierta. Es lo deseado (comparten carpeta de datos).
- `Proc`: despacho; WM_ERASEBKGND = 1; WM_GETMINMAXINFO con el DPI de la ventana; WM_SIZE; WM_PAINT; ratón; teclado (`rejilla::tecla` y atajos); WM_ACTIVATE (devuelve el foco a la ventana); WM_COPYDATA como hoy; WM_CLOSE con repintado forzado (`UpdateWindow`) antes de la espera de 3 s.
- **WM_QUERYENDSESSION**: si calcula, cancela y espera el hilo; cierra la celda confirmando; si hay cambios sin guardar, llama a `ShutdownBlockReasonCreate` ("Hay cambios sin guardar en NestTubo"), pregunta con `cuadro()` "¿Guardar los cambios del trabajo actual?" y devuelve FALSE si elige Cancelar o si Guardar falla; si no, TRUE. **WM_ENDSESSION** con `wParam` TRUE: espera hasta 3 s la copia pendiente, como WM_CLOSE, sin preguntar. Windows puede cerrar el programa si tarda; lo que ya estaba en disco local se copia al volver a abrir (como hoy).
- Números de mensaje de hoy: `WM_APP_MOVER` +1, `PROGRESO` +2, `FIN` +3, `PENDIENTES` +4, `datos::WM_APP_RED` +20. El `WM_APP_TECLA` del prototipo (WM_APP+4) no se copia.
- No se añade arrastrar un `.ntb` a la ventana (§8).

### 3.11 Estado y ficheros que no cambian
- `App` (`app.h`): trabajo y catálogo local; archivo, sello, `modificado`; `res`, `res_viejo`, `version_datos`; `calculando`, `cancelar`, `hilo`, `tocado_desde_F5`; `examinando`, `examen`, cola de avisos; `modal`; `pantalla`, `perfil_elegido`, `perfil_nuevo_sin_ver`; dos `Rejilla`, `Zonas`, `dpi`.
- **config.ini**: no se añade ninguna clave. Ni pantalla ni tamaño.
- **Motor, trabajo, salidas, datos**: sin cambios. La marca «no cabe» se calcula en `vista` y una prueba la compara con `preparar()`.
- **Construcción**: `compilar.sh` añade los archivos nuevos, `-ld2d1 -ldwrite`, y `pruebas_panel.cpp` al objetivo `pruebas`. `app.manifest` **no cambia** (comctl32 v6 + `dpiAware` de sistema).

### 3.12 Flujo de datos
```
teclado/ratón ─> main::Proc ─> rejilla (navegar) ─> celda_edit (EDIT nativo)
                                                └─ confirmar ─> edicion::confirmar_* ─> Efecto
Efecto ─> App.trabajo, version_datos++, datos_cambiaron ─> invalidar(zonas)
       ─> acciones::anotar ─> datos::registrar ─> pedir_pasada ─(hilo red)─> WM_APP_RED
F5 ─> habilitado? ─> calculo::calcular ─> preparar ─ errores ─> cuadro("Corrige esto...")
                                               └─ hilo: resolver() por perfil
                                                   ├─ WM_APP_PROGRESO ─> estado, tarjeta, indicador
                                                   └─ WM_APP_FIN ─> fin_calculo ─> App.res
App.res ─> vista ─> dibujo (Resultado, mini tarjeta, insignia)
App.res vigente ─> acciones::exportar ─> salidas
WM_APP_RED ─> pasada_hecha ─> vista::etiqueta_datos ─> tarjeta Carpeta (invalidar solo esa zona)
WM_APP_PENDIENTES ─> atender_pendientes (si !ocupado()) ─> cuadro() uno a uno
```

---

## 4. Dibujo y rendimiento

### 4.1 Destino
- `ID2D1DCRenderTarget` con `BindDC` al HDC de `BeginPaint`, tipo `DEFAULT` en el producto.
- Razones: bajo wine `HwndRenderTarget` presenta un cuadro atrasado (capturas zm5, zm7); y el HDC de `BeginPaint` respeta el recorte de `WS_CLIPCHILDREN`, así que no se pinta encima del EDIT. Esto último **no está verificado en Windows real**.
- La sonda mide `DEFAULT` y `SOFTWARE` sola (§7.1); si `SOFTWARE` resulta mejor, la 0.4 usa ese y punto.
- Las capturas de vitrina (rap, res2, cap07) son de modo HWND; el modo DC se ve en zna, zn5b, dpi1. Se dice al entregar.

### 4.2 Doble búfer
- Implícito en el DC render target. WM_ERASEBKGND = 1, sin `hbrBackground`, `WS_CLIPCHILDREN`.

### 4.3 Repintado por zonas
1. Distribución, zonas clicables y posición del EDIT: en WM_SIZE y en cambios de estado; nunca en WM_PAINT.
2. Cada elemento tiene su rectángulo; `invalidar(zona)` pide solo ese.
3. WM_PAINT: `PushAxisAlignedClip(rcPaint)` y se salta lo que no corta.
4. Fondo, menú quieto y marcos de tarjeta en un mapa de bits en caché, rehecho en WM_SIZE y al recrear el destino.
5. Degradados en caché por par de colores.
- Reglas: no se pinta en bucle; el único temporizador vive mientras haya una animación visible (§2.6); pasar el ratón invalida la zona vieja y la nueva; teclear en la celda no invalida nada fuera de esa celda.
- Meta bajo wine (llvmpipe, `--log` del build de diagnóstico): mover la selección por debajo de 10 ms por cuadro (hoy 70 ms el cuadro completo).

### 4.4 Animaciones
Dos, con las condiciones de §2.6: el indicador mientras calcula (gira mientras dure el cálculo) y la de 0,7 s al terminar. Ambas se apagan si `GetSystemMetrics(SM_REMOTESESSION)` es verdadero (se consulta al empezar cada una) o si `SPI_GETCLIENTAREAANIMATION` es FALSE. Ninguna depende de una llamada de red o de exportar.

### 4.5 DPI
- **DPI de sistema, como hoy**: `app.manifest` sin cambios (`dpiAware` true), se lee el DPI de la ventana al arrancar, dibujo en DIP y clics/EDIT en píxeles (proto.cpp:179-180). No se llama a `SetProcessDPIAware` (sobra con el manifiesto). No hay PerMonitorV2 ni WM_DPICHANGED en la 0.4 (§8).
- WM_GETMINMAXINFO convierte los 860×420 DIP con el DPI de la ventana.

### 4.6 Fuentes (único sitio donde se fija)
- Producto: Segoe UI, luego Tahoma, luego Arial; si ninguna, la de respaldo de DirectWrite. El EDIT usa la misma familia por GDI y el mismo tamaño.
- Pruebas bajo wine: el build de diagnóstico (§5.4) acepta `--fuente=<familia>`; las capturas de referencia se toman con `--fuente=DejaVu Sans` (sin las alternativas de Inter que cambian x por ×). Al quitar Inter de la cadena, las capturas bajo wine cambian respecto de las del prototipo: se toman de nuevo.
- wine ignora el recorte de texto: los "…" se revisan en las capturas de Windows de la sonda.

---

## 5. Errores y respaldo

### 5.1 Direct2D no arranca
- Al arrancar, antes de crear la ventana, se crean las fábricas y un destino de prueba. Si falla, `MessageBoxW` **directo con ventana nula** (todavía no hay ventana ni hilos, así que `cuadro()` no aplica):
  > "NestTubo 0.4 no pudo iniciar el dibujo de la ventana (Direct2D: código 0x…). Tus trabajos y perfiles no se tocaron. Usa NestTubo 0.3 y envía una captura de este mensaje y el archivo %LOCALAPPDATA%\NestTubo\registro.txt."
- Termina sin escribir en la carpeta de datos.
- No hay segunda interfaz dentro del mismo `.exe`. El respaldo inmediato es la 0.3, que se entrega y se conserva **al lado** de la 0.4 (§7.2). El respaldo de diseño es el camino A (§5.3).

### 5.2 D2DERR_RECREATE_TARGET
- `soltar_destino()` libera destino, pincel, degradados y mapa de bits del fondo, y llama a `InvalidateRect(hwnd, nullptr, FALSE)` (falta en proto.cpp:1378).
- Si falla 3 veces seguidas, el mensaje de 5.1 (por `cuadro()`, porque ya hay ventana) y cierre ordenado pasando por `puede_descartar`.

### 5.3 Cuándo se vuelve al camino A
En la sonda, en su Windows, y sin arreglo con `SOFTWARE` ni con un ajuste puntual:
- Direct2D no arranca o el destino se pierde repetidamente;
- se pierden teclas, o el EDIT parpadea o queda desfasado de su celda;
- mover la selección pasa de unos 50 ms por cuadro de forma sostenida, ya con el repintado por zonas;
- Camilo la encuentra peor de usar que la 0.3.
El modelo (`edicion`, `vista`, `rejilla` sin dibujo), el hilo, `EditProc` y `acciones` sirven igual en A; solo se cambia la capa de dibujo.

### 5.4 Registro y diagnóstico
- **Producto 0.4**: sin banderas. Escribe un registro mínimo en `%LOCALAPPDATA%\NestTubo\registro.txt` (versión de Windows, HRESULT de cada creación, tipo de destino, DPI, familia de fuente, sesión remota, recreaciones del destino; no teclas ni datos). Se sobrescribe en cada arranque.
- **Build de diagnóstico** (solo pruebas y sonda, con `-DNT_DIAG`): `--log` (teclas y ms por cuadro), `--hwnd`, `--software`, `--perder-destino=N`, `--sin-animacion`, `--fuente=`.

---

## 6. Pruebas

### 6.1 Nativas (Linux, sin wine)
- `./compilar.sh pruebas` sigue terminando en "FALLOS: 0" y ahora también corre `pruebas_panel.cpp` sobre `edicion`, `vista`, `rejilla`, `distribucion` y `habilitado`:
  - nombre repetido; traer del catálogo; crear con los valores iniciales; copiar el perfil de arriba; fila vacía siempre; quitar un rango de perfiles y anotarlos; `piezas_que_usan`;
  - texto de fin de cálculo con error, cortado, cancelado y no demostrado;
  - mismo color para un perfil aunque `preparar` salte otro sin piezas;
  - «no cabe» y útil iguales a `preparar()` en `ejemplos/` y en bordes (largo igual al útil, coma, punto, "1.234", ángulo 90);
  - Inicio, Fin, Ctrl+Inicio/Fin, RePág, AvPág, Mayús+flechas, Supr con y sin filas marcadas, Tab en la última celda, resto de rueda;
  - distribución a 860×420, 900×440 y 1280×800: al menos 8 filas en Pedido a 900×440, ningún rectángulo fuera, ningún perfil omitido con 12 perfiles;
  - durante el cálculo solo están habilitados Cancelar y cambiar de pantalla.
- Pruebas del laboratorio en verde (CLAUDE.md, "Antes de entregar algo").

### 6.2 Bajo wine, con prefijo propio (nunca el compartido)
PID guardado al lanzar, se mata por número, `wineserver -k` del prefijo propio. Con el build de diagnóstico y `--fuente=DejaVu Sans`:
1. `./compilar.sh datos`: "FALLOS: 0".
2. **Tecleo fijo**: xdotool escribe 15 filas de piezas y 3 de perfiles con Tab, Enter, flechas, Esc y RePág, a 5, 12 y 20 ms por tecla; Ctrl+S; compara el `.ntb` con el esperado salvo el sello. Variantes: desplazar con la rueda mientras se escribe; redimensionar la ventana con la celda abierta; repetir con `--perder-destino=5`. Un carácter perdido es fallo.
3. **Tildes y ñ**: abrir un `.ntb` con "Peña", "Ángulo", "Tubería"; editar otra celda; guardar; reabrir; comparar archivo y captura.
4. **Teclado y foco**: todos los atajos de §2.3; durante el cálculo los de archivo y Supr no hacen nada; tras Calcular, Cancelar y Cambiar carpeta el teclado responde sin clic; Supr siempre pregunta.
5. **Exportar** `ejemplos/ejemplo.ntb` desde la ventana: idéntico byte a byte a `build/pruebas --salidas`; cada archivo abierto con un lector independiente y mirado como imagen.
6. **Archivos viejos**: `.ntb` y `config.ini` de la 0.3 abren igual.
7. **Avisos con la celda abierta**: desconectar N: mientras se escribe; el aviso espera y no confirma lo escrito a medias.
8. **Instancia única**: con la ventana abierta, lanzar otro `.exe` con un `.ntb`: lo abre la primera, pasando por "¿Guardar los cambios?".

### 6.3 Capturas que se miran
A 96 y 144 DPI, en 860×420 y 1280×800 DIP: Pedido (vacío, ejemplo, celda abierta, «no cabe», 12 perfiles); Resultado (vacío, calculando, vigente, viejo, no demostrado, tiempo agotado, cancelado, ERROR, 12 perfiles, plan largo desplazado al final, Ver detalle); Perfiles (carpeta en verde y en cada estado rojo); línea de estado. En cada una: ningún texto fuera de su tarjeta, ningún perfil omitido, mismo color por perfil en las tres pantallas, ningún rótulo de depuración.

### 6.4 Lo que wine no demuestra

| No se demuestra | Cómo se cubre |
|---|---|
| Velocidad de D2D y DEFAULT frente a SOFTWARE | la sonda lo mide sola |
| Segoe UI, ClearType, "…" | capturas de la sonda |
| Parpadeo o desfase del EDIT | pregunta 2 de la sonda |
| Tildes muertas escritas a mano | Camilo escribe "Peña Ángulo" en la sonda |
| Recreación real del destino | `--perder-destino`; en Windows, suspender (opcional) |
| Sesión remota | registro; solo si la usa |
| Cuelgue real de SMB, diálogos de Windows | igual que hoy (docs/carpeta-de-datos.md) |
| Lector de pantalla y contraste alto | no se cubre: se pierde frente a la ListView y se dice; contraste alto avisa (§2.8) |

---

## 7. Entrega por etapas

### 7.1 Etapa 1: sonda (`NestTubo-sonda.exe`)
- `.exe` aparte, build de diagnóstico, en **su propio ZIP** (el `.exe` y un LEEME corto; listado y de menos de 30 MB).
- **No usa el mutex de instancia única, no lee ni escribe `config.ini`, no toca la carpeta de datos ni el catálogo.** Convive con la 0.3 abierta. Su única escritura es `%LOCALAPPDATA%\NestTubo\sonda.txt`.
- Muestra las tres pantallas con `ejemplos/ejemplo.ntb` incrustado: Pedido con cuadrícula y EDIT reales, Resultado calculado con el motor real, Perfiles con el diagrama.
- **Al arrancar mide sola** `DEFAULT` y `SOFTWARE` (unos segundos de redibujado de prueba) y se queda con el más rápido; lo anota.
- **Prueba de aviso con la celda abierta**: 20 s después de abrir la primera celda encola un aviso de prueba por la misma vía que los avisos del hilo de red (`ocupado()`/`revisar_pendientes`); debe salir solo al cerrar la celda. No toca la red.
- Botones al pie: **"Abrir carpeta del informe"** y **"Copiar informe"** (al portapapeles).
- Preguntas a Camilo (opción múltiple):
  1. ¿Abrió y se ve como la captura que te mando? (captura de cada pantalla)
  2. Escribe rápido unas filas con tildes y ñ. ¿Se perdió alguna tecla? ¿El aviso de prueba te interrumpió mientras escribías?
  3. ¿Se siente rápido al moverte y cambiar de pantalla? (pega el informe)
  - Opcional: suspender el equipo con la sonda abierta y volver.
  - Dos datos: ¿usas a veces un segundo monitor o un proyector? ¿entras a ese equipo por Escritorio Remoto?

### 7.2 Etapa 2: NestTubo 0.4
- `main.cpp` reescrito sobre el panel con las unidades de §3, conservando los modelos de main.cpp.
- Pasa §6.1 a §6.3 y la lista "Antes de entregar algo" de CLAUDE.md completa (núcleo en verde; `.exe` bajo wine con capturas miradas; salidas del propio `.exe` abiertas y miradas; ZIP con `.exe`, fuente, `build.bat` y LEEME, listado y < 30 MB; qué se probó y qué no, y lo que solo se vio en wine).
- **Se entrega al lado de la 0.3, sin reemplazarla.** El LEEME dice:
  - que los `.ntb`, el catálogo y `config.ini` sirven en las dos y se puede volver a la 0.3 en cualquier momento;
  - que no se pueden tener las dos abiertas a la vez (abrir una le pasa el archivo a la otra);
  - los atajos nuevos; que teclear reemplaza la celda; que Supr quita filas y siempre pregunta; que ya no se arrastra el ancho de columna;
  - que los diálogos se ven claros.

---

## 8. Fuera de alcance
- Pegar varias filas desde Excel y multiplicar cantidades por conjuntos (CLAUDE.md: se ofrecen después de la primera versión). La selección por rangos no les cierra la puerta.
- Deshacer de varios pasos (quedan Esc y el Ctrl+Z del EDIT).
- La curva "Sobrante por barra".
- Celdas pintadas en rojo por dato mal escrito antes de calcular (idea de los jueces; no está en la maqueta). Manda el cuadro "Corrige esto antes de calcular", como hoy.
- Hora del cálculo en Resultado (la maqueta la inventó; no se guarda hoy).
- Historia de los últimos avisos en la línea de estado.
- Aviso al renombrar un perfil que usan piezas (sí entra el aviso al **quitarlo**, §2.7).
- Soltar un `.ntb` sobre la ventana abierta (hoy funciona soltarlo sobre el `.exe` y la instancia única, que siguen).
- PerMonitorV2 y WM_DPICHANGED; se reconsidera si Camilo dice que usa un segundo monitor o proyector.
- Tema de contraste alto, lector de pantalla, columnas de ancho ajustable, barra de título oscura.
- Guardar la pantalla o el tamaño en `config.ini`.
- Una segunda interfaz de respaldo dentro del mismo `.exe`.

---

## 9. Preguntas abiertas para Camilo
1. **Medidas del perfil**: ¿se editan solo en la pantalla Perfiles (recomendado) o también en una franja de Pedido (§2.9)?
2. **Al terminar de calcular**: ¿te parece bien que pase solo a Resultado si no tocaste nada desde F5, y que si seguiste escribiendo te deje donde estás?
3. **Teclear reemplaza la celda** (como Excel) en vez de buscar fila por letra: ¿de acuerdo?
4. ¿Usas un segundo monitor o proyector, o entras por Escritorio Remoto? (decide PerMonitorV2 y cuánto pesa la velocidad por red).

---

## 10. Críticas: cómo se resolvió cada una

**Revisión 1**
1. Supr / Ctrl+Supr contradictorios (alta): aplicada, regla única §2.3/§2.5/§3.5; se elige la de la revisión 2 (Supr siempre quita con pregunta, no vacía).
2. Instancia única no ubicada (alta): aplicada en §3.10 y §6.2.8; se anula la nota del inventario; reinventario de main.cpp:1394-1462 exigido en la cabecera.
3. config.ini: aplicada, no se guarda nada (§3.11).
4. WM_DROPFILES y WM_QUERYENDSESSION: soltar sobre la ventana queda fuera (§8); WM_QUERYENDSESSION precisado en §3.10.
5. Cadena de fuentes: aplicada, un solo sitio §4.6, `--fuente` solo en diagnóstico.
6. Nombres de pruebas: aplicada, `pruebas_panel.cpp` desde `./compilar.sh pruebas`.
7. Fallo de fábricas por `cuadro()`: aplicada, `MessageBoxW` directo con ventana nula; 0.3 al lado (§5.1, §7.2).
8. Esc como Cancelar: aplicada, Esc no cancela.
9. Cuándo se cambia de pantalla: aplicada, regla única §2.6.
10. Lista de atajos en EditProc: aplicada, tabla única §2.3 y `ejecutar()` en §3.8.
11. Ancho de columnas: aplicada, se pierde y se dice; "…" y texto en la línea de estado.
12. Perfiles sin piezas y ERROR: aplicada (§2.6).
13. Exportar con ERROR: aplicada, encendido, comprobado en main.cpp:775-782.
14. EDIT con cambio de pantalla/tamaño: aplicada (§2.5) y prueba en §6.2.2.
15. Razón del DC: aplicada, reescrita y marcada no verificada (§4.1).
16. PerMonitorV2: no aplica tal cual, porque se adopta la crítica 6 de la revisión 2 (quitarlo); sí se aplica WM_GETMINMAXINFO con el DPI y el manifiesto con comctl32 v6 sin cambios.
17. Sonda y 0.3: aplicada (sin mutex, sin config.ini, ZIP propio).
18. "Al día" y punto girando: aplicada, texto nuevo declarado y sin giro.
19. Indicador: aplicada (§2.6, §4.4).

**Revisión 2**
1. Sonda con banderas y mucho trabajo (alta): aplicada, mide sola, botones de informe, tres preguntas, suspensión opcional.
2. Salto a Resultado roba el lugar: aplicada (§2.6).
3. Supr vacía sin preguntar: aplicada.
4. Teclear reemplaza: aplicada con valor anterior en la línea de estado; queda como pregunta 3.
5. Contraste alto: aplicada, aviso una vez (§2.8).
6. PerMonitorV2: aplicada, fuera de la 0.4; pregunta 4.
7. Novedades no pedidas: aplicada en parte. Quedan fuera la hora del cálculo, las celdas rojas, la historia de avisos y el aviso al renombrar. **Se mantienen** Pieza seleccionada, columna Útil, barras de margen dibujadas y «no cabe» en Pedido, porque están en la maqueta que Camilo aprobó; el riesgo de contradecir al cálculo se cubre con la prueba de §6.1.
8. Banderas en el producto: aplicada, solo en el build de diagnóstico (§5.4).
9. Sonda sin prueba de avisos: aplicada (§7.1).
10. 0.3 como salida: aplicada (§7.2).
