#Requires AutoHotkey v2.0
#SingleInstance Force
SetTitleMatchMode 2

; ============================================================
;  DIAGNOSTICO — encuentra en qué paso falla la exportación.
;  Úsalo con la "Vista previa" de BySoft ABIERTA y al frente.
;
;  PASO 1: pon el mouse sobre el botón "Exportar como" y pulsa F8.
;          Copia las coords y pégalas abajo en EX / EY. Recarga.
;  PASO 2: pulsa Ctrl+Alt+1  -> ¿se abre el menú "Exportar como"?
;  PASO 3: pulsa Ctrl+Alt+2  -> ¿elige PDF y aparece el diálogo?
;  Esc para salir.
; ============================================================

; >>> Pega aquí las coords que te dé F8 <<<
EX := 1130
EY := 70

PREVIEW := "Vista previa"


; ---- F8: capturar la posición del botón "Exportar como" ----
*F8:: {
    CoordMode "Mouse", "Window"
    MouseGetPos &x, &y
    A_Clipboard := "EX := " . x . "`nEY := " . y
    MsgBox("Coords (relativas a la ventana): " . x . ", " . y
         . "`n`nCopiado al portapapeles. Pega EX/EY arriba y recarga.",
           "Calibrar", "Iconi")
}


; ---- Ctrl+Alt+1: TEST 1 -> solo abrir el menú ----
^!1:: {
    global EX, EY, PREVIEW
    CoordMode "Mouse", "Window"
    WinActivate PREVIEW
    Sleep 150
    Click EX . " " . EY
    ToolTip("TEST 1: cliqué en " . EX . "," . EY
          . "`n¿Se abrió el menú 'Exportar como'? (PDF, HTML, ... XLS)")
    SetTimer(() => ToolTip(), -4000)
}


; ---- Ctrl+Alt+2: TEST 2 -> abrir menú + elegir 1er formato (PDF) ----
^!2:: {
    global EX, EY, PREVIEW
    CoordMode "Mouse", "Window"
    WinActivate PREVIEW
    Sleep 150
    Click EX . " " . EY
    Sleep 500
    Send "{Down 1}"        ; 1er ítem del menú = "Documento PDF"
    Sleep 150
    Send "{Enter}"
    ToolTip("TEST 2: intenté elegir el 1er formato (PDF)."
          . "`n¿Apareció 'Opciones de Exportación' o el diálogo de guardar?")
    SetTimer(() => ToolTip(), -4000)
}


Esc::ExitApp
