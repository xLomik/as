# Ingeniería inversa BySoft: DXF / BOX / LOCK — Informe fase 1

Fecha del análisis: 2026-10-01. Muestra: `hcmf75423_r0`.
Script del análisis: `analisis_muestra.py`. No es el conversor.

## 1. Resumen

| Archivo | Estado |
|---|---|
| `.dxf` | Analizado por completo. Es DXF R14 ASCII **exportado por BySoft** (`999 BYSOFT7 PART MODEL`). |
| `.box` | **NO RECIBIDO.** Llegaron dos copias idénticas del DXF (mismo SHA-256). No se puede analizar. |
| `.lock` | Decodificado al 100 % y reconstruido byte a byte (idéntico). **Es un archivo de bloqueo de sesión** (`LockInfo`): usuario, equipo y fecha. **No contiene datos de la pieza.** |

Conclusiones clave:
1. Sin el `.box` no hay forma de juzgar si se puede generar. Esto bloquea las fases 3, 7, 9 y 10 para el BOX.
2. El `.lock` no se deriva del DXF. Probablemente lo crea BySoft cuando una pieza está abierta o en edición. **Recomendación: no generarlo** (ver §22).

## 2. Archivos analizados

| Archivo | Bytes | MD5 | SHA-256 | Tipo |
|---|---|---|---|---|
| hcmf75423_r0.dxf (copia 1) | 3440 | b0d71505d3c9233b12d875db1892d340 | f5fd30ab2660cea48a039f08ac3174d1c2e8260bb3b42a9cf72fafde1d76c4f5 | Texto ASCII, CRLF |
| hcmf75423_r0.dxf (copia 2) | 3440 | idéntico | idéntico | — duplicado |
| hcmf75423_r0.lock | 179 | b95ba7c1ec68c05347d5f97b641c337e | 4942734e817df2f813d1651e0464b1c7fbbbeaa8ea93357bb87a287e7b3706fa | Binario (.NET BinaryWriter) |
| hcmf75423_r0.box | — | — | — | **falta** |

`file(1)` identifica el `.lock` como "Tower/XP rel 3 object", lo cual es falso: su firma coincide por azar.

## 3. Formato real del DXF

- Texto ASCII puro, CRLF, sin bytes >127. Codepage declarada `ANSI_1252`.
- Primera línea: comentario `999` / `BYSOFT7 PART MODEL`. Esto indica que el archivo fue **escrito por BySoft** y no por un CAD externo.
- `$ACADVER = AC1014` (R14). `$MEASUREMENT = 1`, es decir métrico (mm). No hay `$INSUNITS`. `$HANDSEED = 10000`.
- Tablas mínimas con capas fijas `0`, `GEOMETRY`(4), `BENDLINE`(2), `ENGRAVING`(3) y `CONSTRUCTION`(8). Linetypes BYBLOCK, BYLAYER, CONTINUOUS y CENTERX2.
- VPORT `*ACTIVE`: centro (51.025, 17), altura 102.05, aspecto 3.00147 = 102.05/34. Todo se deriva del bbox.
- Sin bloques de usuario. Hay 2 entidades, ambas `LWPOLYLINE` en la capa `GEOMETRY`:

| Entidad | Vértices | Cerrada | Sentido | BBox | Área | Perímetro | Interpretación |
|---|---|---|---|---|---|---|---|
| 1D | 2, bulge = 1 en ambos | sí | antihorario | (9.8, 8.75)–(26.3, 25.25) | 213.82 | 51.84 | Círculo Ø16.5, centro (18.05, 17). Agujero. |
| 1E | 5 | sí | **horario** | (0, 0)–(102.05, 34) | 3469.70 | 272.10 | Contorno exterior 102.05 × 34 |

Observaciones:
- El contorno exterior tiene un vértice colineal en (51.025, 34). Es un punto medio sobrante, probablemente un artefacto de BySoft.
- Sentidos: exterior en sentido horario y agujero en antihorario. Es posible que sea una convención de BySoft. **Hipótesis**: se necesitan más piezas para confirmarlo.
- Los círculos se exportan como LWPOLYLINE con 2 bulges y no como CIRCLE.
- Pieza total: 102.05 × 34 mm, con 1 contorno exterior y 1 agujero. Área neta 3255.88 mm².
- Miniatura recibida (PNG): rectángulo con agujero a la izquierda, un trazo en el círculo y una marca en la esquina inferior izquierda. Puede ser una vista previa de BySoft con un punto de inicio/lead-in y el origen. No se confirma.

**Consecuencia importante:** el DXF de muestra es una **salida** de BySoft, no la entrada original. Los DXF reales del usuario, de AutoCAD, SolidWorks u otro CAD, tendrán otra estructura (CIRCLE, ARC, LINE, otras capas, INSUNITS). El futuro conversor tendrá que normalizarlos.

## 4. Formato real del BOX

**DESCONOCIDO. No se recibió el archivo.** No se afirma nada sobre él.

## 5. Formato real del LOCK — CONFIRMADO

Serialización con `System.IO.BinaryWriter` de .NET: cadenas UTF-8 con prefijo de longitud codificado en 7 bits y enteros little-endian.

| Offset | Long. | Tipo | Valor observado | Significado | Confianza |
|---|---|---|---|---|---|
| 0x00 | 1 | byte | `0x01` | Marcador o versión de formato (¿bool "no nulo"?) | HIPÓTESIS (valor confirmado, significado no) |
| 0x01 | 2 | 7-bit varint | `95 01` = 149 | Longitud de la cadena | CONFIRMADO |
| 0x03 | 149 | string UTF-8 | `Bystronic.BySoft.Common.Persistence.LockInfo, Bystronic.BySoft.Common.Persistence, Version=8.2.22.0, Culture=neutral, PublicKeyToken=3b9ed0ffebbd7787` | Nombre de tipo .NET del objeto serializado y versión del ensamblado BySoft | CONFIRMADO |
| 0x98 | 8 | Int64 LE | `0x88DF1FF9EF28EF6F` | `DateTime.ToBinary()`: bits 63-62 = 2 (Kind=Local), ticks UTC → **2026-10-01 20:24:02.965 UTC** | MUY PROBABLE (la decodificación da una fecha coherente: 11 min antes de la subida de los archivos) |
| 0xA0 | 1 | varint | 7 | Longitud | CONFIRMADO |
| 0xA1 | 7 | string | `calvear` | Usuario de Windows | MUY PROBABLE |
| 0xA8 | 1 | varint | 10 | Longitud | CONFIRMADO |
| 0xA9 | 10 | string | `FA-PDYD023` | Nombre del equipo | MUY PROBABLE |
| 0xB3 | — | EOF | total 179 bytes | No hay checksum ni relleno | CONFIRMADO |

Prueba: el script vuelve a serializar los 5 campos y obtiene un archivo **idéntico byte a byte** al original.

## 6. Relación DXF ↔ BOX

No determinable (no hay BOX).

## 7. Relación DXF ↔ LOCK

**Ninguna.** El `.lock` no contiene geometría, nombre de pieza, hash del DXF ni dimensiones. Sus únicas entradas son la versión de BySoft, la hora, el usuario y el equipo. El vínculo con la pieza es **solo el nombre del archivo** (mismo nombre base).

## 8. Campos del BOX

Ninguno identificado.

## 9. Campos del LOCK

Ver §5.

## 10. Derivable del DXF (A)

| Dato | Confianza |
|---|---|
| Unidades (mm, `$MEASUREMENT=1`) | CONFIRMADO |
| BBox 0,0 – 102.05,34; ancho 102.05; alto 34 | CONFIRMADO |
| N.º de contornos (2), exteriores (1), agujeros (1), Ø agujero 16.5 | CONFIRMADO |
| Área (3469.70 bruta, 3255.88 neta) y perímetros | CONFIRMADO (calculado) |
| Capas, entidades, sentido de los contornos | CONFIRMADO |
| Si alguno de estos datos aparece en el BOX | DESCONOCIDO |

## 11. Información externa (C)

| Dato | Origen | Confianza |
|---|---|---|
| Versión de ensamblado 8.2.22.0 y PublicKeyToken | Instalación de BySoft | CONFIRMADO |
| Fecha y hora del bloqueo | Reloj del sistema en el momento del bloqueo | MUY PROBABLE |
| Usuario `calvear` | Sesión de Windows | MUY PROBABLE |
| Equipo `FA-PDYD023` | Nombre del PC | MUY PROBABLE |
| Material, espesor y tecnología (probables en el BOX) | Base de datos de BySoft | HIPÓTESIS, sin evidencia |

Derivable del nombre o ruta (B): `hcmf75423` es el nombre de la pieza y `_r0` es probablemente la revisión 0 (HIPÓTESIS). No se encontró ninguno de los dos dentro del `.lock`.

## 12. Confirmados

Estructura completa del `.lock`. Formato del DXF y toda su geometría. Las dos copias del DXF son idénticas.

## 13. Muy probables

Fecha, usuario y equipo en el `.lock`. El `.lock` es un archivo de bloqueo de edición de BySoft. El DXF fue generado por BySoft.

## 14. Hipótesis pendientes

- H1: BySoft crea el `.lock` al abrir o editar una pieza y lo borra al cerrarla.
- H2: un `.lock` huérfano hace que BySoft muestre la pieza como bloqueada o de solo lectura para otros usuarios.
- H3: el byte 0x00 = versión del formato de LockInfo.
- H4: el BOX es el archivo nativo de la pieza en BySoft. Por el nombre podría ser un contenedor (ZIP/OLE/serialización .NET); no hay ninguna evidencia.
- H5: `_r0` = revisión.

## 15. Desconocidos

Todo el `.box`. El significado exacto del byte 0x00 del `.lock`.

## 16–17. Pruebas realizadas y resultados

| Prueba | Resultado |
|---|---|
| Hashes de las dos copias del DXF | Idénticas: no hay una segunda muestra ni un BOX. |
| Parseo del DXF y cálculo de bbox, área y perímetro | OK, valores en §3 |
| Coherencia VPORT ↔ bbox | 51.025 = ancho/2, 17 = alto/2, 3.00147 = 102.05/34 ✔ |
| Decodificación del `.lock` como BinaryWriter | Consume exactamente los 179 bytes ✔ |
| Reserialización del `.lock` | Idéntico byte a byte ✔ |
| Decodificación del Int64 como DateTime.ToBinary | 2026-10-01 20:24:02 UTC, coherente con la subida (20:35 UTC) ✔ |
| Investigación externa | Solo encontré material comercial. No hay documentación pública del `.box` ni del `.lock`. Las páginas de Bystronic MediaCenter y SPI están bloqueadas por el proxy de este entorno. |

## 18. Hipótesis descartadas

- "El `.lock` es texto": falso, es binario.
- "El `.lock` depende del DXF o contiene un checksum de la pieza": falso, no tiene ningún campo derivado de la pieza.
- "Las dos copias del DXF son el DXF y el BOX": falso, son idénticas.

## 19. Limitaciones

- Solo hay 1 pieza y no hay BOX: no se puede hacer análisis comparativo ni diferencial.
- Solo hay 1 `.lock`: no se puede confirmar la variación de usuario o equipo.
- No hay acceso a la documentación oficial.
- El DXF de muestra no es representativo de los DXF de entrada reales.

## 20. Muestras adicionales necesarias

| Muestra | Hipótesis que permite comprobar |
|---|---|
| **El `.box` de hcmf75423_r0** (imprescindible) | Todo el formato BOX |
| BOX de 3 o más piezas: rectángulos de distintas medidas sin agujeros | Campos de ancho, alto y bbox; tamaño del archivo frente a la geometría |
| BOX de la misma pieza con 0, 1, 3 y 10 agujeros | Contadores de contornos y estructura de las listas |
| BOX de piezas con arcos, splines o texto de grabado (ENGRAVING) | Codificación de los segmentos y de las capas |
| BOX de la **misma pieza guardada dos veces** sin cambios (con minutos de diferencia) | Campos de fecha, GUID y sesión (diff puro) |
| BOX de la misma pieza con otro material o espesor | Separar los parámetros tecnológicos de la geometría |
| BOX de la misma pieza renombrada | Si el nombre está dentro del BOX |
| El **DXF original** (de CAD) que se importó en BySoft | Qué transforma BySoft al importar |
| Carpeta de la pieza **con BySoft cerrado** | H1: si el `.lock` desaparece |
| `.lock` de otro usuario o PC | Confirmar los campos de usuario y equipo |
| Versión exacta de BySoft (Ayuda → Acerca de) | Relación con el ensamblado 8.2.22.0 |
| Cómo se importa hoy: ¿BySoft lee los `.box` de una carpeta, o hay que importarlos? | Saber si basta con generar archivos o hace falta otra vía |

## 21. Viabilidad BOX

**No evaluable** hasta recibir muestras. Si resulta ser una serialización .NET propietaria con datos de tecnología o de la base de datos, generarla fuera de BySoft será arriesgado. En ese caso, alternativas a considerar:
- automatizar la importación DXF de BySoft (AutoHotkey, como `bysoft-export`, o línea de comandos si existe);
- usar alguna API o servidor de BySoft (existe `Bystronic.BySoft.Server.exe`).

## 22. Viabilidad LOCK

**Técnicamente trivial** de reproducir: es *dependiente del entorno* y *dinámico*, pero no tiene secretos ni checksum. Basta con versión + hora + usuario + equipo.

**Sin embargo, no se recomienda generarlo.** Todo indica que es un candado de edición, no un dato de la pieza. Crear `.lock` artificiales podría:
- marcar todas las piezas como "en uso" por un usuario o equipo concreto;
- bloquear la edición a otros puestos;
- dejar bloqueos huérfanos.

Hay que confirmar H1 (cerrar BySoft y mirar la carpeta) antes de decidir.

## 23. Riesgos de compatibilidad

- El BOX puede depender de la versión de BySoft (8.2.22), de la base de datos de materiales o de GUIDs internos.
- El `.lock` lleva la versión exacta del ensamblado: un cambio de versión podría invalidarlo.
- Los DXF de entrada vienen de otros CAD: requerirán normalizar unidades, entidades y capas.
- Un `.lock` falso podría bloquear la producción.

## 24–25. Arquitectura y flujo (borrador, sujeto a ver el BOX)

1. Escanear la carpeta raíz → lista de `*.dxf`, ignorando los que ya tienen `.box`.
2. Parser DXF → modelo normalizado (contornos cerrados, agujeros, bbox, unidades), con validación de contornos cerrados y entidades no soportadas.
3. Escritor BOX: módulo aislado y versionado por versión de BySoft, **solo cuando el formato esté confirmado**.
4. LOCK: por defecto **no se genera** (opción desactivada).
5. Informe por archivo: OK, avisos y errores.
6. Alternativa B, si el BOX no se puede reproducir: un driver que automatice la importación en BySoft.

## 26. Criterios de validación

1. Que el BOX generado para piezas de muestra sea igual byte a byte al de BySoft, salvo los campos dinámicos ya identificados.
2. Que BySoft **abra** el BOX generado sin avisos y muestre la geometría correcta (cotas, agujeros).
3. Que BySoft **anide y genere el CNC** con esa pieza sin errores.
4. Que el reguardado en BySoft no altere la geometría (round-trip).
5. Que los `.lock` de otros usuarios o equipos no afecten al resultado.
