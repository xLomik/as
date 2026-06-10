#Requires AutoHotkey v2.0
#SingleInstance Force
SetTitleMatchMode 2        ; coincide por "contiene" (títulos parciales)
SetKeyDelay 30

; ============================================================
;  Exportar PDF + XLS de BySoft con UN solo atajo
;  Atajo por defecto: Ctrl+Alt+E  (con la "Vista previa" abierta)
;
;  Qué hace:
;   1. Te pide el nombre del trabajo UNA vez (ej. LP0626652)
;   2. Exporta a PDF  -> tú eliges la carpeta y pulsas Guardar (1 vez)
;   3. Exporta a XLS  -> se guarda SOLO en esa misma carpeta
;
;  >>> ANTES DE USAR: calibra la posición del botón "Exportar como"
;      con Calibrar.ahk (ver README) y pega los valores aquí abajo. <<<
; ============================================================

; ===================== CONFIGURACIÓN ========================
PREVIEW_TITLE  := "Vista previa"          ; título de la ventana de BySoft
EXPORT_BTN_X   := 1130                     ; X del botón "Exportar como" (RELATIVA a la ventana)
EXPORT_BTN_Y   := 70                       ; Y del botón "Exportar como"
PDF_DOWN       := 1                         ; flechas ABAJO hasta "Documento PDF" (1er ítem)
XLS_DOWN       := 6                         ; flechas ABAJO hasta "Documento XLS" (6º ítem)
OPCIONES_TITLE := "Opciones de Exportación" ; diálogo de opciones (Aceptar)
GUARDAR_TITLE  := "Guardar como"           ; diálogo de guardado de Windows
; ============================================================

; El atajo SOLO funciona cuando la Vista previa está activa.
#HotIf WinActive(PREVIEW_TITLE)
^!e::DobleExport()
#HotIf


DobleExport() {
    global

    ib := InputBox("Escribe el nombre del trabajo (ej. LP0626652):",
                   "Exportar PDF + XLS", "w360 h130")
    if (ib.Result != "OK" || Trim(ib.Value) = "")
        return
    nombre := Trim(ib.Value)

    ; ---------- 1) PDF (eliges carpeta + Guardar) ----------
    if !AbrirFormato(PDF_DOWN)
        return
    PulsarOpcionesSiAparece()
    if !WinWait(GUARDAR_TITLE, , 12) {
        MsgBox "No apareció 'Guardar como' para el PDF.`n`nRevisa EXPORT_BTN_X/Y y PDF_DOWN en la configuración.",
               "Exportar PDF + XLS", "Iconx"
        return
    }
    WinActivate(GUARDAR_TITLE)
    PonerNombre(nombre . ".pdf")
    A_Clipboard := nombre           ; por si necesitas pegarlo
    ToolTip("PASO 1/2  ->  navega a la carpeta del trabajo y pulsa GUARDAR`n(el nombre ya está puesto)")
    WinWaitClose(GUARDAR_TITLE)     ; espera a que TÚ guardes el PDF
    ToolTip()

    ; ---------- 2) XLS (automático, misma carpeta) ----------
    if !AbrirFormato(XLS_DOWN)
        return
    PulsarOpcionesSiAparece()
    if !WinWait(GUARDAR_TITLE, , 12) {
        MsgBox "No apareció 'Guardar como' para el XLS.`n`nRevisa XLS_DOWN en la configuración.",
               "Exportar PDF + XLS", "Iconx"
        return
    }
    WinActivate(GUARDAR_TITLE)
    PonerNombre(nombre . ".xls")
    Sleep 200
    Send "{Enter}"                  ; guarda solo, en la misma carpeta del PDF
    ToolTip("Listo:  " . nombre . ".pdf  +  " . nombre . ".xls")
    SetTimer(() => ToolTip(), -2800)
}


; Abre el menú "Exportar como" y elige el formato por índice (flechas abajo).
AbrirFormato(down) {
    global PREVIEW_TITLE, EXPORT_BTN_X, EXPORT_BTN_Y
    if !WinActive(PREVIEW_TITLE)
        WinActivate(PREVIEW_TITLE)
    Sleep 120
    CoordMode "Mouse", "Window"
    Click EXPORT_BTN_X, EXPORT_BTN_Y    ; despliega "Exportar como"
    Sleep 300
    if (down > 0)
        Send "{Down " . down . "}"
    Sleep 150
    Send "{Enter}"
    return true
}


; Si aparece "Opciones de Exportación XLS/PDF", pulsa Aceptar (Enter).
PulsarOpcionesSiAparece() {
    global OPCIONES_TITLE
    if WinWait(OPCIONES_TITLE, , 2) {
        WinActivate(OPCIONES_TITLE)
        Sleep 150
        Send "{Enter}"                  ; Aceptar
        WinWaitClose(OPCIONES_TITLE, , 5)
    }
}


; Escribe el nombre de archivo en el diálogo "Guardar como".
PonerNombre(archivo) {
    global GUARDAR_TITLE
    try {
        ControlSetText archivo, "Edit1", GUARDAR_TITLE
    } catch {
        ; Respaldo: enfocar el campo y escribir
        try ControlFocus "Edit1", GUARDAR_TITLE
        Send "^a"
        SendText archivo
    }
}
