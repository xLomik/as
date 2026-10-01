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
