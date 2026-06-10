# PDF-a-Excel — Lista de órdenes (Bystronic) → Excel

Convierte automáticamente los reportes **PDF "Lista de órdenes"** de la
cortadora láser (BySoft / BYSPRINT) a un **Excel (.xlsx)** con el **mismo
layout** del reporte.

> **Regla:** solo procesa los PDF cuyo **nombre empieza por `LD` o `LP`**
> (por ejemplo `LP0626652.pdf`, `LD12345.pdf`). Los demás se ignoran.

El `.xlsx` se guarda **junto al PDF**, con el mismo nombre
(`LP0626652.pdf` → `LP0626652.xlsx`).

---

## Uso rápido (programa portable, sin instalar nada)

Descarga `PDF-a-Excel.exe` (ver **"Obtener el .exe"** abajo). Es **portable**:
cópialo a cualquier PC con Windows; **no requiere instalar Python ni nada**.

Tienes varias formas de usarlo:

| Quiero... | Cómo |
|-----------|------|
| Convertir **un PDF** | Arrastra el PDF **encima** de `PDF-a-Excel.exe` |
| Convertir **varios PDF** | Selecciónalos y arrástralos todos sobre `PDF-a-Excel.exe` |
| Convertir **una carpeta entera** | Arrastra la carpeta sobre el `.exe`, o pon `Convertir-PDFs-aqui.bat` dentro de la carpeta y haz doble clic |
| **Automático** (vigilar carpeta) | Pon `Vigilar-carpeta.bat` en la carpeta donde caen los PDF y déjalo abierto: convierte cada PDF nuevo en cuanto aparece |

### Modo automático (vigilancia)

```
PDF-a-Excel.exe --watch "C:\ruta\a\la\carpeta"
```

Queda vigilando esa carpeta y convierte cada PDF `LP*`/`LD*` nuevo o
modificado. Ideal para apuntarlo a la carpeta donde la máquina deja los PDF.
Cierra con `Ctrl+C`.

### Carpeta de salida distinta

Por defecto el `.xlsx` se crea junto al PDF. Para mandarlos a otra carpeta:

```
PDF-a-Excel.exe -o "C:\salida" "C:\entrada"
```

---

## Obtener el .exe

### Opción A — Descargarlo ya compilado (recomendado)

Lo compila GitHub Actions automáticamente:

1. En GitHub, pestaña **Actions** → workflow **"Build Windows EXE"**.
2. Abre la última ejecución en verde → sección **Artifacts**.
3. Descarga **`PDF-a-Excel-windows`** (trae el `.exe` + los `.bat` + este README).

> También se adjunta a una **Release** si se publica un tag `vX.Y.Z`.

### Opción B — Compilarlo tú mismo en Windows

Necesitas Python 3.9+ instalado (solo para compilar). Luego:

```
build_exe.bat
```

Genera `dist\PDF-a-Excel.exe`.

---

## Ejecutar desde el código (sin compilar)

```bash
pip install -r requirements.txt
python pdf_to_excel.py archivo.pdf          # un PDF
python pdf_to_excel.py C:\carpeta            # una carpeta
python pdf_to_excel.py --watch C:\carpeta    # automático
```

---

## ¿Qué extrae?

Reproduce el reporte completo en su layout original:

- **Cabecera:** JOB, Máquina, Descripción, Material, Grosor, Tecnologías,
  Gas de corte, Desperdicio, tiempos (corte / improductivo / total).
- **Chapas:** Dimensión X/Y, Peso, Cantidad.
- **Pieza:** nombre, nominal/efectivo, descripción, dimensiones, superficie,
  peso, perímetro, tiempos.
- **Planos de corte:** nombre, pasadas, desperdicio, dimensiones de plano y
  chapa, tiempos.
- **Tabla de piezas:** N°, nombre, dimensiones, peso, tiempo de corte, cantidad.
- **Pie:** empresa, usuario, fecha.

Las tablas (chapas / piezas) crecen automáticamente si el reporte trae varias filas.

## Nota sobre precisión de los números

El Excel se construye **a partir del texto del PDF**, así que los números
quedan con la **misma cantidad de decimales que muestra el PDF**
(p. ej. `51,94%`, `0,162 kg`). El archivo nativo de la máquina puede traer
más decimales internos, pero esos **no están en el PDF** y no se pueden
recuperar desde él. La `Superficie efectiva` se toma tal cual aparece en el
PDF (en `m²`).

## Limitaciones

- Diseñado para el formato Bystronic **"Lista de órdenes"**. Otros reportes
  con distinto layout no se parsean igual.
- El PDF debe tener **texto** (no ser un escaneo/imagen). Estos reportes de
  máquina ya vienen con texto, así que funciona directo.
