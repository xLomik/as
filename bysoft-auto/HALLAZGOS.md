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
