# AutoBySoft

Herramienta de escritorio (WinForms, .NET Framework 4.7.2, C# 5). A partir de la carpeta de un pedido (subcarpetas `MATERIAL Esp=Xmm` con DXF) y del Excel de cantidades:

1. Valida todo **sin modificar BySoft**.
2. Por cada subcarpeta (un programa LP/LD):
   - crea la carpeta en Parts y PartJobs con `FolderInfo.CreateSubfolder`;
   - genera el `.pis`;
   - importa con `PartImporter.exe -s= -dir= -log=`;
   - verifica que las piezas quedaron en BySoft;
   - genera el Excel para el Part Nester.
3. Genera las filas para el listado de programas.

Toda la investigación que respalda cada decisión está en [`../HALLAZGOS.md`](../HALLAZGOS.md). Las instrucciones de uso están en [`LEEME.txt`](LEEME.txt).

## Estructura

| Ruta | Contenido |
|---|---|
| `src/Inicio.cs` | Arranque: usa `PartImporter.exe.config` como config propia y resuelve las DLL desde la carpeta de BySoft |
| `src/Ventana.cs` | Interfaz |
| `src/Motor.cs` | Revisar / Ejecutar |
| `src/Reglas.cs` | Reglas puras: subcarpetas, materiales, elección de `.PAR` (espesor igual o superior), Excel de cantidades |
| `src/BySoftApi.cs` | API de persistencia de BySoft (carpetas, búsqueda por nombre, catálogo) |
| `src/Pis.cs` | Genera el `.pis` a partir de `config/plantilla.pis` |
| `src/Listado.cs` | Lee el siguiente consecutivo del listado `.xls` (Excel por COM, solo lectura) |
| `src/Xlsx.cs` | Lectura y escritura de `.xlsx` sin Excel (todas las celdas como texto) |
| `config/` | `AutoBySoft.ini`, `materiales.txt`, `tecnologia.txt`, `plantilla.pis` |
| `build/compilar.ps1` | Compila con Roslyn (PowerShell 7 + referencias net472 + `Bystronic.BySoft.Common.Persistence.dll`) |
| `build/empaquetar.py` | Arma `dist/AutoBySoft.zip` |
| `Compilar-en-este-PC.bat` | Compila en el PC del usuario con el `csc.exe` de Windows |

Las DLL de Bystronic **no** se incluyen en el repositorio ni en el ZIP: se cargan desde la instalación de BySoft.
