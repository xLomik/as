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
5. En el **primer** "Guardar como": navega a la carpeta del trabajo y pulsa
   **Guardar** (el nombre ya está escrito).
6. El **XLS** se guarda **solo** en esa misma carpeta. ✅

Resultado: `LP0626652.pdf` y `LP0626652.xls` juntos, con un atajo y una sola
navegación de carpeta.

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

## Próximo paso: carpeta automática (opcional)

Hoy navegas a la carpeta **una vez** por trabajo. Si tu carpeta se puede
**deducir del número** (ej. `LP0626652` → siempre la misma ruta por una regla),
el script puede ir **directo** sin que navegues. Para eso hace falta tu
estructura de carpetas. Compártela y lo activamos.
