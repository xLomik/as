# Lince — buscador de archivos instantáneo para Windows

Busca archivos por nombre en todo el disco en milisegundos. Igual que
UltraSearch o Everything, no recorre carpetas: **lee directamente la MFT de
NTFS** (la tabla con un registro por cada archivo del volumen), con lecturas
en paralelo ajustadas al tipo de disco y búsqueda paralela con SIMD. También
busca **dentro** de Word, Excel, PDF y archivos de texto.

> Estado: núcleo, ventana, línea de comandos, unidades de red, clic derecho
> del Explorador y búsqueda en contenido (primera entrega). Próximas fases:
> más formatos (filtros de Windows: .ppt, .rtf, .msg...), actualización en
> tiempo real (USN Journal) y servicio para empresa sin UAC. Diseño:
> `docs/superpowers/specs/2026-10-22-lince-design.md`.

## Programa con ventana: `Lince.exe`

Doble clic en **`Lince.exe`**. Windows pedirá permiso de administrador (UAC):
acéptelo para leer la MFT y tener resultados al instante. Si lo rechaza, Lince
funciona igual pero recorre las carpetas (más lento).

- Escriba en la caja de arriba: los resultados aparecen mientras escribe.
- **Doble clic** o **Enter** abre el archivo. **Clic derecho**: abrir, abrir la
  carpeta que lo contiene, copiar la ruta o el nombre, exportar.
- **Clic en una columna** (Nombre, Carpeta, Tamaño, Modificado) ordena; otro
  clic invierte el orden.
- La lista de la derecha filtra por unidad (C:, D:...).
- **Exportar a Excel…** (o Ctrl+E) guarda los resultados en un CSV que Excel
  abre directamente.
- **F5** vuelve a leer los discos (para ver archivos nuevos). **Ctrl+F**
  vuelve a la caja de búsqueda; **flecha abajo** pasa a la lista; **Esc**
  borra la búsqueda; **Ctrl+C** copia las rutas seleccionadas.

La caja de búsqueda acepta la misma sintaxis que la versión de consola (tabla
de abajo): `*.pdf`, `size:>10mb`, `dm:2026-10`, `ext:dwg;dxf`...

### Buscar dentro de los archivos

La **segunda caja** («Texto dentro del archivo») busca dentro de los
documentos que cumplen la primera. Lee:

| Tipo | Archivos |
|---|---|
| Word | `.docx`, `.docm`, `.dotx` y los viejos `.doc`, `.dot` (97–2003) |
| Excel | `.xlsx`, `.xlsm`, `.xlsb` y los viejos `.xls`; los números se encuentran con coma o punto (`4512,75` o `4512.75`) |
| PDF | `.pdf` con texto (no los escaneados como imagen) |
| Texto | `.txt`, `.csv`, `.log`, `.xml`, `.html`, `.json`, `.ini`, código, programas CNC (`.nc`, `.tap`, `.gcode`...) y `.dxf`; en UTF-8, UTF-16 o ANSI |
| También | PowerPoint `.pptx` y OpenDocument (`.odt`, `.ods`, `.odp`) |

- No distingue mayúsculas y una frase se encuentra aunque el documento la
  parta en dos líneas.
- Los resultados van apareciendo mientras lee; la barra de abajo dice
  cuántos archivos lleva. Escribir otra cosa (o **Esc** en la caja) la
  detiene.
- Leer el contenido es mucho más lento que buscar por nombre: acote con la
  primera caja (`ext:pdf`, `dm:2026`, `path:Proyectos`) o abra Lince en la
  carpeta con el **clic derecho** del Explorador. Los archivos se leen del
  más pequeño al más grande, para que los primeros resultados salgan antes.
- En la primera caja también se puede escribir `contenido:"orden de compra"`
  (o `content:`), igual que en la consola.
- Archivos protegidos con contraseña o dañados se saltan (se cuentan en la
  barra como «no se pudieron leer»).

### Buscar solo en una carpeta (clic derecho del Explorador)

1. Abra Lince y active **Buscar en → «Buscar aquí con Lince» en el clic
   derecho del Explorador** (queda con ✓; otro clic lo quita). Hágalo con
   Lince.exe ya en su carpeta definitiva, no abierto dentro del .zip.
2. En el Explorador de archivos, clic derecho **dentro** de una carpeta
   («Buscar aquí con Lince»), **sobre** una carpeta («Buscar en esta carpeta
   con Lince») o sobre una unidad («Buscar en esta unidad con Lince»).
   En Windows 11 está en **Mostrar más opciones**.
3. Se abre una ventana de Lince que busca **solo en esa carpeta** y sus
   subcarpetas; el título muestra cuál. No pide permisos de administrador
   (recorre la carpeta); en la raíz de un disco sí los pide, para leer la MFT.

Se guarda solo para su usuario de Windows (no hace falta ser
administrador). Si mueve Lince.exe a otra carpeta, la entrada se corrige
sola la próxima vez que lo abra. Para instalarlo o quitarlo desde un script:
`Lince.exe --instalar-menu` y `Lince.exe --quitar-menu`.

### Unidades y carpetas de red

- Las **unidades de red conectadas** (Z:, P:...) se incluyen solas, también
  cuando Lince corre como administrador (Windows las oculta a los programas
  elevados; Lince le pasa su ruta `\\servidor\carpeta` a la copia elevada).
- Para una carpeta de red sin letra, o cualquier otra carpeta: menú
  **Buscar en → Agregar carpeta o unidad de red…** y escriba
  `\\servidor\carpeta` (o pulse «Examinar…»). Queda guardada para las
  próximas veces. **Buscar en → Quitar carpetas agregadas…** las borra.
- Los discos locales están listos en segundos; la red se sigue leyendo en
  segundo plano (la barra de abajo dice «indexando N más…») y sus archivos
  van apareciendo en la lista. En red no hay MFT: se recorren las carpetas
  con 32 lecturas a la vez, así que tarda más que un disco local.
- Lince no se entera solo de los cambios en la red: **F5** vuelve a leerla.

## Versión de consola: `lince-cli.exe`

Para scripts y uso avanzado. Abra **PowerShell o CMD como administrador**
(leer la MFT lo exige) y ejecute:

```
lince-cli informe              todo lo que contenga «informe» en todos los discos NTFS
lince-cli "*.pdf size:>10mb"   PDF de más de 10 MB
lince-cli "ext:docx;xlsx" dm:2026-10    Word y Excel modificados en octubre de 2026
lince-cli --volume D: foto     solo en D:
lince-cli --csv resultados.csv ext:dwg  guarda los resultados para Excel
lince-cli --count *            cuántos archivos y carpetas hay
lince-cli --stats notepad.exe  muestra tiempos de lectura y búsqueda
```

En **cmd** y **PowerShell**, `>` y `<` redirigen la salida: ponga entre
comillas los filtros que los lleven, o la consulta entera:
`lince-cli "*.pdf size:>10mb"`. En PowerShell `;` separa comandos: escriba
`"ext:docx;xlsx"`. Para una frase exacta, las comillas deben llegar
a Lince: `lince-cli '"foto vacaciones"'` (PowerShell).

Sin permisos de administrador, o en unidades de red y memorias FAT/exFAT,
Lince recorre las carpetas en paralelo (más lento, pero funciona):

```
lince-cli --path \\servidor\compartido contrato
lince-cli --path E:\ ext:jpg
```

### Sintaxis

| Escriba | Encuentra |
|---|---|
| `texto` | nombres que contienen «texto» (sin distinguir mayúsculas ni acentos en mayúscula) |
| `a b` | contienen `a` **y** `b` |
| `"a b"` | la frase exacta |
| `!texto` | no contienen «texto» |
| `*.pdf`, `f?to*` | comodines sobre el nombre completo |
| `ext:pdf;docx` | por extensión |
| `size:>10mb`, `size:<1kb`, `size:1kb..2mb` | por tamaño (solo archivos) |
| `dm:2026`, `dm:2026-10`, `dm:22/10/2026`, `dm:>=2026-01-01`, `dm:2026-01..2026-03` | por fecha de modificación (hora local); `dc:` por fecha de creación |
| `file:`, `folder:` | solo archivos / solo carpetas |
| `path:proyectos\2026` | la ruta completa contiene ese texto |
| `regex:^IMG_\d+\.jpg$` | expresión regular sobre el nombre |
| `contenido:"orden de compra"` | el archivo contiene el texto (Word, Excel, PDF, texto); también `content:` |

### Opciones

| Opción | Qué hace |
|---|---|
| `-v, --volume X:` | volumen a leer (se puede repetir); por defecto todos los NTFS fijos |
| `-p, --path CARPETA` | recorrer una carpeta en vez de leer la MFT |
| `-n, --limit N` | máximo de resultados |
| `-s, --sort none\|name\|path\|size\|date` y `--desc` | orden (por defecto, ruta) |
| `--csv ARCHIVO` | CSV con BOM y `;` (se abre directo en Excel en español) |
| `--count` | solo el número de resultados |
| `--stats` | tiempos, MB leídos y velocidad, por volumen |

Códigos de salida: `0` hay resultados, `1` ninguno, `2` error.

## Cómo aprovecha el equipo

- Lee la MFT con **32 lecturas simultáneas de 4 MiB en NVMe**, 8 × 2 MiB en
  SSD y 2 × 1 MiB en discos mecánicos (detecta el tipo de disco).
- Salta los registros libres de la MFT usando su `$BITMAP`.
- Todos los volúmenes se leen a la vez y cada lectura se interpreta en el
  mismo hilo que la hizo, así que el parseo usa todos los núcleos.
- Índice compacto (40 bytes por nombre + el nombre); la búsqueda recorre un
  único bloque de memoria con `memchr` (SIMD) repartido entre todos los núcleos.

## Compilar

Requiere Rust estable.

```
cd lince
cargo build --release          # target\release\Lince.exe y lince-cli.exe
cargo test --workspace
```

GitHub Actions (`.github/workflows/lince.yml`) compila y prueba en Linux y
Windows, ejecuta una prueba real contra la MFT del runner y publica
`Lince.exe` y `lince-cli.exe` como artefacto descargable.

Las imágenes NTFS de prueba (`fixtures/`) se regeneran con
`sudo tools/gen-fixtures.sh` (Linux con ntfs-3g).

Windows SmartScreen puede avisar la primera vez porque el ejecutable no está
firmado.
