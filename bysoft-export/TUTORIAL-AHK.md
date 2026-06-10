# Aprende AutoHotkey v2 (para automatizar BySoft tú mismo)

Guía corta y práctica. La idea: aprendes a **leer** la ventana (títulos,
botones, posición del mouse) y a **mandar** teclas/clics. Con eso armas
cualquier macro y la arreglas solo cuando algo cambia.

---

## 0) Idea clave

AutoHotkey hace 2 cosas:
1. **Lee** la pantalla: títulos de ventana, controles, dónde está el mouse.
2. **Manda** acciones: teclas (`Send`), clics (`Click`), esperas (`Sleep`).

Automatizar = leer dónde estás → mandar la acción correcta → esperar → repetir.

---

## 1) Instalar

- Baja AutoHotkey **v2** de https://www.autohotkey.com/ e instala.
- Trae una utilidad clave: **Window Spy** (la usamos en el paso 4).

## 2) Crear y correr un script

1. En el escritorio: clic derecho → **Nuevo → AutoHotkey Script** (o crea un
   `.txt` y renómbralo a `prueba.ahk`).
2. Clic derecho al `.ahk` → **Edit** (ábrelo en Bloc de notas).
3. Pega esto:

```ahk
#Requires AutoHotkey v2.0

^!j::MsgBox("Hola. Funciona.")
```

4. Guarda. Doble clic al `.ahk`. Aparece un ícono verde **H** en la bandeja
   (junto al reloj) = está corriendo.
5. Pulsa **Ctrl+Alt+J** → sale el mensaje. ¡Listo!

`^!j` = el atajo. **`^`=Ctrl, `!`=Alt, `+`=Shift, `#`=Windows**.
Ej: `^+s` = Ctrl+Shift+S. La parte tras `::` es lo que ejecuta.

## 3) Recargar / cerrar

Cada vez que editas el `.ahk`, debes **recargar**:
- Clic derecho al ícono H en la bandeja → **Reload Script** (o ciérralo y
  doble clic otra vez).
- **Exit** para cerrarlo.

Si hay error de sintaxis, sale una ventanita roja diciendo línea y motivo.
(Ej clásico: comentario `;` pegado a un texto sin espacio antes → error.)

## 4) Window Spy — TU HERRAMIENTA #1

Sirve para **leer** lo que el script necesita: el título exacto de una
ventana, el nombre de un control (campo de texto, botón) y la posición del
mouse.

1. Clic derecho al ícono H en la bandeja → **Window Spy**
   (o búscalo en la carpeta de instalación de AHK).
2. Deja Window Spy abierto. Pasa el mouse sobre la ventana/botón que te
   interesa. Window Spy muestra en vivo:
   - **Window Title** → el título exacto (ej. `Exportar como`, `Vista previa`).
   - **Mouse Position** → coordenadas. Mira la fila **In Active Window**
     (esas son relativas a la ventana, las que usa `CoordMode "Window"`).
   - **Control under Mouse** → el nombre del control (ej. `Edit1`, `Button2`).

> Truco: para "congelar" lo que estás viendo y poder leerlo, en Window Spy
> normalmente se usa la tecla que indica abajo, o simplemente mueve el mouse
> despacio. Anota el Título y el Control.

**Esto resuelve el 90% de los bugs**: si el script "no hace nada", casi
siempre es porque el **título** o el **control** que pusiste no coincide con
el real. Window Spy te dice el real.

## 5) Comandos que necesitas (v2)

```ahk
Sleep 300                       ; esperar 300 milisegundos

Send "{Enter}"                  ; mandar Enter
Send "{Down 6}"                 ; flecha abajo 6 veces
Send "hola"                     ; escribir texto
SendText "C:\ruta\archivo.pdf"  ; escribir texto TAL CUAL (sin interpretar)

CoordMode "Mouse", "Window"     ; clics relativos a la VENTANA (recomendado)
Click "1130 70"                 ; clic en x=1130 y=70 (un solo string)

WinWait "Exportar como", , 10   ; esperar hasta 10s a que aparezca esa ventana
WinActivate "Exportar como"     ; traerla al frente
if WinExist("Exportar como")    ; ¿existe esa ventana?

; Escribir en un campo de un diálogo (sin tocar el mouse):
ControlSetText "C:\ruta\arch.pdf", "Edit1", "Exportar como"
ControlFocus "Edit1", "Exportar como"

ib := InputBox("Nombre del trabajo:", "Titulo", "w300 h120")
nombre := ib.Value              ; lo que escribió el usuario

carpeta := DirSelect()          ; selector de carpeta; "" si cancela

MsgBox("valor = " . variable)   ; ver un valor (para depurar)
ToolTip("paso 1 ok")            ; mensajito flotante
ToolTip()                       ; quitarlo
```

Reglas:
- Arriba siempre: `#Requires AutoHotkey v2.0`.
- Comentario `;` **debe llevar un espacio antes** si va al final de una línea
  con código. `x := 5  ; bien` / `x := 5; MAL`.
- Unir textos con `.` → `carpeta . "\" . nombre . ".pdf"`.

## 6) Atajo solo en cierta ventana

```ahk
#HotIf WinActive("Vista previa")   ; el atajo solo vale si esa ventana está activa
^!e::MiFuncion()
#HotIf                             ; cierra la condición

MiFuncion() {
    MsgBox("Solo salgo si 'Vista previa' está al frente.")
}
```

## 7) Método para armar el macro: POR PARTES

No escribas todo de una. Prueba **pieza por pieza** con `MsgBox`/`ToolTip`:

**Paso A — clic en "Exportar como":**
```ahk
#Requires AutoHotkey v2.0
^!e:: {
    CoordMode "Mouse", "Window"
    Click "1130 70"        ; <-- pon TUS coordenadas (Window Spy, paso 4)
}
```
Recarga, abre la Vista previa, pulsa Ctrl+Alt+E. ¿Se abrió el menú "Exportar
como"? Si el clic cae en otro lado, corrige las coordenadas.

**Paso B — elegir el formato (PDF):**
```ahk
^!e:: {
    CoordMode "Mouse", "Window"
    Click "1130 70"
    Sleep 300
    Send "{Down 1}"        ; "Documento PDF" = 1er item del menú
    Send "{Enter}"
}
```
¿Eligió PDF? Para XLS son 6 abajo (`{Down 6}`): PDF,HTML,MHT,RTF,DOCX,**XLS**.

**Paso C — confirmar opciones (si aparece):**
```ahk
    if WinWait("Opciones de Exportación", , 2) {
        Send "{Enter}"     ; botón Aceptar
    }
```

**Paso D — guardar con ruta (el diálogo se llama "Exportar como"):**
```ahk
    WinWait "Exportar como", , 10
    WinActivate "Exportar como"
    Sleep 200
    ControlSetText "C:\temp\LP0626652.pdf", "Edit1", "Exportar como"
    Send "{Enter}"
```
> Con Window Spy confirma 2 cosas en ESE diálogo: el **título** (¿"Exportar
> como"?) y el **control** del campo Nombre (¿"Edit1"? puede ser otro). Ajusta.

Cuando cada paso funcione solo, los juntas. Así depuras fácil.

## 8) Depurar

- ¿"No hace nada"? Mete `MsgBox("llegué aquí")` entre pasos para ver hasta
  dónde corre.
- ¿Título no coincide? Window Spy. Y arriba del script pon
  `SetTitleMatchMode 2` (coincide si el título **contiene** el texto).
- ¿El campo no recibe la ruta? El control no es `Edit1`. Míralo con Window Spy
  (Control under Mouse) y cámbialo.

## 9) Tu caso (resumen)

- Atajo: `^!e` con `#HotIf WinActive("Vista previa")`.
- Abrir menú "Exportar como": `Click` en su posición (Window Spy).
- PDF = `{Down 1}`, XLS = `{Down 6}` (verifica el orden real del menú).
- Opciones XLS: `WinWait "Opciones de Exportación"` → `Send "{Enter}"`.
- Guardar: el diálogo es **"Exportar como"** (no "Guardar como"); escribe la
  **ruta completa** en el campo Nombre y `Send "{Enter}"`.

Con esto puedes mantener tú mismo el script `ExportarPDFyXLS.ahk`.
Dudas puntuales: pregunta y te explico esa pieza.
