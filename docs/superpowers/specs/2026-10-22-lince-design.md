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
