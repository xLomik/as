# Hallazgos del diagnóstico (PC FA-PDYD023, 2026-10-01)

Fuente: `diagnostico-bysoft-cmd.txt`, generado por la parte cmd del diagnóstico, y una captura de PowerShell interactivo.

## Entorno

| Dato | Valor | Confianza |
|---|---|---|
| Windows | 10.0.19042 (20H2) | CONFIRMADO |
| PowerShell interactivo | 5.1.19041, FullLanguage | CONFIRMADO |
| PowerShell lanzado desde el `.bat` | No imprime nada y sale con código 0 | CONFIRMADO; causa DESCONOCIDA |
| Instalación | `C:\Program Files\Bystronic\BySoft CAM\Programmer\` | CONFIRMADO |
| Runtime | .NET Framework 4.7.2 | CONFIRMADO |
| Ensamblados BySoft | 8.2.22.0 y 8.1.11.0 (bindingRedirect) | CONFIRMADO |
| Licencias | Sentinel RMS, Bystronic LMU | CONFIRMADO |

## Almacén de datos: PersistenceManager

`BySoftCAM.exe.config`, `PartImporter.exe.config` y `Massmutation.exe.config` declaran:

```
persistenceManager defaultPath="c:\BystronicData\BySoftCam"
virtualRoots: AppData, System, Parts, ClusterParts, PartJobs, Tubes, TubeJobs,
              ResidualSheets, ReportTemplates, Costings, BendJobs,
              AutoPartProjects, SortJobs
```

Conclusiones:
- Las piezas, los trabajos (PartJobs = nesteos, hipótesis) y los retales están guardados en **archivos dentro de `c:\BystronicData\BySoftCam\<raíz>`**. No es una base SQL. CONFIRMADO para la configuración; la estructura en disco está PENDIENTE.
- El `.lock` analizado es `Bystronic.BySoft.Common.Persistence.LockInfo`, del **mismo ensamblado** que `PersistenceManagerSection`. Pasa de hipótesis a MUY PROBABLE: el `.lock` es el bloqueo del PersistenceManager.
- El `.box` es probablemente el objeto persistido (una pieza dentro de `Parts`). HIPÓTESIS hasta ver la carpeta.
- `AutoPartProjects` sugiere un módulo de proyectos automáticos (¿importación + nesteo automático?). HIPÓTESIS.

## ApiService

`Bystronic.BySoft.ApiService.exe.config`:

| Setting | Valor |
|---|---|
| WebAppUrl | `http://+:56111` → API HTTP en el puerto 56111 |
| ShopFloorServerBaseAddress | `http://localhost:56112` |
| ShopFloorServerTimeout | 00:01:00 |
| ApiServiceTraceSwitch | 3 (info) |

- **No está corriendo**: no aparece entre los procesos ni como servicio de Windows.
- Escuchar en `http://+:` normalmente requiere permisos de administrador o una reserva de URL (`netsh http add urlacl`). Riesgo para un usuario sin permisos de administrador.
- PENDIENTE: arrancarlo y ver si expone documentación (`/`, `/swagger`, `/help`).

## Procesos en ejecución durante el diagnóstico

BySoftCAM.exe (pid 10116) y PartImporter.exe (pid 37824). Ninguno escuchaba en un puerto TCP.

## Limitación del diagnóstico cmd

El filtro de seguridad (`findstr /v "token"`) también eliminó las líneas que contienen `publicKeyToken`. Por eso faltan los nombres de las dependencias (`assemblyIdentity`) en las configuraciones. No es grave.

---

# Ronda 2: diagnóstico PowerShell, carpeta de datos y prueba del ApiService

## Versión

BySoft CAM **1.0.0.18 Release**, "x64 Oro" según la Start View. Todos los `.exe` llevan la versión 1.0.0.18; DwgConverter, la 2.0.0.0.

## ApiService: tiene justo lo necesario, pero requiere licencia

- Al arrancarlo, muestra **"ApiService: ¡No hay ninguna licencia válida!"**. CONFIRMADO.
- Entre los textos del ejecutable aparecen `ApiServiceAutonestFeature` y `bsc_api_autonest`. Parece una **opción de licencia aparte**. MUY PROBABLE.
- Por lo que aparece en el ejecutable y en `Bystronic.BySoft.Api.Interface.dll`, la API cubre exactamente el flujo buscado: `ImportPartGeometry(WithImportConfiguration)`, `CreatePartJob`, `AddNestPart` (con `Quantity`), `AddNestSheet`, `Nest`/`CreateNestings`, `SavePartJob`, `DefaultApiPartJobsFolder`. Hay un controlador REST de ASP.NET Core con Swagger (Swashbuckle).
- Conclusión: con la licencia, el objetivo (carpeta + nombre del nesteo + Excel de cantidades) se podría hacer con llamadas HTTP, sin automatizar la pantalla.

## PartImporter: posible línea de comandos y pestaña de pedido

- Cadenas tipo argumento encontradas: `-dir`, `-log`, `/Endschnitt`. HIPÓTESIS: importación de una carpeta por línea de comandos (hay `ImportDirectory`, `ImportFileList`, `AutoImportFilter`).
- La configuración se guarda en archivos **`.pis`** (`NewPartImporterSettings.pis`, `TxtFilterPisFiles`).
- La interfaz tiene pestañas de **Orden/Pedido** (`_tabPageOrder`, `_processOrder`) y de **ajustes de nesting** (`_bsNestingSettings`, `_grpPropNesting`, `_sePropPriority`). También tiene campos `NestPart.OrderInfo`, `NestPart.UserInfo1-3` y `NestSheet.*`. HIPÓTESIS: PartImporter podría crear o alimentar un nesteo al importar.
- Componentes de soporte: `Bystronic.BySoft.Pmc.PartImporterLib.dll`.

## Otros

- `BySoftCAM.exe`: exporta `job.csv`/`part.csv` (`ExportJobs`, `JobCsvExportPath`). No se encontró una importación de pedidos por CSV.
- `PartInfoUpdater.exe`: lee CSV/Excel (`ReadCsv`, `ExcelFileFullPath`). Actualiza datos de piezas existentes.
- `Massmutation.exe`: `/Update` y CSV. Cambios masivos.
- Hay DLL de integración: `Infrastructure.Integration.CmdLine/Json/Xml/Kafka/Nats` y `Pmc.ErpDataExchange`. Indican vías de integración con ERP que todavía no se han explorado.
- Hay un módulo **Auto Part** (`Bystronic.BySoft.AutoPart.mod.dll`, raíz `AutoPartProjects`), visible en la interfaz. Sin explorar.

## Carpeta de datos `C:\BystronicData\BySoftCam`

- Cada raíz (`Parts`, `PartJobs`, `System`…) tiene un `index.db`. Hay `System.Data.SQLite.dll` instalada, así que probablemente es un índice SQLite. HIPÓTESIS.
- En `Parts` hay **un solo** `.box` (+ `.png`). Sin embargo, la Start View muestra muchas piezas y jobs. Conclusión: BySoft guarda piezas y jobs **donde elige el usuario** (por ejemplo, junto a los DXF) y el `index.db` lleva el registro. MUY PROBABLE.
- `System` contiene máquinas, materiales y costes como `.box`. La tecnología de corte está en `.PAR` (354 archivos) y `.cuttingrulesetx`.
- El `.box` es el formato general de objeto del PersistenceManager (piezas, máquinas, materiales, plantillas). Esto **refuerza la decisión de no fabricar `.box`** a mano: dependen del sistema, del índice y de la tecnología.

---

# Ronda 3: manual oficial de BySoft CAM 1.0.0

Fuente: copia del sitio onlinehelp.bystronic.com/BySoft_CAM/1.0.0/en (492 páginas).

## CONFIRMADO: Part Nester importa piezas y cantidades desde Excel

Página `PartNester/PAN_FCN_NewPartFile` (y `PAN_FCN_NewPartClipboard` para el portapapeles).
Ruta: Part Nester > Datos > Pieza > **Nueva pieza** > **Importar piezas desde archivo**.

La primera fila lleva los encabezados (texto libre). Las columnas van **en este orden**:

| Col | Contenido |
|---|---|
| A | Nombre de la pieza **o ruta completa a la pieza en la base de datos**. El ejemplo del manual usa una ruta relativa, `AU20\Sort_Part_01` |
| B | Cantidad |
| C | Info del pedido |
| D | Info 1 |
| E | Info 2 |
| F | Info 3 |
| G | Color en hexadecimal (`#FF0000`) |

Requisitos: **las piezas deben existir ya en la base de datos de piezas**, y las columnas tienen que ir en ese orden. Ejemplo: `referencia/formato-excel-importar-piezas.png`.

## Otros datos del manual

- **Import part** (Part Editor) admite varios archivos a la vez. Para importar muchas piezas, el manual remite a la herramienta **Part Importer** ("Bystronic > BySoft CAM > Tools"). La copia del manual no tiene páginas propias de Part Importer ni de Auto Part.
- Entidades DXF admitidas: ARC, CIRCLE, LINE, LWPOLYLINE, POLYLINE, VERTEX, ELLIPSE y SPLINE (las dos últimas se convierten en contorno), INSERT, TEXT y MTEXT (solo de una línea).
- **Nuevo job**: solo el **nombre** es obligatorio. Se puede crear **desde una plantilla de job** (material, máquina, ajustes) o desde el Settings Manager. Existe un generador de nombres de job.
- Pasos de un job según el manual: crear el job → añadir chapas → insertar piezas → ajustes de nesting → nestear → ajustes de exportación → exportar → guardar.
- **Almacenamiento de datos** (Archivo > Configuración del sistema > Almacenamiento de datos): cada tipo de objeto puede tener su propia ruta, incluida una unidad de red. Existe "Actualizar índice". Por eso en `C:\BystronicData\BySoftCam\Parts` hay una sola pieza: la base real de piezas está probablemente en otra ruta. PENDIENTE de verificar en el PC.

## Flujo propuesto sin licencia de API (todo con funciones oficiales)

1. **Part Importer**: importar todos los DXF de la carpeta a la base de piezas (con la configuración `.pis` de material, espesor y tecnología).
2. **Herramienta propia** (lo único que hay que programar): leer el Excel del usuario (referencia + cantidad), comprobar que cada referencia existe en la base de piezas y generar el **Excel con el formato de BySoft** (columnas A–G, con la ruta correcta en A).
3. **Part Nester**: Nuevo job (nombre, desde una plantilla) → Nueva pieza → Importar piezas desde archivo → nestear → guardar.
4. Opcional: automatizar los clics de los pasos 1 y 3 con AutoHotkey, como `bysoft-export`.

---

# Ronda 4: línea de comandos de PartImporter (CONFIRMADO al leer el código)

Fuente: PartImporter.exe 1.0.0.18 y Bystronic.BySoft.Pmc.PartImporterLib.dll, descompilados solo para saber cómo darle órdenes al programa (interoperabilidad). El código de Bystronic **no** se sube a este repositorio.

## Sintaxis

Si el programa recibe cualquier argumento, entra en **modo silencioso**: no abre ninguna ventana.
Los parámetros llevan **`=` pegado, sin espacio**, y deben escribirse **en minúsculas**:

| Parámetro | Uso |
|---|---|
| `-s=<archivo.pis>` | Configuración de importación (XML). Si no existe, sale con código 10 |
| `-dir=<carpeta>` | Importa todos los archivos admitidos de la carpeta **y de sus subcarpetas** |
| `-file=<archivo>` | Importa un solo archivo |
| `-list=<txt>` | Importa una lista de rutas (una por línea, ASCII) |
| `-log=<archivo>` | Escribe un registro de la importación |

Ejemplo: `PartImporter.exe "-s=C:\x\prueba.pis" "-dir=C:\x\dxf" "-log=C:\x\log.txt"`

Las pruebas anteriores (`-dir "..."`, con espacio) devolvían 1, que significa "Wrong or missing program parameter". Explicado.

## Códigos de salida

| Código | Significado |
|---|---|
| 0 | OK |
| 1 | Sin licencia (`bsc_cut_and_bend` + `bsc_launch`) **o** parámetros incorrectos |
| 2 | Hubo errores al importar (ver el log) |
| 3 | No se pudo crear el importador |
| 10 / 11 | El `.pis` no existe / no se pudo leer |
| 20 | `-file`: el archivo no existe. Un valor negativo indica que BySoft lo importó con otro nombre (ver `ImportSingleFile`) |
| 30 / 31 | `-dir`: la carpeta no existe / no hay archivos admitidos |
| 40 / 41 / 42 | `-list`: el txt no existe / está vacío / hubo una excepción |

Los mensajes de texto se escriben con `Console.WriteLine`, pero como es una aplicación de ventanas no se ven en cmd. Hay que fiarse del **código de salida y del log**.

## Cómo guarda las piezas

- **Nombre de la pieza = nombre del archivo sin extensión** (`Path.GetFileNameWithoutExtension`). La columna A del Excel coincide con el nombre del DXF.
- Se guardan en la base de piezas, en la subcarpeta `SavePathRelative` del `.pis`.
- Conflictos (`HandleFileConflicts` del `.pis`): `Ignore` (deja la pieza que ya existe), `Overwrite` (la sobrescribe) o `Indexing` (crea otro nombre con un índice, lo que **rompe** la correspondencia con el Excel). Para automatizar conviene Ignore u Overwrite.
- La ruta de la base sale de `PartImporter.exe.config` + `%APPDATA%\Bystronic\BySoftCam\Common.config`, la configuración del usuario. Esto explica por qué `C:\BystronicData\BySoftCam\Parts` casi no tiene piezas.
- El `.pis` es XML (`ImportSettings`): material (GUID), espesor, máquina, regla de corte, tecnología, nesting, `SavePathRelative`, `HandleFileConflicts`, campos de datos, etc.

---

# Ronda 5: estructura real de la base de piezas y de jobs (red)

Fuente: `Registros-BySoft.bat` ejecutado en las dos raíces. Los listados se cortan a 500 entradas, así que solo se ve parte de DESARROLLO.

| Base | Ruta | Carpetas | Archivos |
|---|---|---|---|
| Piezas | `\\fnsrvnas\Planos\PLANOS_DIBUJO_FANALCA\BLANCOS\LASER\BystronicData\BySoftCam\Parts-FANALCA` | 1260 | 7771 (7661 `.box`, 98 `index.db`, 6 `.lock`) |
| Jobs | `...\BySoftCam\PartJobs-FANALCA` | 1015 | 2401 (2337 `.box`, 47 `index.db`, 12 `.lock`) |

## Convención de carpetas (la crea el usuario a mano)

```
<Raíz>\{DESARROLLO|PRODUCCION}\<NNN. Proyecto o Ref>\<espesor + material>\pieza.box
```

Ejemplos: `DESARROLLO\108. Ref.10510104935\4.5 HARDOX-450`, `DESARROLLO\111. Ref 10510096795\6.0 SAEJ 080`, `DESARROLLO\F.2509-078\6.35 mm`, `DESARROLLO\Carrocerias\MAYO 2024 ...\4.5`.

- **La misma estructura se repite en Parts y en PartJobs.**
- La subcarpeta de espesor y material tiene formato libre: `4.5 HARDOX-450`, `3.42mm SAEJ 050`, `6 mm`, `GR 50; E=3.00mm`. No hay un patrón fijo.
- Hay `index.db` en la raíz y en algunas subcarpetas. Hay archivos `.lock` sueltos (piezas abiertas o bloqueos huérfanos).
- La importación de prueba por línea de comandos dejó las 2 piezas de prueba en la **raíz** de Parts-FANALCA, porque el `.pis` tenía `SavePathRelative` vacío. CONFIRMADO.

## Implicaciones para la herramienta

- Crear `<Área>\<Proyecto>\<Espesor Material>` en Parts-FANALCA, y opcionalmente también en PartJobs-FANALCA.
- Poner `SavePathRelative = <Área>\<Proyecto>\<Espesor Material>` en una copia del `.pis` → PartImporter `-s= -dir=`.
- Columna A del Excel de BySoft = `<ruta relativa>\<pieza>` (por confirmar en la prueba del Part Nester).
- PENDIENTE: confirmar que BySoft reconoce las carpetas creadas con el Explorador o por script (no desde BySoft), y cómo aparece `SavePathRelative` escrito en el XML del `.pis`.

---

# Ronda 6: Pruebas-BySoft.bat en el entorno real

- Las carpetas `DESARROLLO\PRUEBA_AUTO` se crearon con el script en Parts-FANALCA y PartJobs-FANALCA. OK.
- **Formato de `.pis` CONFIRMADO**: XML con atributos en `<ImportSettings ...>`. La carpeta de destino es el **atributo** `SavePathRelative="/DESARROLLO/PRUEBA_AUTO/"`, con **barras normales** al principio y al final. Los demás atributos: `MaterialGuid`, `Thickness`, `CuttingMachineGuid`, `CuttingRuleGuid`, `NcParameterFile`, `CuttingGasTypeGuid`, `MoveToOrigin`, `AutoRotation`… `HandleFileConflicts` **no aparece**, así que se aplica el valor por defecto `Ignore`.
- Importación por línea de comandos: código 0, pero el log dice "Ya existe un archivo para la pieza X -> ¡La pieza no se tiene en cuenta!". En `PRUEBA_AUTO` hay `.box`. Probablemente se importaron antes desde la interfaz. Según el código, si el índice exige **nombres únicos**, la búsqueda de duplicados recorre **toda la base** (también las piezas de la raíz). HIPÓTESIS: los nombres de pieza deben ser únicos en toda la base.
- **Part Nester → Importar piezas desde archivo**: error `No se puede convertir un objeto de tipo 'System.Double' al tipo 'System.String'` en `PartNesterModulesHelper.Commands.Data.NestPartCmd.ProcessExcelRow`. Alguna celda numérica (casi seguro la cantidad) se convierte con un cast a string. Siguiente prueba: un Excel con **todas las celdas como texto**.

---

# Ronda 7: lector de Excel del Part Nester (CONFIRMADO al leer el código)

Fuente: `Bystronic.BySoft.ModulesHelper.PartNesterModulesHelper.dll` (`NestPartCmd.ReadExcelFile` / `ProcessExcelRow`, `ExcelMaterialListImportHelper`).

- Abre el `.xlsx` con **Excel por COM**, así que Excel tiene que estar instalado. Lee la **hoja activa** y su `UsedRange`, **todas las filas**. La fila de encabezado también se procesa: se le busca pieza y no se encuentra.
- Columna A: `Value2.ToString()`, acepta cualquier tipo.
- **Columna B (cantidad): `(string)Value2` + `int.TryParse`.** Si la celda es numérica, el cast lanza el error `Double → String`. **La cantidad tiene que ser TEXTO.** Si no se puede convertir, se toma 0.
- Columnas C–F: `(string)Value2`. Tienen que ser texto o estar vacías.
- **La columna G (color) no se lee** desde un archivo.
- Búsqueda de la pieza (`CheckPartsImport` / `GetPartFromFullPathInBySoft`):
  - **Sin carpeta** (solo el nombre): `GetObjectInfos<Part>(nombre)`. Si hay **más de 1** pieza con ese nombre en toda la base, da el error "piezas repetidas".
  - **Con carpeta**: se toma `Path.GetDirectoryName`, se quita la raíz física y la `\` inicial, y se busca en `"/" + carpeta + "/"`. **Las `\` interiores no se convierten a `/`**, así que una ruta de 2 o más niveles (`DESARROLLO\PRUEBA_AUTO`) se busca como `/DESARROLLO\PRUEBA_AUTO/` y **no se encuentra**. El ejemplo del manual (`AU20\Sort_Part_01`) tiene un solo nivel. MUY PROBABLE que sea un bug de BySoft para rutas de varios niveles. `GetDirectoryName` también convierte `/` en `\`, así que ninguna variante lo evita.
- **Conclusión de diseño**: la columna A lleva **solo el nombre** de la pieza. La herramienta debe garantizar que el nombre sea **único** en Parts-FANALCA, comprobándolo antes de generar el Excel. Todas las celdas se escriben como texto.

---

# Ronda 8: importación del Excel en el Part Nester. Comportamiento peligroso

Código (`ExcelMaterialListImportHelper.LoadNestPartsForAdding`): si **no encuentra** la pieza por nombre, **crea una pieza VACÍA** con ese nombre (`new Part { Name = ... }`), **la guarda en la base de piezas** (`BySoft7DataAccess.SaveNewPart`) y la añade al job. **No avisa.**

Consecuencias:
- **La fila de encabezado del Excel se procesa como una pieza**, así que se crea una pieza vacía "Part name" en la base. Observado: el job mostró una fila "Part name". **El Excel generado NO debe llevar encabezado.**
- Una referencia mal escrita, o una pieza que todavía no se ha importado, crea una pieza vacía en la base. Después, esa pieza vacía **duplica el nombre** cuando se importe la real y provoca el error "repetidas".
- La herramienta **debe comprobar que cada pieza existe** (y que es única) **antes** de que el Excel llegue a BySoft.

Observado en la prueba TEST_AUTO: el job tiene 3 filas ("Part name", TEST_AUTO_01, TEST_AUTO_02), todas con un indicador rojo. PENDIENTE: saber si TEST_AUTO_01/02 eran las piezas reales o vacías (depende de que el import por línea de comandos se hubiera hecho antes).

---

# Ronda 9: ¿quién crea la carpeta destino? (PC real)

- ZIP v1: si `DESARROLLO\PRUEBA_AUTO2` no existe, PartImporter falla con el código 2 ("No se puede encontrar una parte de la ruta…" en `PersistenceManager.DoSave`). **CONFIRMADO: PartImporter NO crea carpetas.**
- La carpeta `PRUEBA_AUTO` de la ronda 6 la creó el usuario con el botón "Crear nueva carpeta" del Part Importer. Ese botón llama a `FolderInfo.CreateSubfolder` (API de persistencia de BySoft), no a `mkdir`.
- ZIP v2: el `.bat` crea `PRUEBA_AUTO2` con `mkdir` y después importa. Código 0, las 2 piezas se guardaron (`.box` + `.png`). **CONFIRMADO: se puede importar en una carpeta creada con mkdir.**
- PENDIENTE: comprobar si BySoft (Abrir pieza / Excel del Part Nester) ve `PRUEBA_AUTO2` y sus piezas **sin "Actualizar índice"**. Si no las ve, plan B: crear la carpeta con `FolderInfo.CreateSubfolder` desde las DLL de BySoft.
- **Resultado (diálogo Abrir de BySoft, sin actualizar el índice): `PRUEBA_AUTO` (creada con CreateSubfolder) SÍ aparece; `PRUEBA_AUTO2` (creada con mkdir) NO aparece.** CONFIRMADO: una carpeta creada con mkdir no entra en el índice. La herramienta debe crear las carpetas con `FolderInfo.CreateSubfolder` (Bystronic.BySoft.Common.Persistence.dll).
- PENDIENTE: saber si las **piezas** guardadas por PartImporter en esa carpeta sí quedaron indexadas (búsqueda por nombre / Excel del Part Nester).

---

# Ronda 10: crear carpetas con la API de persistencia (Bystronic.BySoft.Common.Persistence.dll)

- `FolderInfo.CreateSubfolder(nombre)` = `DirectoryInfo.CreateSubdirectory` + `index.InsertFolder(LocalPath)`. CONFIRMADO en el código.
- El índice es **SQLite** (`index.db`), con las tablas `FolderInfo(FolderId, PathUC, Path, ParentId)` y `ObjectInfo(Guid, FolderId, TypeName, Name, ...)`. Si `VirtualRoot.IndexServiceHost` está configurado, usa un índice remoto (`RemoteIndex`/`IndexServiceClient`) en lugar del archivo.
- `IndexDb.InsertObject` busca el `FolderId` de la carpeta. Si la carpeta no está en el índice, la pieza se inserta **sin carpeta (FolderId 0)**. Por eso las piezas AUTOTEST_A/B quedaron **huérfanas** en el índice.
- `PersistenceManager.GetRootFolder("Parts")` permite obtener la raíz por nombre, sin depender del tipo `Part`.
- Herramienta `BySoftCarpeta` (C# 5, .NET Framework 4.7.2): carga las DLL de BySoft desde la carpeta de instalación (`AssemblyResolve`), se configura igual que PartImporter (`exe.config` copiado de `PartImporter.exe.config` + `%APPDATA%\Bystronic\BySoftCam\Common.config`) y recorre la ruta: `ExistsFolder` → `GetFolderFromLocalPath`, o `CreateSubfolder`. Si el directorio ya existe en disco pero no en el índice, solo lo registra. Compila con Roslyn contra las referencias de net472 y la DLL real. En el PC no está probada.

---

# Ronda 11: prueba 3 en el PC real. FLUJO COMPLETO CONFIRMADO ✅

- `BySoftCarpeta.exe` (el exe compilado aquí) funcionó en el PC: leyó `Common.config` del usuario y devolvió las bases de red de Parts y PartJobs, que coinciden con las esperadas.
- Creó `DESARROLLO\PRUEBA_AUTO3` en Parts-FANALCA y PartJobs-FANALCA con `CreateSubfolder`, y **registró en el índice `PRUEBA_AUTO2`** (que se había creado con mkdir).
- **Abrir pieza, sin "Actualizar índice"**: aparecen `PRUEBA_AUTO2` y `PRUEBA_AUTO3`. `PRUEBA_AUTO3` contiene `AUTOTEST_C_R0` y `AUTOTEST_D_R0`. CONFIRMADO.
- PartImporter por línea de comandos, con `SavePathRelative="/DESARROLLO/PRUEBA_AUTO3/"`: código 0, las 2 piezas se guardaron. CONFIRMADO.
- **Part Nester → Importar piezas desde archivo** (sin encabezado, todo texto, solo el nombre): AUTOTEST_C_R0 y AUTOTEST_D_R0 entran con su geometría (miniatura visible). CONFIRMADO. Indicador amarillo/rojo pendiente de interpretar (¿cantidad sin nestear?).
- La compilación local con csc falló con CS0012: falta `/r:Bystronic.BySoft.Common.dll`. Corregido en el `.bat`. El exe incluido sirvió de respaldo.

Cadena confirmada: **BySoftCarpeta (crear carpeta en Parts y PartJobs) → copia del `.pis` con SavePathRelative → PartImporter -s= -dir= → Excel (nombre, cantidad como texto, sin encabezado) → Part Nester "Importar piezas desde archivo"**.

---

# Ronda 12: requisitos del usuario y diseño de la herramienta

Respuestas del usuario:
1. El formato del Excel de cantidades lo definimos nosotros: `formato/FORMATO_CANTIDADES.xlsx` (Referencia | Cantidad | Observacion).
2. El área, el proyecto y el nombre del programa los decide el usuario a mano. Enviará su formato de nombres.
3. La carpeta de DXF tiene subcarpetas por material y espesor. En la base real hay nombres muy variados: "6", "4.5", "3.42 SAEJ 050", "SAEJ 080 Esp=6mm", "ASTM A36 Esp=6,35mm", "JIS G 3141 SPCD SD Esp=2mm", "SAPH-440; E=4.5mm"… La convención propuesta es `<MATERIAL> Esp=<espesor>mm` (la forma más común), con punto decimal.
4. **El usuario no usa `.pis`**: configura el Part Importer a mano en cada importación. La herramienta tiene que **generar el `.pis`** para cada subcarpeta.
5. Basta con dejar las piezas importadas y el Excel generado. El job lo crea el usuario.

Código del Importer, sobre lo que necesita el `.pis`:
- `MaterialGuid` → busca en la raíz de `Material`. Si no lo encuentra, crea un `new Material()` vacío (registra LogMaterialNotFound). El gas se toma de la tabla de espesores del CuttingMaterial.
- `CuttingMachineGuid` → la máquina. `CuttingRuleGuid` → el asistente de corte (en System hay ByFiber_N2 y ByFiber_O2).
- **`NcParameterFile` puede quedar vacío**: el importador elige el `.PAR` automáticamente con la ruta NC de la máquina + material + espesor (`TryCalculateCuttingTime`).
- Siguiente paso: **catálogo** (modo `BySoftCarpeta catalogo System`), que lista materiales, máquinas, reglas y gases con su GUID leyendo solo el índice. Con él se arma la tabla de equivalencias "alias de la subcarpeta → MaterialGuid / regla de corte".

---

# Ronda 13: catálogo de System, registro de programas y DomainModel

## Catálogo (BySoftCarpeta catalogo System)
- La base **System es LOCAL**: `c:\BystronicData\BySoftCam\System`. No está en la red.
- **Materials** (los que usa el `.pis`, campo `MaterialGuid`): 1.4031 (inox), ASTM (A1011), AW5083, AW5754, AW6082 (aluminio), **DC01** `32d65efb-…` (acero suave, el del `prueba2.pis`), DC01+ZE (galvanizado), DD11, RuukkiLaser250C. Son **materiales genéricos de Bystronic**: SAEJ 050, SAPH-440, HARDOX… no existen como tales. Hay que hacer una **tabla de equivalencias** "material comercial → material BySoft".
- CuttingMachine: BYSPRINT FIBER 4020 `cfcb40d8-…`. CuttingRuleSet: ByFiber_O2 `03e10b39-…` y ByFiber_N2 `d034b4e3-…`.
- DomainModel: `Material` no tiene espesor (densidad, imán, CuttingMaterial, grupo). El espesor va aparte (`Thickness` del `.pis`). El gas sale de `CuttingMaterial.Thicknesses`.

## Registro de programas (LISTADO_PROGRAMAS_LASER-FANALCA.xls)
- Hojas `LASER-PRODUCCION` (prefijo LP) y `LASER-DESARROLLO` (prefijo LD). Columnas: FECHA CREACION | CODIGO (LP/LD) | MES (texto "09") | AÑO (texto "26") | consecutivo (número) | [REVISION "R0", solo en PRODUCCION] | NOMBRE | REFERENCIA | ESP | MATERIAL | PROY/CLIENTE.
- **Nombre del programa = prefijo + MES + AÑO + consecutivo**. Ejemplos: LP0926724, LD0826770. Concuerda con los jobs de la Start View. El consecutivo **no se reinicia** por mes. Último usado: LP 731 (09/26), LD 804 (10/26).
- **Un programa por material y espesor**, que coincide con una subcarpeta.
- Hay filas pre-numeradas sin nombre (LP hasta 897, LD hasta 895): el siguiente libre es la primera fila con NOMBRE vacío.

---

# Ronda 14: parámetros de corte y regla de espesor (usuario + parametros-bysoft.txt)

- Los `.PAR` que se usan están en `C:\BystronicData\BySoftCam\8109C_BySprint_Fiber_4020_6000_BIMO2_POWERCUT\Parametros_europa`. Formato del nombre: `SPRINT4020_8109_6000_<MAT>_<ESP>_200_<GAS>[_SUFIJO].PAR`.
- Acero (gas O2, sin sufijo): **DC01** = 0.8, 1, 1.5, 2, 2.5, 3; **DD11** = 4, 5, 6, 8, 10, 12. Inox 1.4301 (N2): 0.8–8 sin sufijo, 10+ con sufijos. Aluminio AW5754: N2 con o sin QUALITY/SPEED. Galvanizado DC01+ZE N2: 0.8–4.
- **Regla del usuario**: se elige el **espesor inmediatamente superior** que tenga `.PAR` (3.2 → 3.5 si existe, si no 4). Para el acero, el material sale de ese `.PAR`: ≤3 → DC01, ≥4 → DD11.
- La lista de espesores del desplegable (DD11: 3.5, 4.5, 6.35, 12.7…) sale de la tabla del CuttingMaterial, no de los `.PAR`. El `.PAR` que se elige es el de espesor superior (por ejemplo, 4.5 → DD11_5).
- Código: `NcParameterFile` solo se usa para calcular el tiempo de corte. La tecnología la aplica el CuttingRuleSet (ByFiber_O2/N2).

---

# Ronda 15: AutoBySoft v1 (herramienta completa)

Código en `app/` (ver `app/README.md`). Pruebas hechas aquí, sin BySoft:
- Interpretación de subcarpetas y elección de `.PAR` con los 129 `.PAR` reales de Parametros_europa: 3.42→DD11 4, 4.5→DD11 5, 6.35→DD11 8, 8.25→DD11 10, 2.9→DC01 3, "6"→ACERO asumido DD11 6, inox 2→1.4301_2_N2, aluminio 3→AW5754_3_N2_QUALITY, 12.7→sin `.PAR` (error).
- Excel de cantidades: suma de repetidas, cantidades no enteras y filas sin referencia dan error. El Excel de salida se relee con openpyxl y todas sus celdas son texto. LibreOffice no funciona en este entorno, así que no sirvió para validar.
- `.pis` generado: cambia MaterialGuid/Thickness/Machine/Rule/NcParameterFile/SavePathRelative, quita CuttingGasTypeGuid y deja HandleFileConflicts por defecto (Ignore).
- Revisión de un pedido simulado: detecta DXF suelto, subcarpeta sin espesor, DXF en dos subcarpetas, referencia sin DXF, DXF sin cantidad y extensión .DXF en mayúsculas.
- PENDIENTE (requiere el PC real): conexión con BySoft, lectura del listado `.xls` por COM, la ventana en Windows y la ejecución completa.
- Duda abierta: espesores mayores que el máximo `.PAR` de la familia (ACERO > 12 mm, ej. 12.7 o 19) dan error. ¿Qué hace el usuario en esos casos?
