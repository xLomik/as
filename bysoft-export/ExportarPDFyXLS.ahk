#Requires AutoHotkey v2.0
#SingleInstance Force
SetTitleMatchMode 2        ; coincide por "contiene" (títulos parciales)
SetKeyDelay 30

; ============================================================
;  Exportar PDF + XLS de BySoft con UN solo atajo
;  Atajo por defecto: Ctrl+Alt+E  (con la "Vista previa" abierta)
;
;  Flujo:
;   1. Pide el nombre del trabajo UNA vez (ej. LP0626652)
;   2. Abre un SELECTOR DE CARPETA -> eliges la carpeta de la solicitud
;      (recuerda la ultima usada)
;   3. Exporta a PDF y a XLS, guardando AMBOS en esa carpeta, con ese
;      nombre, SIN que navegues los dialogos de "Guardar como".
;
;  >>> ANTES DE USAR: calibra la posicion del boton "Exportar como"
;      con Calibrar.ahk (ver README) y pega los valores aqui abajo. <<<
; ============================================================

; ===================== CONFIGURACION ========================
PREVIEW_TITLE  := "Vista previa"          ; titulo de la ventana de BySoft
EXPORT_BTN_X   := 1130                     ; X del boton "Exportar como" (RELATIVA a la ventana)
EXPORT_BTN_Y   := 70                       ; Y del boton "Exportar como"
PDF_DOWN       := 1                         ; flechas ABAJO hasta "Documento PDF" (1er item)
XLS_DOWN       := 6                         ; flechas ABAJO hasta "Documento XLS" (6to item)
OPCIONES_TITLE := "Opciones de Exportación" ; dialogo de opciones (Aceptar)
GUARDAR_TITLE  := "Exportar como"          ; OJO: el dialogo de guardado se llama "Exportar como" (no "Guardar como")
INI            := A_ScriptDir . "\pdf2xls.ini"   ; recuerda la ultima carpeta
; ============================================================

; El atajo SOLO funciona cuando la Vista previa esta activa.
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

    ; Selector de carpeta (arranca en la ultima usada)
    ultima := IniRead(INI, "cfg", "ultimaCarpeta", "")
    carpeta := DirSelect(ultima, 1, "Elige la carpeta donde guardar " . nombre . ".pdf y .xls")
    if (carpeta = "")
        return
    carpeta := RTrim(carpeta, "\")
    IniWrite(carpeta, INI, "cfg", "ultimaCarpeta")

    rutaPDF := carpeta . "\" . nombre . ".pdf"
    rutaXLS := carpeta . "\" . nombre . ".xls"

    ; ---------- PDF ----------
    if !AbrirFormato(PDF_DOWN)
        return
    PulsarOpcionesSiAparece()
    if !GuardarComo(rutaPDF) {
        MsgBox "No se pudo guardar el PDF.`nRevisa EXPORT_BTN_X/Y y PDF_DOWN.",
               "Exportar PDF + XLS", "Iconx"
        return
    }

    ; ---------- XLS ----------
    if !AbrirFormato(XLS_DOWN)
        return
    PulsarOpcionesSiAparece()
    if !GuardarComo(rutaXLS) {
        MsgBox "No se pudo guardar el XLS.`nRevisa XLS_DOWN.",
               "Exportar PDF + XLS", "Iconx"
        return
    }

    ToolTip("Listo:`n" rutaPDF "`n" rutaXLS)
    SetTimer(() => ToolTip(), -3500)
}


; Abre el menu "Exportar como" y elige el formato por indice (flechas abajo).
AbrirFormato(down) {
    global PREVIEW_TITLE, EXPORT_BTN_X, EXPORT_BTN_Y
    if !WinActive(PREVIEW_TITLE)
        WinActivate(PREVIEW_TITLE)
    Sleep 120
    CoordMode "Mouse", "Window"
    Click EXPORT_BTN_X " " EXPORT_BTN_Y   ; despliega "Exportar como"
    Sleep 300
    if (down > 0)
        Send "{Down " down "}"
    Sleep 150
    Send "{Enter}"
    return true
}


; Si aparece "Opciones de Exportacion XLS/PDF", pulsa Aceptar (Enter).
PulsarOpcionesSiAparece() {
    global OPCIONES_TITLE
    if WinWait(OPCIONES_TITLE, , 2) {
        WinActivate(OPCIONES_TITLE)
        Sleep 150
        Send "{Enter}"                  ; Aceptar
        WinWaitClose(OPCIONES_TITLE, , 5)
    }
}


; Escribe la RUTA COMPLETA en "Guardar como" y guarda (sin navegar a mano).
GuardarComo(rutaCompleta) {
    global GUARDAR_TITLE
    if !WinWait(GUARDAR_TITLE, , 12)
        return false
    WinActivate(GUARDAR_TITLE)
    Sleep 200
    ; El campo "Nombre" de Windows acepta una ruta absoluta y guarda alli.
    try {
        ControlSetText rutaCompleta, "Edit1", GUARDAR_TITLE
    } catch {
        ControlFocus "Edit1", GUARDAR_TITLE
        Send "^a"
        SendText rutaCompleta
    }
    Sleep 200
    Send "{Enter}"
    ; Confirmacion de sobrescritura, si el archivo ya existe.
    Sleep 350
    if (WinExist("Confirmar Guardar como") || WinExist("Confirm Save As"))
        Send "{Enter}"
    WinWaitClose(GUARDAR_TITLE, , 8)
    return true
}
