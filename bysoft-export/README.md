# Exportar PDF + XLS de BySoft con un solo atajo

Automatiza el doble export de BySoft. Con la **Vista previa** abierta, pulsas
**Ctrl+Alt+E**, escribes el nombre del trabajo **una vez**, y se generan el
**PDF** y el **XLS** (el XLS nativo de BySoft: idéntico, editable y con el dibujo).

> No reconstruye nada desde el PDF. Usa los exportadores propios de BySoft, así
> que el Excel sale **igual** al que haces hoy a mano — solo que con un atajo.

---

## Instalación (una vez)

1. Instala **AutoHotkey v2** (gratis, oficial): https://www.autohotkey.com/
   (Descarga → "Download v2.0" → instala. ~5 MB.)

## Calibrar (una vez, ~1 minuto)

El script necesita saber dónde está el botón **"Exportar como"** en tu pantalla.

1. Abre BySoft con la **Vista previa** del Job, **maximizada**.
2. Doble clic en **`Calibrar.ahk`**.
3. Pon el mouse **encima** del botón **"Exportar como"** (sin hacer clic).
4. Pulsa **F8**.
5. Sale una ventana con `EXPORT_BTN_X` y `EXPORT_BTN_Y` (ya copiados).
6. Abre **`ExportarPDFyXLS.ahk`** con el Bloc de notas y **pega** esos dos
   valores en la sección CONFIGURACIÓN (reemplaza los que están).
7. Guarda el archivo.

## Uso diario

1. Doble clic en **`ExportarPDFyXLS.ahk`** (queda corriendo en la bandeja del
   sistema, junto al reloj). Solo hace falta una vez por sesión de Windows.
2. En BySoft, abre la **Vista previa** del trabajo.
3. Pulsa **Ctrl+Alt+E**.
4. Escribe el nombre (ej. `LP0626652`) → Enter.
5. Se abre un **selector de carpeta**: elige la carpeta de la solicitud
   (la nueva que creaste). Arranca en la última que usaste. → Aceptar.
6. El script guarda **PDF y XLS solos** en esa carpeta. ✅ **No navegas** los
   diálogos de "Guardar como".

Resultado: `LP0626652.pdf` y `LP0626652.xls` juntos, con un atajo, un nombre y
un clic de carpeta.

---

## Ajustes (si algo no calza)

Edita la sección **CONFIGURACIÓN** arriba de `ExportarPDFyXLS.ahk`:

| Variable | Para qué | Si falla... |
|----------|----------|-------------|
| `PREVIEW_TITLE` | Título de la ventana de BySoft | Cámbialo si tu ventana se llama distinto |
| `EXPORT_BTN_X/Y` | Posición del botón "Exportar como" | Re-calibra con `Calibrar.ahk` |
| `PDF_DOWN` | Nº de flechas ↓ hasta "Documento PDF" | Si exporta el formato equivocado, prueba `0` o `2` |
| `XLS_DOWN` | Nº de flechas ↓ hasta "Documento XLS" | Cuenta los ítems del menú: PDF=1, HTML=2, MHT=3, RTF=4, DOCX=5, **XLS=6** |
| `OPCIONES_TITLE` | Diálogo de opciones de export | Debe coincidir con "Opciones de Exportación..." |
| `GUARDAR_TITLE` | Diálogo de guardado | En algunos Windows es "Guardar como" |

Cambiar el atajo: la línea `^!e::DobleExport()` → `^!e` = Ctrl+Alt+E.
(`^`=Ctrl, `!`=Alt, `+`=Shift). Ej. `^!x` = Ctrl+Alt+X.

---

## Por qué selector de carpeta (y no automático por número)

La carpeta de cada solicitud **no se deduce del número** `LP`/`LD`: tu
estructura es por solicitud (`43-RQI-41182;...`) → espesor/material
(`4.5 SAEJ 060`) → a veces `PROGRAMAS LASER`. El número es el nombre del
programa, no de la carpeta. Por eso el script usa un **selector** que recuerda
la última carpeta: eliges la nueva con un clic y listo.

Posible mejora futura: que el selector **sugiera la carpeta recién creada**
(la más reciente bajo la raíz LASER). Se puede agregar si lo quieres.
