# Diseño corregido: carpeta de datos de NestTubo

## Qué cambia respecto del borrador

- **Un solo equipo.** CLAUDE.md dice, en la parte de entorno y marcado como dicho por Camilo, que "No tiene Python ni otro equipo". Que use "varios PCs" no está en ningún requisito. Por eso se quitan la copia "(copia de PC fecha)", la comparación de fechas y lo demás que solo sirve con dos equipos. El registro de cambios del catálogo ya está escrito y probado (commit dc986de) y se queda. Con un solo PC hace lo mismo que copiar el archivo entero. Si algún día hay un segundo equipo, el daño queda limitado a los perfiles que se tocaron.
- **Guardar escribe primero en este equipo** cuando el destino es de red. La copia a la red la hace un hilo aparte. Si se corta la red, se congela algo o se mata el proceso, el trabajo ya está en disco.
- **Nada automático toca la red desde la ventana, y ninguna acción espera 3 s.** La ventana usa el último estado que informó el hilo.
- **El catálogo se separa de la tabla en memoria.** Cada edición a mano se anota al confirmar la celda, solo el campo que se editó. Ya no hay "puntos de guardado" ni lista de perfiles tocados.
- `perfil_si_falta` y la fila nueva de perfiles buscan primero en el catálogo.
- **Si perfiles.ntb existe pero no se puede leer, no se toma como vacío.**
- **Los trabajos pendientes van a una carpeta local que copia la ruta del destino.** Así no hace falta pendientes.ini, no hay choques de nombres y cambiar de carpeta no deja nada huérfano.
- **Abrir usa la versión pendiente si existe.** Sin red, los diálogos empiezan en la carpeta local.
- La copia local y los pendientes pasan a `%LOCALAPPDATA%`.
- No se crean las subcarpetas Trabajos ni Exportados.

Si se quiere confirmar lo de un solo equipo, sirve una pregunta de opción múltiple a Camilo: "¿Usarás NestTubo en más de un equipo? a) solo en este b) en varios". Con b) vuelve la detección de conflictos (ver Límites).

## Decisiones finales

### Archivos y rutas

1. **[main.cpp] Dónde va cada cosa.**
   - `config.ini` sigue en `%APPDATA%\NestTubo` y gana la clave `carpeta_datos`.
   - En `%LOCALAPPDATA%\NestTubo` (`CSIDL_LOCAL_APPDATA`) van tres cosas:
     - `perfiles.ntb`: la copia local del catálogo, con el formato de hoy.
     - `perfiles_pendientes.ntb`: los cambios que aún no llegan a la carpeta, con `a_texto_cambios`.
     - La carpeta `Sin conexión\`: aquí se escriben primero los trabajos que van a la red (en adelante P).
   - Migración desde 0.2: al arrancar, si no hay copia local y sí existe `%APPDATA%\NestTubo\perfiles.ntb`, se copia esa.
   - Motivo: con perfil móvil o AppData redirigida, la "copia local" estaría en la red caída. Cuesta unas pocas líneas.

2. **[trabajo.cpp] Funciones puras de ruta, en UTF-8.**
   - `ruta_pendiente(raiz, destino)`:
     - `\\srv\rec\a\b.ntb` → `raiz\red\srv\rec\a\b.ntb`
     - `Z:\a\b.ntb` → `raiz\Z\a\b.ntb`
     - cualquier otra forma, incluidas `\\?\` y las rutas relativas → `""`
   - `bool destino_de_pendiente(raiz, ruta, destino&)` hace lo inverso. Devuelve true solo si `ruta` está bajo `raiz`, sin distinguir mayúsculas ASCII.
   - Dos "Pedido 1.ntb" de carpetas distintas caen en archivos distintos sin necesidad de un índice.

3. **[main.cpp] Rutas de red.**
   - `es_remota(ruta)` es verdadero si la ruta empieza por `\\` (pero no por `\\?\` ni `\\.\`), o si `GetDriveTypeW(L"X:\\") == DRIVE_REMOTE`.
   - `raiz_red(ruta)` devuelve `\\srv\rec` o `X:\`.

4. **[main.cpp] Una sola regla de lectura.**
   - Un archivo que no existe (`ERROR_FILE_NOT_FOUND`, con su carpeta respondiendo) cuenta como vacío.
   - Uno que existe pero no se puede abrir, se lee incompleto o `de_texto` no lo entiende es un error, y no se escribe nada a partir de él.
   - `leer_archivo` devuelve el código de error y abre con `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`, para no estorbar el `MoveFileExW` de otra ventana.

### Catálogo

5. **[main.cpp] El catálogo es una lista aparte.** `app.catalogo` (`vector<PerfilTxt>`) vive separado de `app.trabajo.perfiles`.
   - Al arrancar: `app.catalogo = aplicar_cambios(copia local, pendientes)`, por si se cortó entre las dos escrituras. La tabla = `app.catalogo`, como hoy.
   - **Nuevo:** tabla = `app.catalogo`, y la barra de estado dice "Trabajo nuevo con los perfiles del catálogo."
   - **Abrir:** tabla = perfiles del trabajo; el catálogo no se toca.

6. **[main.cpp] `registrar(const CambiosCatalogo& c)` es el único punto que cambia el catálogo.**
   - Trabaja bajo el mutex con nombre `NestTubo-local`, que solo protege operaciones en disco local y nunca se tiene durante una operación de red. Si se recibe `WAIT_ABANDONED`, cuenta como obtenido.
   - Pasos:
     1. Si hay carpeta de datos, lee los pendientes y les junta `c` con `juntar_cambios`. Si el archivo de pendientes existe y no se puede leer, los pendientes pasan a ser todo el catálogo local como "cambiados".
     2. Lee la copia local; si no se puede leer, parte de `app.catalogo`, nunca de vacío. Le aplica `c`.
     3. Escribe los dos archivos con `escribir_archivo` y deja el resultado en `app.catalogo`.
     4. Pide una pasada al hilo.
   - Se llama al confirmar cada celda. Así apagar Windows o matar el proceso no pierde ediciones de perfiles, y responder "No" a "¿Guardar los cambios?" no afecta al catálogo.
   - `guardar_catalogo()` desaparece de `guardar()` y de `WM_CLOSE`.

7. **[trabajo.cpp] Funciones puras nuevas.** Se borran `cambios_de_tabla` y su prueba.
   - `int buscar_perfil(v, nombre)` usa `clave_perfil`; es el `buscar` interno, ahora visible.
   - `CambiosCatalogo cambio_por_campo(catalogo, fila, col, valor)`, para las columnas 1 a 6:
     - Si la clave está en el catálogo, se manda la fila **del catálogo** con solo esa columna cambiada.
     - Si no está, se manda la fila de la tabla con ese valor.
     - Así, corregir la cara en un trabajo viejo no sube su barra vieja.
   - `CambiosCatalogo cambio_por_nombre(catalogo, fila, viejo, nuevo, viejo_sigue_en_tabla)`:

     | Caso | Resultado |
     |---|---|
     | `nuevo` vacío | nada |
     | Misma clave (solo cambian mayúsculas o espacios) | la fila del catálogo con el nombre nuevo, sin quitar nada |
     | `viejo` vacío y `nuevo` en el catálogo | nada (la ventana trae la fila del catálogo) |
     | `viejo` vacío y `nuevo` fuera del catálogo | se manda la fila de la tabla |
     | `nuevo` ya está en el catálogo | nada; la fila conserva sus valores en este trabajo |
     | `viejo` en el catálogo | se quita `viejo` (salvo que `viejo_sigue_en_tabla`) y se manda la fila del catálogo de `viejo` con el nombre `nuevo`, con los valores del catálogo |
     | `viejo` fuera del catálogo | se manda la fila de la tabla |

   - `std::vector<std::string> perfiles_distintos(a, b)`: nombres que están en las dos listas con algún campo distinto, comparando sin espacios en los extremos.
   - `PasadaCatalogo pasada_catalogo(lectura, compartido, local, pendientes)`, con `lectura` en {ok, no_existe, error}, devuelve `{escribir, resultado}`:
     - error → no se escribe;
     - no_existe → `resultado = aplicar_cambios(local, pendientes)` y se escribe;
     - ok → `resultado = aplicar_cambios(compartido, pendientes)` y se escribe solo si hay pendientes.

8. **[main.cpp] `cerrar_edicion` en la tabla de perfiles.** Se guarda el valor viejo antes de asignar el nuevo.
   - **Nombre repetido:** si la clave nueva ya está en otra fila, no se acepta. La celda vuelve a su valor y la barra de estado dice "Ya hay un perfil "X" en la fila N". Esto evita de raíz quitar una fila repetida y borrar el perfil del catálogo.
   - **Nombre escrito en una fila sin nombre:**
     - Si el nombre está en el catálogo, la fila toma todos los valores del catálogo, no se registra nada y la barra de estado dice "Perfil "X" traído del catálogo".
     - Si no está, se ponen los valores iniciales como hoy y se registra la fila.
   - **Otro cambio de nombre:** `registrar(cambio_por_nombre(...))`.
   - **Columnas 1 a 6 de una fila con nombre:** `registrar(cambio_por_campo(...))`.

9. **[main.cpp] `perfil_si_falta`.** Si el perfil no está en la tabla pero sí en el catálogo, copia la fila del catálogo a la tabla sin registrar nada, y avisa "traído del catálogo". Solo un nombre que no está en ninguno de los dos recibe los valores iniciales, y esa fila se registra.

10. **[main.cpp] Quitar perfil.**
    - La pregunta pasa a decir: "¿Quitar N perfiles? También se quitan del catálogo y no saldrán en los trabajos nuevos."
    - Por cada fila quitada se registra que se quitó el perfil, pero solo si ninguna fila que queda tiene esa clave.

11. **[main.cpp] `puede_descartar`** no pregunta si el trabajo no tiene archivo ni piezas: las ediciones de perfiles ya están en el catálogo.

### Trabajos

12. **[main.cpp] `guardar()` con destino de red.**
    - Pasos:
      1. Bajo `NestTubo-local`, escribe `ruta_pendiente(P, destino)` con `escribir_archivo`, creando las carpetas locales que falten.
      2. `app.archivo = destino`, el trabajo queda sin `*` y se actualiza el título.
      3. Pide una pasada. La barra de estado dice "Guardado en este equipo; copiando a <destino>...".
    - Si la raíz del destino no estaba en línea según el último estado, sale además **un solo cuadro por sesión**: "No hay conexión con <raíz>. El trabajo quedó guardado en este equipo y se copiará solo cuando vuelva la conexión, con NestTubo abierto."
    - Si escribir el pendiente falla, o la ruta en P pasa de `MAX_PATH`, sale el error de hoy y el trabajo sigue con `*`.
    - Si el destino es local, se escribe como hoy.

13. **[main.cpp] Guardar dentro de P.** Si la ruta que devuelve el diálogo está dentro de P (se guardó sin conexión en la carpeta local), el destino real es `destino_de_pendiente(...)`. En la clave `carpeta` de config se guarda la carpeta del destino, nunca la de P.

14. **[main.cpp] `abrir(ruta)`**, tanto desde el diálogo como desde la línea de órdenes:
    - Si `ruta` está dentro de P, se lee esa y `app.archivo` pasa a ser su destino.
    - Si `ruta` es de red y tiene pendiente, se lee el pendiente con `app.archivo = ruta`, y la barra de estado dice "Abierta la versión guardada en este equipo, más nueva que la de <ruta>".
    - Si no, se lee como hoy.

### Hilo de red

15. **[main.cpp] Un solo hilo de vida larga.**
    - Se crea al arrancar y se suelta con `detach`. Nunca `join` ni `std::async`, porque los dos esperan a la red y congelan la ventana.
    - Tiene una cola con mutex y una bandera de "pasada pedida": varias peticiones seguidas se juntan en una sola pasada.
    - Las tareas llevan copias de las rutas. Los resultados vuelven con `PostMessageW(WM_APP_RED)` y un struct en el heap que libera la ventana.
    - Hay dos tareas: `PASADA` y `EXAMINAR(carpeta)`.
    - Piden una pasada:
      - un temporizador cada 30 s, si el hilo está libre;
      - el arranque;
      - cada `registrar()`;
      - cada Guardar a la red;
      - el cierre.

16. **[main.cpp] `PASADA`.** Se toma el mutex con nombre `NestTubo-sincronizar` con espera 0. Si otra ventana de NestTubo está en una pasada, esta se salta.
    1. Con `GetFileAttributesW` mira si responden las raíces de: la carpeta de datos, `carpeta`, `carpeta_exportar`, `app.archivo` y los destinos pendientes. Informa el estado de cada una.
    2. **Catálogo**, si hay carpeta de datos y respondió:
       1. Bajo `NestTubo-local`, lee los bytes de los pendientes.
       2. Lee `<carpeta>\perfiles.ntb` y lo clasifica en ok, no_existe o error.
       3. Aplica `pasada_catalogo`. Si dio error: no escribe nada, conserva los pendientes y la etiqueta muestra el error.
       4. Si toca, escribe con `escribir_archivo`.
       5. Bajo `NestTubo-local`: borra los pendientes solo si sus bytes no cambiaron. Luego deja la copia local en `aplicar_cambios(resultado, pendientes que queden)` y la escribe solo si cambió.
       6. Pide a la ventana que recargue `app.catalogo`. La tabla no se toca.
    3. **Trabajos.** Recorre P buscando `*.ntb` (ignora `*.tmp`). Por cada uno cuya raíz respondió:
       1. Lee sus bytes.
       2. Crea las carpetas que falten en el destino.
       3. Lo escribe con `escribir_archivo`: temporal en la carpeta del destino y `MoveFileExW`. Nunca `CopyFile` ni `MoveFileEx` entre discos. Ante `ERROR_SHARING_VIOLATION` o `ERROR_ACCESS_DENIED` reintenta hasta 3 veces, con 200 ms entre intentos.
       4. **Si funcionó:** bajo `NestTubo-local`, borra el pendiente solo si sus bytes no cambiaron. Si cambiaron, es que se guardó otra vez y va en la siguiente pasada. También borra las carpetas vacías de P.
       5. **Si falló:** vuelve a mirar la raíz. Si no responde, es "sin conexión". Si responde, es un error real con su código, y se informa una vez por pendiente y código.
    4. Informa cuántos pendientes quedan y marca un evento de "pasada terminada".

17. **[main.cpp] `WM_CLOSE`.** Después de `puede_descartar`, si hay pendientes, pide una pasada y espera ese evento hasta 3 s con `WaitForSingleObject`. Luego cierra de todos modos: lo que falte ya está en este equipo y se copia la próxima vez que se abra NestTubo.

18. **[main.cpp] Variable de entorno `NESTTUBO_DEMORA_RED_MS`, solo para pruebas.** El hilo duerme ese tiempo antes de cada operación de red. Si la variable no está, no hace nada.

### Diálogos, etiqueta y avisos

19. **[main.cpp] Estado por raíz y carpeta inicial de los diálogos.**
    - La ventana guarda el estado de cada raíz de red: desconocido, en línea, sin conexión o error. "Desconocido" cuenta como sin conexión.
    - Antes de abrir Abrir o Guardar: si la carpeta inicial es de red y su raíz no está en línea, la carpeta inicial pasa a ser su equivalente en P (se crea en local).
      - En Guardar, esa ruta va también en `lpstrFile`, con el nombre del archivo.
      - El título del diálogo dice "Sin conexión con <carpeta>: lo que guardes aquí se copiará cuando vuelva la red" (en Abrir: "se ven los trabajos guardados en este equipo").
    - Exportar y "Carpeta de datos...", en el mismo caso, no pasan la ruta de red (tampoco a `SHCreateItemFromParsingName`) y empiezan en Documentos.

20. **[main.cpp] Carpeta inicial de Abrir y Guardar.**
    - Es la clave `carpeta` de config; si está vacía, la carpeta de datos.
    - Al elegir la carpeta de datos, `carpeta` y `carpeta_exportar` pasan a ser esa carpeta.
    - No se crean subcarpetas.

21. **[main.cpp] Botón "Carpeta de datos..." y etiqueta a su derecha.**
    - La etiqueta usa `SS_ENDELLIPSIS` y se pone roja con `WM_CTLCOLORSTATIC` cuando la carpeta no está en línea o hay un error.
    - El estado va primero, para que una ruta larga no lo corte:
      - "Carpeta de datos: este equipo"
      - "Comprobando \\srv\nesttubo..."
      - "Carpeta de datos: \\srv\nesttubo"
      - "Sin conexión con \\srv\nesttubo: 2 trabajos y cambios de perfiles esperando en este equipo"
      - "Error al copiar a \\srv\nesttubo: 1 trabajo sin copiar"
    - Cuando un pendiente se copia, la barra de estado dice "Copiado a <destino>".
    - Si la conexión se pierde en medio de una pasada, no se abre ningún cuadro: solo cambian la etiqueta y la barra de estado, para no quitarle el foco mientras escribe.
    - Los errores reales abren un `MessageBox` con el texto de `FormatMessageW` y la ruta del pendiente en P. Nunca hay dos cuadros abiertos a la vez.

22. **[main.cpp] Elegir una carpeta F distinta de la actual.**
    - Se desactiva el botón y se encola `EXAMINAR(F)`.
    - Según lo que encuentre:
      - **Error al leer:** "No se pudo leer F\perfiles.ntb: <motivo>. La carpeta de datos no cambió."
      - **No existe:** el nuevo pendiente es todo `app.catalogo` como "cambiados".
      - **Existe:**
        - Primero se copia la copia local a `perfiles antes de cambiar de carpeta.ntb`.
        - Si `perfiles_distintos` no está vacío, se pregunta, listando hasta 15 nombres: "Estos perfiles tienen valores distintos en la carpeta y en este equipo: ... ¿Usar los de la carpeta? Sí = los de la carpeta. No = los de este equipo. Cancelar = no cambiar de carpeta."
        - El resultado es `unir_catalogos(S, local)` o `unir_catalogos(local, S)`, y el nuevo pendiente es todo ese resultado como "cambiados".
    - Si no hubo error ni Cancelar, bajo `NestTubo-local` se escriben la copia local y el pendiente. Este reemplaza al de la carpeta anterior, que ya está incluido en la copia local. Luego se guarda `carpeta_datos` y se pide una pasada.
    - Los trabajos pendientes no se tocan, porque su destino es una ruta completa.

## Lo que se descarta de las críticas

- **Copias "(copia de PC)", fecha base y aviso de conflicto en un cuadro** (pérdida 7 y 8, Windows 5): no hay segundo equipo. Con uno solo, el destino solo cambia por NestTubo, y Abrir usa el pendiente.
- **Guardar el valor anterior de cada campo y "solo borrar si la fila sigue igual"** (pérdida 4): solo sirve con dos equipos editando a la vez.
- **Pendiente que se queda en un PC que no se vuelve a abrir** (pérdida 12, caso C): es el mismo y único PC, y la etiqueta muestra la cuenta.
- **`WM_QUERYENDSESSION` para el catálogo** (pérdida 9): sobra, porque cada celda se anota al confirmarla. Un trabajo sin guardar al apagar se pierde igual que hoy y no tiene que ver con la carpeta de datos.
- **Espera de 3 s por acción** (punto 4 del borrador; la crítica de Windows la conservaba en Guardar): como el pendiente se escribe primero, nada tiene que esperar salvo el cierre.
- **Lista de códigos de `GetLastError` para decidir "red caída"** (Windows 6): volver a mirar la raíz es más simple y cubre los códigos raros de una letra de unidad desconectada.
- **Avisar al volver la red qué perfiles del catálogo cambiaron** (Windows 8): con un solo PC, la copia local nunca es más vieja que la de la carpeta. De ese hallazgo se queda solo la revisión periódica de la conexión.
- **Guardar la carpeta como UNC con `WNetGetUniversalNameW`** (Windows 8): mezclaría dos formas de escribir la misma ruta en los pendientes.
- **Nombre de la copia de conflicto con / o :** (Windows 6b): ya no hay copia de conflicto.
- **"`a_texto_cambios` sin definir"** (nota de Windows): ya están definidas en dc986de.

## Fuera de este cambio (anotar, no hacer ahora)

- Pedir confirmación al apagar Windows con un trabajo sin guardar (`ShutdownBlockReasonCreate`).
- Que `minusculas` y `minus` pasen a minúscula Á, É, Í, Ó, Ú y Ñ. Hoy afecta igual a `preparar`.
- Renombrar un perfil no renombra las piezas que lo usan (igual que hoy).

## Pruebas nativas (pruebas.cpp; `./compilar.sh pruebas` debe terminar en "FALLOS: 0")

1. **`cambio_por_campo` en un trabajo viejo:** el catálogo tiene "Tubo 40x40" con barra 6400 y cara 40; la tabla tiene barra 6000 y cara vacía. Al editar la cara, se manda barra 6400 y la cara nueva.
2. **`cambio_por_campo` con un perfil fuera del catálogo:** se manda la fila de la tabla con el campo editado.
3. **`buscar_perfil`:** encuentra "  tubo 40X40 " en un catálogo con "Tubo 40x40".
4. **`cambio_por_nombre`:** los siete casos de la decisión 7, uno por prueba. Entre ellos, que cambiar solo mayúsculas no quita nada y que no se quita el nombre viejo si sigue en la tabla.
5. **`juntar_cambios` en orden:**
   - Quitar X y luego cambiar X deja X solo en "cambiados", y `aplicar_cambios` lo conserva.
   - Al revés, X queda solo en "quitados" y se borra.
6. **`aplicar_cambios` se puede repetir:** `aplicar(aplicar(S, c), c) == aplicar(S, c)`. Es lo que permite reintentar una pasada que quedó a medias.
7. **`pasada_catalogo`:**
   - error → no escribe;
   - no_existe, con 30 perfiles locales y 1 pendiente → 31 perfiles y escribe;
   - ok sin pendientes → no escribe;
   - ok con pendiente → compartido más el pendiente.
   - Además: en no_existe el resultado nunca tiene menos perfiles que la copia local.
8. **`perfiles_distintos` y `unir_catalogos`** en los dos sentidos: gana la primera lista y se agregan los perfiles que solo están en la otra.
9. **`a_texto_cambios` y `de_texto_cambios`:** ida y vuelta con tildes y campos vacíos. Un archivo vacío da error.
10. **`ruta_pendiente` y `destino_de_pendiente`:**
    - ida y vuelta con UNC y con letra de unidad;
    - dos "Pedido 1.ntb" en carpetas distintas dan rutas distintas;
    - `\\?\C:\x` y una ruta relativa dan `""`;
    - P escrita con otras mayúsculas se reconoce como P;
    - una ruta fuera de P devuelve false.
11. **Corte entre las dos escrituras:** `aplicar_cambios(copia local sin el cambio, pendientes con el cambio)` lo incluye.

## Pruebas bajo wine (con capturas, como pide CLAUDE.md)

**Preparación.** Una letra `N:` de tipo red:
- `ln -s $SRV $WINEPREFIX/dosdevices/n:`
- `wine reg add 'HKLM\Software\Wine\Drives' /v n: /d network /f`

Antes de nada, comprobar que la etiqueta o `es_remota` la ven como de red. Si no, usar `$WINEPREFIX/dosdevices/unc/srv/rec` con `\\srv\rec`, y decir cuál se usó. "Sin conexión" es `mv $SRV $SRV.off`, que falla al instante; la conexión vuelve deshaciendo el `mv`. Los archivos se comprueban desde Linux con `cmp` o `grep`.

1. **Sin carpeta de datos:** se comporta como 0.2 (abrir el ejemplo, calcular, guardar en local) y la etiqueta dice "este equipo". Migración: con solo `%APPDATA%\NestTubo\perfiles.ntb`, la tabla sale con esos perfiles y aparece la copia en LOCALAPPDATA.
2. **Elegir `N:\datos` vacía:** `perfiles.ntb` queda igual a la copia local, y `carpeta` y `carpeta_exportar` pasan a ser `N:\datos`.
3. **Elegir una carpeta con un `perfiles.ntb` distinto:** sale la pregunta (captura). Con "Sí" quedan los valores de la carpeta más los perfiles que solo estaban en este equipo, y existe el respaldo.
4. **Cierre forzado:** editar una celda y hacer `wineserver -k` sin cerrar. La copia local ya tiene el cambio; al volver a abrir está en la tabla y llega a la carpeta.
5. **Trabajo viejo:**
   - Abrir un .ntb donde "Tubo 40x40" tiene barra 6000, con 6400 en el catálogo.
   - Escribir una pieza con "Platina 50x5", que está en el catálogo y no en el trabajo: la fila sale con los valores del catálogo y el catálogo no cambia.
   - Editar la cara de Tubo 40x40: el catálogo queda con barra 6400 y la cara nueva.
6. **Perfiles repetidos y quitados:**
   - La pregunta de Quitar perfil muestra el texto nuevo (captura).
   - En un .ntb viejo con una fila repetida, quitar una de las dos no saca el perfil del catálogo.
   - Escribir un nombre que ya está en otra fila se rechaza con el aviso.
7. **Cerrar sin piezas después de editar perfiles:** no pregunta, y los perfiles quedan guardados.
8. **Sin conexión:**
   - La etiqueta pasa a roja en 30 s o menos (captura).
   - Guardar un trabajo de `N:\datos` lo deja sin `*`, sale un solo cuadro y el archivo aparece en `Sin conexión\N\datos\...`. Un segundo Guardar no muestra cuadro.
   - Al volver la conexión, en 30 s o menos: el destino es igual al pendiente, el pendiente se borró, la etiqueta vuelve a la normalidad y los cambios de perfiles hechos sin red llegan a la carpeta.
9. **Trabajo nuevo sin conexión:** Guardar abre el diálogo en la carpeta local, con el título de "Sin conexión" (captura). Se guarda como "Nuevo 1" y, al volver la conexión, existe `N:\datos\Nuevo 1.ntb`.
10. **Abrir con pendiente:** con un pendiente A y un destino B más viejo, abrir `N:\datos\x.ntb` por línea de órdenes muestra A, y la barra de estado lo dice. Abrir sin conexión muestra el pendiente en el diálogo.
11. **Catálogo de la carpeta ilegible:**
    - Reemplazar `perfiles.ntb` por el texto "hola", y en otra corrida por una carpeta con ese nombre.
    - Al editar un perfil, el archivo de la carpeta no cambia, la copia local conserva todos los perfiles y la etiqueta muestra el error.
    - Al arreglarlo, la pasada termina. Si se borra el archivo, se vuelve a crear completo.
12. **Error real:**
    - Con `chmod 555` en la carpeta destino sale un solo cuadro con el error y la ruta del pendiente. El pendiente se conserva y la etiqueta dice "1 trabajo sin copiar".
    - Con `chmod 755`, se copia.
13. **La ventana sigue viva con la red lenta:**
    - Con `NESTTUBO_DEMORA_RED_MS=20000`, pulsar Guardar y enseguida editar una celda con xdotool. La captura a los 2 s muestra lo escrito, y a los 20 s aproximadamente sale "Copiado a".
    - Si se hace `wineserver -k` durante la demora, al reabrir, Abrir da la versión del pendiente, que después se copia.
14. **Dos ventanas a la vez:**
    - `Cliente A\Pedido 1.ntb` y `Cliente B\Pedido 1.ntb`, guardados sin conexión, quedan como dos pendientes distintos. Al volver la conexión, cada destino tiene sus propias piezas.
    - Si se editan perfiles en las dos ventanas, la copia local y la carpeta quedan con las dos ediciones.
15. **Cerrar con red respondiendo "Sí" a guardar:** cuando el proceso termina, el destino ya está actualizado (la espera de 3 s).

## Lo que wine no demuestra (decirlo así al entregar)

- **El cuelgue real de 20 a 60 s de una carpeta compartida (SMB) que no responde.** Tampoco que `GetDriveTypeW` no se bloquee con una letra desconectada. Bajo wine la carpeta caída falla al instante; por eso la prueba 13 usa la demora artificial.
- **Que el diálogo de Windows respete la carpeta local** frente a su regla de "última carpeta usada" de Windows 7 en adelante.
- **Perfiles móviles y AppData redirigida.**

## Límites conocidos

- Si un mismo trabajo se abre una vez como `Z:\...` y otra como `\\srv\rec\...`, se tratan como dos destinos distintos.
- Si `lpstrInitialDir` coincide con el valor que se pasó la primera vez que se usó el diálogo en ese equipo, Windows puede abrir en la última carpeta que recuerda. Es raro, porque las rutas de P no coinciden con ese primer valor.
- Si el usuario mismo navega dentro del diálogo a una carpeta de red caída, la ventana se cuelga igual: lo hace Windows.
- Con dos equipos (hoy no es requisito): el catálogo se junta perfil por perfil. En los trabajos, lo guardado sin conexión ya no pisa un archivo de la red que cambió (ver abajo); lo guardado con conexión sí gana, como en 0.2.

## Cambios tras la revisión independiente (0.3)

Una revisión adversarial del código encontró caminos que perdían datos sin aviso. Quedó así:

- **El catálogo se edita sobre lo que hay en disco.** `registrar` recibe la edición (qué campo de qué perfil) y la calcula, bajo NestTubo-local, sobre la copia local recién leída, nunca sobre el catálogo en memoria de la ventana. Así dos ventanas, o una edición que llega justo después de una pasada, no se deshacen entre sí. La pasada ya no manda su copia del catálogo: la ventana relee la copia local.
- **Nada se escribe si algo no se pudo leer o anotar.** Si la copia local existe y no se puede leer, `registrar` no escribe y lo dice. Si no se pueden anotar los pendientes, tampoco se toca la copia local (si no, la pasada siguiente borraría la edición).
- **Cambiar de carpeta mezcla con la copia de ese momento.** `cambiar_carpeta` recibe el catálogo de la carpeta y la decisión (Sí/No) y vuelve a leer la copia local bajo el bloqueo; lo que otra ventana agregó mientras estaba la pregunta no se pierde.
- **Lo guardado sin conexión no pisa otro archivo.** Junto a cada pendiente va `<archivo>.ntbase` con cómo estaba el archivo de la red cuando la ventana lo leyó o lo escribió (hora y tamaño), o "no existía" para un trabajo nuevo. Al copiar, si el archivo de la red no está así, lo guardado queda al lado como `<nombre> (guardado sin conexión).ntb` (o `... 2`, `... 3`), el trabajo abierto pasa a ser ese y sale un aviso.
- **Una letra que vuelve como disco local.** Abrir usa la versión en espera aunque la ruta ya no sea de red, y guardar directo borra la versión en espera (es más vieja).
- **Nombres y carpetas que no se aceptan.** No se guarda un trabajo llamado `perfiles.ntb` (es el catálogo) ni dentro de la carpeta `Sin conexión\` fuera de las carpetas que repiten una de red; un archivo suelto ahí no cuenta como trabajo esperando.
- **Los cuadros del hilo esperan.** Un aviso o la pregunta de la carpeta elegida no se abren mientras hay una celda en edición u otro cuadro abierto (le quitarían el foco a la celda y confirmarían lo escrito a medias, o cambiarían la tabla debajo de una pregunta). Salen al cerrar la celda o el cuadro.
- **La etiqueta dice a quién se espera.** El informe cuenta los trabajos por raíz sin conexión: "1 trabajo esperando conexión con N:\ (guardado en este equipo)", en rojo, y "copiando" solo cuando esa raíz responde. El título del diálogo sin conexión solo sale cuando el diálogo de verdad empieza en la copia local, con otro texto para Abrir.
- Pruebas nuevas en `pruebas_datos.cpp` para cada caso; `compilar.sh datos` apaga las pasadas solas (`NESTTUBO_INTERVALO_S`) para que no se crucen con las que piden las pruebas.
## Cambios tras la segunda revisión (0.3)

Una segunda revisión adversarial sobre las correcciones anteriores cerró 19 de los 23 hallazgos originales y encontró 10 nuevos, todos corregidos:

- **Una sola ventana a la vez.** NestTubo abre una sola instancia (`CreateMutexW` + `FindWindowW`); al abrir un segundo `.exe`, le pasa su archivo a la ventana ya abierta por `WM_COPYDATA` y se cierra. Así desaparecen todos los casos de "dos ventanas" que la revisión marcó (una ventana que adoptaba el "(guardado sin conexión)" de la otra y lo reemplazaba, etc.). Es la recomendación de la skill `apps-windows-cpp` para un programa que escribe archivos compartidos.
- **El catálogo vigente incluye los pendientes.** `catalogo_vigente` (antes `copia_local`) devuelve la copia local con los cambios pendientes aplicados. Nuevo, el perfil traído del catálogo y cambiar de carpeta usan esa lectura, así que un cambio que quedó solo en los pendientes (corte entre las dos escrituras, o copia que no se pudo reemplazar) ya no se pierde ni se calcula con el valor viejo.
- **La base del próximo Guardar sale de lo que este equipo copió.** Al copiar un trabajo de red, la pasada anota en `conocidos.txt` cómo quedó el archivo. El siguiente Guardar del mismo trabajo abierto usa ese registro, no el `app.sello` que la ventana aún no actualizó. Elimina los falsos "(guardado sin conexión)" cuando un Guardar cae entre la copia y su informe.
- **La base se decide después del diálogo, por dónde estuvo.** Si el diálogo mostró la copia de este equipo (sin conexión), el trabajo se trata como nuevo y no pisa lo que haya en la carpeta real, aunque la red vuelva mientras el diálogo está abierto.
- **Al copiar no se pisa un archivo que apareció.** Donde no había nada, la copia se escribe sin reemplazar; si justo apareció otro archivo con ese nombre, queda al lado como "(guardado sin conexión)".
- **Destinos de red inválidos.** Una ruta como `\\servidor\x.ntb` (sin recurso compartido) se rechaza al guardar y no queda como pendiente eterno; un archivo suelto en `Sin conexión\red\<servidor>` no cuenta como trabajo esperando.
- **Los pendientes se borran solo con la copia al día.** `sincronizar_catalogo` escribe primero la copia local y solo entonces borra los pendientes; si la copia no se puede reemplazar, los pendientes se quedan y una edición posterior no revierte la anterior en la carpeta.
- **Avisos antes de cerrar.** Al cerrar la ventana se procesan los informes de la última pasada y se muestran sus avisos (una copia que quedó "(guardado sin conexión)") antes de destruir la ventana.
- **Un cambio de perfiles que no quedó no se pierde en silencio.** Si `registrar` no pudo guardar, la ventana marca el estado y pregunta "¿Guardar los cambios del trabajo actual?" aunque no haya archivo ni piezas; las barras de estado "Perfil quitado del catálogo" / "agregado" solo salen si el cambio realmente quedó.
- **Un informe saltado no pisa los contadores.** Cuando otra pasada estaba en curso, su informe ya no trae conteos: la ventana conserva el último informe completo y no muestra "copiando" con la raíz sin conexión.

Pruebas nuevas en `pruebas_datos.cpp` para la base desde `conocidos.txt`, los destinos inválidos, el catálogo vigente con pendientes, la copia local que no se puede reemplazar y el cambio que solo está en los pendientes al cambiar de carpeta. `compilar.sh datos`: FALLOS 0.
