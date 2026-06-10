#Requires AutoHotkey v2.0
#SingleInstance Force

; ============================================================
;  CALIBRADOR — encuentra la posición del botón "Exportar como"
;
;  Uso:
;   1. Abre BySoft con la "Vista previa" (maximizada).
;   2. Ejecuta este archivo (doble clic).
;   3. Pon el mouse ENCIMA del botón "Exportar como" (NO hagas clic).
;   4. Pulsa  F8.
;   5. Te muestra X,Y (relativas a la ventana) y las copia al portapapeles.
;   6. Pega esos valores en ExportarPDFyXLS.ahk:
;          EXPORT_BTN_X := <X>
;          EXPORT_BTN_Y := <Y>
; ============================================================

*F8:: {
    CoordMode "Mouse", "Window"
    MouseGetPos &x, &y
    A_Clipboard := "EXPORT_BTN_X := " . x . "`nEXPORT_BTN_Y := " . y
    MsgBox "Posición del botón (relativa a la ventana):`n`n"
         . "    EXPORT_BTN_X := " . x . "`n"
         . "    EXPORT_BTN_Y := " . y . "`n`n"
         . "(Ya copiado al portapapeles. Pégalo en ExportarPDFyXLS.ahk)",
           "Calibrador", "Iconi"
}

*Esc::ExitApp
