# Lince — buscador de archivos tipo UltraSearch (diseño)

> Nombre de trabajo: **Lince** (se puede cambiar). Base: `docs/investigacion/buscador-tipo-ultrasearch.md`.

## Alcance pedido

El usuario lo usará para las tres cosas:

- **A.** Buscar por nombre en los discos locales (MFT de NTFS).
- **B.** Buscar dentro del contenido (texto, Office, PDF).
- **C.** Entorno de empresa: unidades de red, servidores, varios usuarios.

## Decisiones (por defecto, corregibles)

| Tema | Decisión |
|---|---|
| Lenguaje | Rust (edición 2021, toolchain estable). Identificadores en inglés, comentarios y textos de usuario en español. |
| SO | Windows 10 1809+ y Windows 11, x64. |
| Permisos v1 | El programa detecta si puede abrir el volumen. Si no (sin admin, FAT, red), recorre carpetas en paralelo y lo avisa. Servicio sin UAC en una fase posterior (C). |
| Índice | En memoria, construido desde la MFT; una entrada por nombre (los hard links aparecen en cada ruta). Nombres en un buffer UTF-8 contiguo + copia en minúsculas separada por `\0` para buscar con SIMD. |
| E/S | Hilos con lecturas posicionales (profundidad de cola = número de hilos), ajustada por tipo de disco: NVMe 32, SSD 8, HDD 2. IOCP/IoRing solo si las mediciones lo justifican. |
| Paralelismo | `rayon` para parsear la MFT y buscar. |
| Consultas v1 | Términos AND, comodines `*` `?`, `ext:`, `size:`, `dm:`, `file:`/`folder:`, `path:` (buscar en ruta completa), `regex:`. |
| Salida v1 | CLI: rutas por línea, `--csv`, `--stats`, `--limit`, `--sort`. |
| Interfaz gráfica | Fase posterior (probable `egui`). |
| Licencia de referencias | No se copia código de UFFS ni de su versión C++; solo ideas. |

## Fases

1. **Núcleo + CLI (A):** parser NTFS puro, lector de MFT, índice, consultas, recorrido de carpetas, CLI, CI en Windows.
2. **Contenido (B):** texto/UTF-16 con motores de ripgrep; luego DOCX/XLSX/PDF.
3. **Tiempo real:** USN Journal + caché en disco.
4. **GUI.**
5. **Empresa (C):** servicio sin UAC con IPC por *named pipe* con ACL, unidades de red, servidores.

## Criterios de aceptación de la Fase 1

- Sobre imágenes NTFS reales (generadas con `mkntfs` + ntfs-3g) el índice
  desde la MFT coincide exactamente (rutas y tamaños) con el listado del
  sistema de archivos montado.
- En el runner Windows de GitHub Actions (administrador), `lince` encuentra
  archivos recién creados en el volumen con su ruta y tamaño exactos.
- Sin permisos o en carpetas no NTFS, `lince --path <carpeta>` funciona con el
  recorrido de carpetas.
- `cargo test`, `cargo clippy -- -D warnings` y `cargo fmt --check` limpios en
  Linux y Windows.

## Resultado de la Fase 1 (medido)

Runner `windows-latest` de GitHub (VM de Azure, 4 vCPU, disco virtual), como
administrador:

| Medida | Valor |
|---|---|
| Registros de la MFT de `C:` | 1 362 328 (1,33 GB) |
| Entradas en el índice | 1 411 362 |
| Índice en frío | 12,6 s (106 MB/s, límite del disco virtual) |
| Índice con la MFT en caché del sistema | 3,8 s |
| Consulta `*` (recorre todo el índice) | 38 ms |
| Consulta dirigida (`cmd.exe`) | 1 ms |
| Prueba de exactitud | 3 archivos nuevos (hard link y Unicode) con ruta y tamaño exactos |

Linux, imagen NTFS de 300 000 archivos (en caché): índice en 0,25 s
(≈1 200 MB/s), 166 MB de memoria máxima, consulta completa en 11 ms.

Pendiente de rendimiento detectado (para la fase de tiempo real):

- En la VM el tipo de disco sale «desconocido» (4 lecturas simultáneas);
  medir si 8–16 rinde más en discos virtuales.
- La lectura en Windows copia cada trozo desde un buffer alineado nuevo:
  reutilizar un buffer alineado por hilo y leer directo en él.
- El parseo crea un `String` y un `Vec` por registro: escribir los nombres
  directamente a un buffer por trozo bajaría tiempo y memoria pico.
