# Lince — buscador de archivos instantáneo para Windows

Busca archivos por nombre en todo el disco en milisegundos. Igual que
UltraSearch o Everything, no recorre carpetas: **lee directamente la MFT de
NTFS** (la tabla con un registro por cada archivo del volumen), con lecturas
en paralelo ajustadas al tipo de disco y búsqueda paralela con SIMD.

> Estado: **Fase 1** (núcleo, ventana y línea de comandos). Próximas fases:
> búsqueda en contenido, actualización en tiempo real (USN Journal) y servicio
> para empresa sin UAC. Diseño: `docs/superpowers/specs/2026-10-22-lince-design.md`.

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
