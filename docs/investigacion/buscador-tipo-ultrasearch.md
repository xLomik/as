# Investigación: cómo construir un buscador de archivos tipo UltraSearch

> Fecha: 2026-10-22 · Estado: **investigación** (sin código todavía).
> Objetivo: reunir todo lo necesario para construir un programa que haga lo
> mismo que [UltraSearch](https://www.jam-software.com/ultrasearch) de JAM
> Software y que aproveche al máximo CPU, disco y memoria del equipo.

---

## 0. Resumen ejecutivo

- **El truco de UltraSearch (y de Everything, WizFile, SwiftSearch, UFFS):** no
  recorre carpetas con la API normal de Windows. **Lee directamente la Master
  File Table (MFT) de NTFS**, que es una tabla con un registro por cada archivo
  del disco. Leer la MFT completa son unos pocos cientos de MB leídos casi en
  secuencia, en vez de millones de llamadas al sistema. Eso da búsquedas por
  nombre en milisegundos sobre millones de archivos.
- **Requisito duro:** leer la MFT exige **permisos de administrador** (abrir el
  volumen `\\.\C:`). Las alternativas son: pedir UAC al iniciar (lo que hace
  UltraSearch), o instalar un **servicio** que lea la MFT y atienda al programa
  por IPC sin UAC (lo que hacen Everything y UFFS).
- **Para "usar todos los recursos":** E/S asíncrona con colas profundas (IOCP)
  ajustadas al tipo de disco (NVMe ≠ SSD SATA ≠ HDD), parseo en paralelo en
  todos los núcleos, índice compacto en RAM con nombres en un único buffer y
  búsquedas SIMD, actualizaciones en vivo con el **USN Journal**, y para la
  búsqueda en contenido un pipeline lector→buscador con los motores de
  ripgrep (memchr/Teddy/regex con DFA).
- **Stack recomendado:** **Rust** (crates `windows`, `rayon`, `memchr`,
  `aho-corasick`, `regex`, `grep-searcher`) + interfaz con `egui` o Win32
  nativo, compilado como `.exe` portable en GitHub Actions (`windows-latest`),
  igual que ya hace este repo con `pdf2excel`.
- **Cifras de referencia públicas** (proyecto abierto UFFS, Rust, MPL-2.0):
  25,9 M de registros en 7 discos indexados en frío en 68,5 s; consultas
  dirigidas en 0–3 ms del lado del motor; recorrido en memoria a
  167 M registros/s. Son números del propio proyecto, no verificados por
  terceros, pero dan una meta realista.

---

## 1. Qué hace UltraSearch (funciones a igualar)

Información del fabricante y de fichas de terceros (el sitio de JAM no fue
accesible desde este entorno; ver *Fuentes*). Versión gratuita 4.9.x
(actualizada 2026-03).

| Función | Free | Pro | Notas |
|---|:-:|:-:|---|
| Búsqueda por nombre leyendo la MFT, **sin índice** | ✔ | ✔ | Solo NTFS. Relee la MFT en cada búsqueda. |
| Comodines, expresiones regulares, `AND`/`OR` | parcial | ✔ | |
| Búsqueda en **contenido** (texto, Word, PDF…) | ✔ | ✔ | La tabla de ediciones la lista en ambas. |
| Búsqueda dentro de **ZIP** | | ✔ | |
| Filtros (fecha, tamaño, tipo) y exclusiones | básico | avanzado | |
| Columnas de metadatos | 20 | >250 | |
| Exportar resultados (TXT, CSV, Excel, HTML, PDF) | | ✔ | Según fichas de terceros. |
| Unidades de red, USB/SD, Windows Server, SharePoint | | ✔ | |
| Operaciones en lote (mover, archivar, borrar, renombrar) | | ✔ | |
| Duplicados, búsquedas guardadas/presets | | ✔ | Fuente secundaria. |
| Menú contextual del Explorador, vista previa | ✔ | ✔ | Fuente secundaria. |
| Ejecución sin admin (`/NOADMIN`) | ✔ | ✔ | Sin MFT → mucho más lento. Por defecto "Iniciar siempre como administrador". |

**Diferencia clave con Everything:** UltraSearch no mantiene un índice; Everything
construye una base en memoria desde la MFT y la mantiene al día con el USN
Journal (y la guarda en disco al salir). La propuesta de este documento es el
**híbrido**: índice en memoria construido desde la MFT + USN Journal +
caché en disco (consultas en ms, arranque en segundos), y búsqueda de contenido
bajo demanda como UltraSearch.

---

## 2. Fundamentos de NTFS que hay que dominar

### 2.1 La MFT

- Cada archivo/carpeta = un **registro FILE** (normalmente 1024 bytes; puede ser
  4096 en discos 4Kn — leer `BytesPerFileRecordSegment`).
- Número de registro = **FRS** (File Record Segment). Una *file reference* de 64
  bits = 48 bits de FRS + 16 bits de número de secuencia.
- Registros especiales: 0 `$MFT`, 5 = carpeta raíz, etc. (0–15 reservados).
- La propia MFT es un archivo y **puede estar fragmentada**: hay que leer el
  registro 0 y su lista de *data runs* para saber dónde están todos los trozos
  (o pedir `FSCTL_GET_RETRIEVAL_POINTERS`).
- `$MFT::$BITMAP` indica qué registros están en uso → **saltarse los libres
  ahorra entre 40 % y 80 % de la lectura** (cifra de UFFS).

`FSCTL_GET_NTFS_VOLUME_DATA` devuelve lo necesario para empezar:
`BytesPerSector`, `BytesPerCluster`, `BytesPerFileRecordSegment`, `MftStartLcn`,
`MftValidDataLength`.

### 2.2 Estructura de un registro FILE

```
Cabecera: firma "FILE", UsaOffset/UsaCount (fixups), Flags (0x01 en uso,
          0x02 directorio), FirstAttributeOffset, BaseFileRecord
Atributos (hasta el marcador 0xFFFFFFFF):
  0x10 $STANDARD_INFORMATION  fechas (creación, modificación, acceso), atributos
  0x20 $ATTRIBUTE_LIST        el archivo tiene atributos en registros de extensión
  0x30 $FILE_NAME             FRS del padre + nombre UTF-16 + espacio de nombres
  0x80 $DATA                  contenido; residente (archivos pequeños) o data runs
  0x90/0xA0/0xB0              índices de directorio
  0xC0 $REPARSE_POINT         symlinks, junctions, OneDrive
```

Trampas obligatorias:

1. **Fixups (Update Sequence Array):** los 2 últimos bytes de cada sector de
   512 B del registro están sustituidos; hay que restaurarlos desde la USA
   antes de parsear. Si no coinciden, el registro está corrupto.
2. **Espacios de nombres de `$FILE_NAME`:** 0 POSIX, 1 Win32, 2 DOS (8.3),
   3 Win32+DOS. Ignorar los que son solo DOS (2) o saldrán duplicados `PROGRA~1`.
3. **Hard links:** un archivo puede tener varios `$FILE_NAME` con padres
   distintos → varias rutas para el mismo FRS.
4. **Registros de extensión:** archivos con muchos atributos los reparten en
   otros registros (`BaseFileRecord != 0`); hay que fusionarlos con el base.
5. **El tamaño fiable está en `$DATA`**, no en `$FILE_NAME` (ese suele estar
   desactualizado). Residente → longitud del valor; no residente → `DataSize`.
6. **Flujos alternos (ADS):** `$DATA` con nombre; contarlos aparte.
7. **Registros huérfanos** (padre inexistente o reutilizado: comparar número de
   secuencia) → colgarlos de una carpeta virtual.
8. **Ruta completa:** se reconstruye subiendo por los FRS padre hasta el 5
   (raíz); cachear rutas de carpetas, no de archivos.

---

## 3. Tres maneras de enumerar archivos (y cuándo usar cada una)

| Método | Velocidad | Admin | Sistemas de archivos | Qué obtienes |
|---|---|:-:|---|---|
| **A. Leer la MFT en crudo** (`ReadFile` sobre `\\.\C:`) | Máxima | Sí | NTFS | Todo: nombre, padre, tamaños, 4 fechas, atributos, ADS, hard links, ubicación física |
| **B. `FSCTL_ENUM_USN_DATA`** | Muy alta | Sí | NTFS; ReFS con `MFT_ENUM_DATA_V1` (IDs de 128 bits, `USN_RECORD_V3`) | Nombre, padre, atributos. **No trae tamaño ni fechas** reales: obtenerlos archivo por archivo destruye la ventaja |
| **C. Recorrer carpetas** | Media/baja | No | Todos (FAT32, exFAT, red, ReFS) | Lo que da el directorio |

Recomendación: **A** como camino principal en NTFS; **B** para ReFS; **C** como
respaldo (sin admin, FAT/exFAT, unidades de red).

Cómo hacer C lo más rápido posible:

- `FindFirstFileExW(..., FindExInfoBasic, ..., FIND_FIRST_EX_LARGE_FETCH)`
  (salta el nombre 8.3 y pide lotes grandes) o
  `GetFileInformationByHandleEx(FileIdExtdDirectoryInfo)` con buffers de 64 KB+.
- **Paralelismo por directorio** con una cola de trabajo compartida (como
  ripgrep/`ignore`). En **unidades de red** el cuello es la latencia, así que
  32–64 consultas de directorio en vuelo multiplican la velocidad.
- Cuidar bucles por *junctions*/symlinks (no seguir reparse points por defecto)
  y rutas > 260 caracteres (prefijo `\\?\`).

Esquema de lectura cruda (C/Win32, ilustrativo):

```c
HANDLE vol = CreateFileW(L"\\\\.\\C:", GENERIC_READ,
                         FILE_SHARE_READ | FILE_SHARE_WRITE, NULL,
                         OPEN_EXISTING, FILE_FLAG_OVERLAPPED | FILE_FLAG_NO_BUFFERING, NULL);
NTFS_VOLUME_DATA_BUFFER nv;
DeviceIoControl(vol, FSCTL_GET_NTFS_VOLUME_DATA, NULL, 0, &nv, sizeof nv, &ret, &ov);
// 1) leer registro 0 en MftStartLcn * BytesPerCluster -> data runs de $MFT y $BITMAP
// 2) planificar trozos (1-4 MB) saltando zonas libres segun $BITMAP
// 3) lanzar N lecturas superpuestas (IOCP); al completarse cada una:
//    aplicar fixups, parsear registros en un hilo del pool, volver a lanzar lectura
```

---

## 4. Mantener el índice al día: USN Journal

- `FSCTL_QUERY_USN_JOURNAL` → `UsnJournalID`, `NextUsn`, `FirstUsn`.
- `FSCTL_READ_USN_JOURNAL` (en un hilo, bloqueante con `BytesToWaitFor`) →
  cada cambio trae FRS, FRS del padre, nombre y motivo (`USN_REASON_FILE_CREATE`,
  `FILE_DELETE`, `RENAME_NEW_NAME`, `DATA_EXTEND`…). Para tamaño/fechas
  actualizados, releer ese registro de la MFT o pedir info del archivo.
- Guardar en la caché `UsnJournalID` + último `Usn` procesado. Al arrancar:
  si el ID cambió o el USN guardado ya no está (el diario dio la vuelta) →
  **reconstrucción completa**; si no → aplicar solo lo pendiente (arranque en
  segundos, como el "WARM CACHE" de UFFS).
- Si un volumen no tiene diario, crearlo (`FSCTL_CREATE_USN_JOURNAL`) modifica
  el sistema: hacerlo solo con permiso explícito del usuario.
- Para volúmenes sin USN (FAT, red): `ReadDirectoryChangesW`, sabiendo que puede
  perder eventos si el buffer se desborda → rescaneo periódico.

---

## 5. Permisos y modelo de procesos

| Opción | Experiencia | Complejidad | Riesgo |
|---|---|---|---|
| Manifiesto `requireAdministrator` (como UltraSearch) | UAC en cada inicio | Baja | Bajo |
| **Servicio** + IPC por *named pipe* (como Everything/UFFS) | Sin UAC después de instalar | Media | El servicio corre como SYSTEM: validar todo lo que llega por el pipe y no exponer nombres de archivo de un usuario a otro |
| Sin admin (recorrer carpetas) | Sin UAC, lento | Baja | — |

**Aviso de seguridad:** un servicio que entrega la lista de *todos* los archivos
del disco a cualquier proceso local es una fuga de información en equipos
compartidos. Restringir el pipe con una ACL al usuario interactivo/administradores
o filtrar resultados por permisos. Nunca aceptar rutas o comandos arbitrarios
por el pipe.

Plan sugerido: v1 con `requireAdministrator` (simple, igual que UltraSearch);
servicio opcional más adelante.

---

## 6. Exprimir el hardware

### 6.1 Disco (normalmente el cuello de botella)

- **Detectar el tipo de disco** por volumen:
  `IOCTL_STORAGE_QUERY_PROPERTY` con `StorageDeviceSeekPenaltyProperty`
  (`IncursSeekPenalty` → HDD) y `StorageAdapterProperty`/`BusType == NVMe`.
  `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` dice en qué **disco físico** vive cada
  volumen: dos particiones del mismo HDD **no** deben leerse a la vez.
- **E/S asíncrona con IOCP** y ventana deslizante de lecturas en vuelo
  (`FILE_FLAG_OVERLAPPED`, buffers alineados a sector con
  `FILE_FLAG_NO_BUFFERING`). Ajuste publicado por UFFS:

  | Disco | Lecturas simultáneas | Tamaño de trozo | Parseo paralelo |
  |---|---|---|---|
  | NVMe | 32 | 4 MB | Sí, todos los núcleos |
  | SSD SATA | 8 | 2 MB | Opcional |
  | HDD | 2–6 (según fragmentación) | 1 MB | No hace falta |

- **Todos los discos físicos en paralelo**, cada uno con su propia cola.
- **IoRing** (Windows 11, build 22000+; equivalente a io_uring): menos coste por
  operación para muchas lecturas pequeñas (búsqueda de contenido en miles de
  archivos chicos). Usarlo si `QueryIoRingCapabilities` lo soporta; IOCP como
  respaldo. Microsoft está rehaciendo su pila NVMe sobre IoRing, con mejoras
  grandes en lectura aleatoria.
- **HDD + búsqueda en contenido:** ordenar los archivos candidatos por su
  **primer clúster físico** (ya viene en los data runs de la MFT) convierte
  lecturas aleatorias en casi secuenciales. Es la mayor ganancia posible en
  discos mecánicos.

### 6.2 CPU

- Pool de hilos = núcleos lógicos, con robo de trabajo (`rayon`).
- **Parseo de la MFT en paralelo**: cada buffer leído se reparte por bloques de
  registros entre núcleos, escribiendo directo a la estructura final
  (cero copias intermedias, cero asignaciones por registro).
- **Búsqueda por nombre con SIMD:** guardar todos los nombres **ya pasados a
  minúsculas/mayúsculas** en un único buffer contiguo; buscar la subcadena con
  `memchr::memmem` (AVX2/SSE2) sobre el buffer entero, partido en tantos trozos
  como núcleos; mapear cada coincidencia a su registro con búsqueda binaria
  sobre el arreglo de offsets. Esto recorre GB/s.
- Aceleradores: índice por **extensión** (`*.pdf` sin escanear nada) y
  opcionalmente **trigramas** para subcadenas cortas.
- **Orden precalculado:** mantener permutaciones ya ordenadas por nombre, ruta,
  tamaño y fecha (calculadas en segundo plano con `par_sort_unstable`); filtrar
  conserva el orden → mostrar resultados ordenados sin ordenar en cada tecla.
- Búsqueda "mientras escribes": cancelar la consulta anterior con un contador
  de generación atómico y *debounce* de ~30–50 ms.

### 6.3 Memoria

- Estructura de arreglos compactos (índices `u32`, no punteros), nombres en
  UTF-8 o UTF-16 en un único buffer, asignador `mimalloc`.
- Presupuesto: un índice "magro" ocupa ~60–100 B por archivo + nombre
  (2 M archivos ≈ 150–250 MB). UFFS, con campos forenses, usa ~224 B/registro.
- Caché en disco serializada (lectura de un bloque → arranque en segundos).

### 6.4 Prioridad y convivencia

- Modo **Turbo** (`HIGH_PRIORITY_CLASS`, colas profundas) y modo **Cortés**
  (`PROCESS_MODE_BACKGROUND_BEGIN`: baja prioridad de CPU, E/S y memoria). El
  usuario elige; "usar todo" puede congelar el equipo si hay otro trabajo.
- El antivirus inspecciona cada archivo que se abre en la búsqueda de contenido
  y puede dominar el tiempo; medirlo y documentarlo, no intentar evadirlo.

### 6.5 ¿GPU?

No compensa. La búsqueda por nombre ya tarda milisegundos en CPU y la de
contenido está limitada por el disco; copiar datos a la GPU por PCIe añade
latencia sin quitar el cuello real. Descartado para v1.

---

## 7. Búsqueda en contenido

Pipeline:

```
índice en RAM ──filtros baratos primero (ruta, extensión, tamaño, fecha)──►
  candidatos ordenados (por clúster físico en HDD)
    ──► hilos lectores (E/S; IOCP/IoRing; mmap para archivos grandes,
        read con buffers de 256 KB–1 MB para chicos)
          ──canal acotado──► hilos buscadores (CPU, todos los núcleos)
                               ──► resultados en streaming a la interfaz
```

Motores de coincidencia (los mismos que hacen rápido a ripgrep):

| Consulta | Motor | Por qué |
|---|---|---|
| Un literal | `memchr::memmem` | SIMD, GB/s |
| Varios literales | `aho-corasick` (Teddy SIMD) | Todos a la vez en una pasada |
| Regex | `regex` (DFA perezoso, tiempo lineal) | Sin "regex catastróficas" |
| Regex con *backreferences*/lookaround | PCRE2 con JIT o `fancy-regex` | Solo cuando se pidan |
| Miles de patrones | Vectorscan (fork portable de Hyperscan) | Escala a muchos patrones |

Detalles que importan:

- **Codificaciones:** detectar BOM; buscar en UTF-8, UTF-16LE/BE (codificar el
  patrón en ambas) y ANSI/Windows-1252 (`encoding_rs`).
- **Binarios:** si aparece un byte NUL en el primer bloque, saltar o tratar
  como binario (opción).
- Prefiltro: si la regex contiene un literal obligatorio, buscar primero ese
  literal con SIMD y correr la regex solo alrededor (técnica de ripgrep).

Formatos con estructura:

| Formato | Cómo extraer texto |
|---|---|
| DOCX/XLSX/PPTX/ODT | Son ZIP con XML: `zip` + `quick-xml` (rápido, sin dependencias) |
| PDF | `pdfium-render` + `pdfium.dll` (mejor calidad) o `lopdf` (Rust puro, más limitado) |
| DOC/XLS antiguos, MSG, otros | **IFilter** de Windows (`LoadIFilter`), si el filtro está instalado |
| ZIP | Recorrer entradas y buscar dentro (función Pro de UltraSearch) |

Los IFilter son servidores COM **en proceso** y algunos (p. ej. filtros PDF)
se cuelgan o fallan: ejecutarlos en un **proceso trabajador aparte** con
tiempo límite, como hace Windows Search. Índice de contenido persistente
(`tantivy`) queda como mejora futura, no para v1.

---

## 8. Interfaz

- **Lista virtual** para millones de filas: Win32 `LVS_OWNERDATA` o tabla
  virtual de `egui_extras::TableBuilder`; solo se pintan las filas visibles.
- Resultados en streaming mientras se busca; contador y barra de progreso.
- Iconos: cachear por extensión con `SHGetFileInfo(SHGFI_USEFILEATTRIBUTES)` y
  pedirlos solo para filas visibles.
- Menú contextual real del Explorador (`SHCreateDefaultContextMenu`), arrastrar
  y soltar, abrir carpeta contenedora, vista previa (`IPreviewHandler`).
- Exportar CSV/HTML/Excel (`rust_xlsxwriter` o XLSX escrito a mano, como en la
  skill de apps Windows).
- Sintaxis de consulta: comodines, `ext:pdf`, `size:>10MB`, `dm:2026`,
  `AND/OR/NOT`, `regex:`, `content:"texto"`.

---

## 9. Arquitectura propuesta

```
┌────────────────────────── motor (proceso elevado o servicio) ─────────────────────┐
│ Por volumen:  lector MFT (IOCP) ─► parseo paralelo ─► índice compacto en RAM       │
│               vigilante USN ─────────────────────────► aplica cambios              │
│               caché en disco (ID diario + último USN)                              │
│ Consultas:    parser de sintaxis ─► filtros + SIMD/índices ─► permutación ordenada │
│ Contenido:    planificador por disco físico ─► lectores ─► buscadores (rayon)      │
└───────────────▲───────────────────────────────────────────────────────────────────┘
                │ IPC (named pipe con ACL) o llamada directa si es un solo proceso
     ┌──────────┴─────────┐      ┌──────────┐
     │ GUI (egui o Win32) │      │   CLI    │
     └────────────────────┘      └──────────┘
```

Módulos con una responsabilidad cada uno (probables *crates* del workspace):
`ntfs-mft` (lectura y parseo), `usn` (cambios), `index` (estructura y consultas),
`content` (pipeline de contenido), `query` (sintaxis), `app` (GUI), `cli`.

---

## 10. Lenguaje y herramientas

| | **Rust (recomendado)** | C++ Win32 | C# .NET 8+ |
|---|---|---|---|
| Rendimiento | Máximo | Máximo | Muy bueno (Span, SIMD), con GC |
| Seguridad al parsear estructuras de disco posiblemente corruptas | Alta | Baja (desbordamientos) | Alta |
| Búsqueda de contenido "de fábrica" | Crates de ripgrep | Hacerlo a mano / PCRE2 / Vectorscan | `Regex` (NonBacktracking) |
| Interfaz | egui / Slint / Win32 vía `windows` | Win32/WTL | WinForms/WPF (lo más fácil) |
| Ejecutable portable | Sí, un `.exe` | Sí, el más pequeño | Single-file autocontenido (grande); NativeAOT sin soporte oficial para WinForms/WPF |
| Referencia abierta existente | UFFS (Rust, MPL-2.0) | SwiftSearch | — |

Por qué Rust: mismas cotas de velocidad que C++, seguridad de memoria al
interpretar bytes crudos del disco, y los mejores motores de búsqueda de texto
ya hechos. Compilación: GitHub Actions `windows-latest` (el repo ya tiene ese
patrón en `.github/workflows/build-windows.yml`); en este entorno Linux hay Rust
1.97 y acceso al registro de crates para pruebas y lógica multiplataforma.

Crates candidatas (versiones verificadas en index.crates.io el 2026-10-22):

| Crate | Versión | Uso |
|---|---|---|
| `windows` | 0.62.2 | API Win32 (CreateFile, DeviceIoControl, IOCP, IoRing, Shell) |
| `rayon` | 1.12.0 | Paralelismo con robo de trabajo |
| `crossbeam-channel` | 0.5.17 | Canales acotados lector→buscador |
| `memchr` | 2.8.3 | Búsqueda SIMD de bytes/subcadenas |
| `aho-corasick` | 1.1.5 | Multipatrón (Teddy) |
| `regex` | 1.13.1 | Regex en tiempo lineal |
| `grep-searcher` / `grep-regex` | 0.1.17 / 0.1.14 | Núcleo de búsqueda de ripgrep (líneas, binarios, codificaciones) |
| `ignore` | 0.4.33 | Recorrido paralelo de carpetas (respaldo sin MFT) |
| `memmap2` | 0.9.12 | Mapear archivos grandes |
| `encoding_rs` | 0.8.42 | UTF-16 / Windows-1252 |
| `mimalloc` | 0.1.52 | Asignador rápido |
| `ntfs` / `mft` / `ntfs-reader` | 0.4.0 / 0.7.0 / 0.6.0 | Parsers NTFS de referencia (revisar licencia y rendimiento; probablemente conviene un parser propio, mínimo y paralelo) |
| `usn-journal-rs` | 0.5.0 | Referencia para USN |
| `eframe` / `egui_extras` | 0.37.0 | Interfaz rápida con tabla virtual |
| `zip` / `quick-xml` | 9.0.5 / 0.42.0 | Texto de DOCX/XLSX/PPTX |
| `pdfium-render` / `lopdf` | 0.9.4 / 0.46.0 | Texto de PDF |
| `calamine` | 0.37.0 | Hojas de cálculo |
| `vectorscan-rs` | 0.0.6 | Miles de patrones (inmaduro; opcional) |
| `tantivy` | 0.27.0 | Índice de contenido (futuro) |

---

## 11. Proyectos de referencia y licencias

| Proyecto | Lenguaje | Licencia | Qué aprender | ¿Se puede copiar código? |
|---|---|---|---|---|
| [UFFS](https://github.com/skyllc-ai/UltraFastFileSearch) | Rust | MPL-2.0 | Lectura MFT con IOCP, salto por bitmap, ajuste por tipo de disco, índice compacto, daemon + broker sin UAC | Sí, respetando MPL (los archivos modificados siguen siendo MPL) |
| [Ultra-Fast-File-Search (C++)](https://github.com/githubrobbi/Ultra-Fast-File-Search) | C++ | CC BY-NC 2.0 | Versión antigua derivada de SwiftSearch | **No** para uso comercial; solo estudiar ideas |
| SwiftSearch | C++ | Abierto (revisar) | Lectura directa de la MFT sin índice (el modelo de UltraSearch) | Revisar licencia |
| [ripgrep](https://github.com/BurntSushi/ripgrep) | Rust | MIT/Unlicense | Búsqueda de contenido, recorrido paralelo | Sí |
| Everything (voidtools) | C | Cerrado | Comportamiento y UX de referencia | No |

No reutilizar nombre, logo ni interfaz de UltraSearch/JAM Software; construir
un producto propio.

---

## 12. Riesgos y casos difíciles

- UAC/administrador (sección 5).
- ReFS no tiene MFT → `FSCTL_ENUM_USN_DATA` con V1 o recorrido de carpetas.
- BitLocker: transparente si el volumen está desbloqueado (se lee a través del
  volumen montado).
- Discos dinámicos, Storage Spaces, RAID: el volumen no es un disco físico;
  `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` lo aclara.
- MFT gigante (decenas de millones de registros): presupuesto de memoria y
  caché por niveles (UFFS probó 100 M de registros).
- Diario USN que da la vuelta o se borra → reconstrucción completa automática.
- Unidades de red: sin MFT, latencia alta → recorrido muy paralelo y caché.
- Comparación de nombres sin distinguir mayúsculas: NTFS usa su tabla `$UpCase`;
  usar `CompareStringOrdinal(..., TRUE)` o la tabla `$UpCase` del volumen.
- Archivos OneDrive "solo en la nube": leer su contenido provoca descarga;
  no abrirlos en búsqueda de contenido salvo que el usuario lo pida
  (`FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`).

---

## 13. Objetivos de rendimiento (para medir, no para prometer)

| Medida | Objetivo inicial |
|---|---|
| Índice en frío, NVMe, ~2 M archivos | < 5 s |
| Arranque con caché | < 2 s |
| Consulta por nombre (motor) | < 10 ms |
| Búsqueda de contenido en NVMe | ≥ 70 % del ancho de banda secuencial del disco |
| Memoria | ≤ 120 B por archivo |

Medir siempre en frío (vaciar la *standby list*, p. ej. con RAMMap) y en
caliente, varias rondas, reportando la mediana.

---

## 14. Plan por fases

1. **Spike (desechable):** leer la MFT de `C:` con IOCP, contar registros en uso
   y medir tiempo y MB/s. Valida la parte más difícil y da números reales del
   equipo del usuario.
2. **MVP CLI:** índice en memoria + búsqueda por nombre (comodines, ext, tamaño,
   fecha) + exportar CSV.
3. **GUI:** lista virtual, búsqueda mientras escribes, menú contextual.
4. **Tiempo real:** USN Journal + caché en disco.
5. **Contenido:** texto plano/UTF-16 con motores de ripgrep; luego DOCX/XLSX/PDF.
6. **Extras:** ZIP, duplicados, operaciones en lote, servicio sin UAC, unidades
   de red.

---

## 15. Decisiones pendientes del usuario

1. Uso principal: ¿solo nombres en discos locales, también contenido, o
   entorno de empresa (red, servidores, varios usuarios)?
2. ¿Aceptable pedir UAC al abrir el programa, o se necesita servicio sin UAC?
3. ¿Interfaz gráfica, línea de comandos o ambas?
4. ¿Windows 10 y 11, o solo 11 (permite IoRing sin respaldo)?
5. ¿Uso personal o se distribuirá/venderá? (afecta licencias de referencia).

---

## Fuentes

- JAM Software — [UltraSearch](https://www.jam-software.com/ultrasearch),
  [comparativa de ediciones](https://www.jam-software.com/ultrasearch/editions.shtml),
  [características (manual)](https://manuals.jam-software.com/ultrasearch/EN/features.html),
  [ficha UltraSearch Professional (PDF)](https://www.jam-software.com/datasheets/UltraSearchProfessional_Product-Data-Sheet_EN.pdf),
  [base de conocimiento](https://knowledgebase.jam-software.com/ultrasearch),
  [cambios](https://jam-software.com/ultrasearch/changes.shtml),
  [pestaña Opciones](https://manuals.jam-software.com/ultrasearch/EN/ribbonoptions.html)
- Fichas de terceros: [comss.ru](https://www.comss.ru/page.php?id=2867),
  [FileHorse](https://www.filehorse.com/download-ultrasearch/),
  [AlternativeTo](https://alternativeto.net/software/ultrasearch/about/),
  [FC Portables](https://www.fcportables.com/ultrasearch-portable/),
  [UpdateStar](https://ultrasearch.updatestar.com/),
  [Softpedia](https://www.softpedia.com/get/File-managers/JAM-Software-UltraSearch.shtml),
  [foro portablefreeware (`/NOADMIN`)](https://portablefreeware.com/forums/viewtopic.php?p=102250)
- voidtools (Everything): [¿por qué es tan rápido?](https://mail.voidtools.com/forum/viewtopic.php?p=33240),
  [cómo no pierde cambios](https://voidtools.com/forum/viewtopic.php?p=486)
- Microsoft Win32 (repositorio [MicrosoftDocs/sdk-api](https://github.com/MicrosoftDocs/sdk-api)):
  `FSCTL_ENUM_USN_DATA`, `FSCTL_READ_USN_JOURNAL`, `FSCTL_GET_NTFS_VOLUME_DATA`,
  `FSCTL_GET_RETRIEVAL_POINTERS`, `IOCTL_STORAGE_QUERY_PROPERTY`,
  `DEVICE_SEEK_PENALTY_DESCRIPTOR`, `MFT_ENUM_DATA_V1`, `USN_RECORD_V2`,
  `CreateIoRing`, `BuildIoRingReadFile`, `FindFirstFileExW`;
  [IFilter](https://msdn2.microsoft.com/en-us/library/ms691105(VS.85).aspx),
  [filtros en Windows Search](https://msdn.microsoft.com/en-us/windows/desktop/dd940428)
- UFFS: [repositorio](https://github.com/skyllc-ai/UltraFastFileSearch) (README,
  `docs/architecture/engine/01-overview.md`, `crates/uffs-mft/README.md`),
  [benchmark contra Everything](https://uffs.io/blog/benchmarking-against-everything/),
  [versión C++](https://github.com/githubrobbi/Ultra-Fast-File-Search/blob/main/README.md)
- SwiftSearch: [gHacks](https://www.ghacks.net/?p=94004),
  [OSDN](https://ko.osdn.net/projects/sfnet_swiftsearch);
  [NTFS-Search](https://www.sourceforge.net/p/ntfs-search)
- ripgrep: [por qué es rápido](https://www.youngju.dev/blog/2026-06-17-why-ripgrep-is-so-fast.en),
  [benchmarks](https://aicodereview.cc/blog/ripgrep-vs-grep-performance/),
  [comentario del autor](https://nyc1.lr.ggtyler.dev/r/linux/comments/118ok87/why_gnu_grep_is_fast/j9lrbaj/?context=3)
- Pila NVMe sobre IoRing: [igor'sLAB](https://www.igorslab.de/en/microsoft-fundamentally-redesigns-nvme-drivers-for-windows-initial-tests-show-significant-performance-shifts/),
  [Tom's Hardware](https://tomshardware.com/pc-components/ssds/new-windows-native-nvme-driver-benchmarks-reveal-transformative-performance-gains-up-to-64-89-percent-lightning-fast-random-reads-and-breakthrough-cpu-efficiency)
- IFilter en la práctica: [CodeProject](https://www.codeproject.com/Articles/31944/WebControls/),
  [Foxit PDF IFilter](https://www.sharepointeurope.com/media/265501/whitepaper_foxitpdfifilter.pdf),
  [PDFlib TET IFilter (retirado)](https://www.componentsource.com/zh-hant/product/pdflib-tetifilter/about)
