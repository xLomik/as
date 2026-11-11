//! Icono de Lince en el área de notificación (junto al reloj).

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_INFO, NIM_ADD,
    NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{HICON, WM_APP};

/// Mensaje que manda el icono a la ventana.
pub const WM_TRAY: u32 = WM_APP + 2;
/// Clic (o Enter con el teclado) sobre el icono.
pub const NIN_KEYSELECT: u32 = NIN_SELECT | 1;

const ID: u32 = 1;

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ID,
        ..Default::default()
    }
}

/// Copia `s` en un búfer de texto de ancho fijo, recortando si no cabe.
fn copy_into(dst: &mut [u16], s: &str) {
    let max = dst.len() - 1;
    let mut n = 0;
    for (d, c) in dst.iter_mut().take(max).zip(s.encode_utf16()) {
        *d = c;
        n += 1;
    }
    dst[n] = 0;
}

/// Pone el icono. `false` si no se pudo (p. ej. el Explorador no está).
pub fn add(hwnd: HWND, icon: HICON, tip: &str) -> bool {
    let mut d = data(hwnd);
    d.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
    d.uCallbackMessage = WM_TRAY;
    d.hIcon = icon;
    copy_into(&mut d.szTip, tip);
    // SAFETY: estructura completa con su tamaño.
    unsafe {
        if Shell_NotifyIconW(NIM_ADD, &d) == 0 {
            return false;
        }
        // Versión 4: clic = NIN_SELECT, menú = WM_CONTEXTMENU.
        d.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        Shell_NotifyIconW(NIM_SETVERSION, &d);
    }
    true
}

/// Texto que aparece al pasar el ratón.
pub fn set_tip(hwnd: HWND, tip: &str) {
    let mut d = data(hwnd);
    d.uFlags = NIF_TIP | NIF_SHOWTIP;
    copy_into(&mut d.szTip, tip);
    // SAFETY: estructura completa con su tamaño.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
}

/// Aviso que sale junto al icono.
pub fn notify(hwnd: HWND, title: &str, text: &str) {
    let mut d = data(hwnd);
    d.uFlags = NIF_INFO;
    d.dwInfoFlags = NIIF_INFO;
    copy_into(&mut d.szInfoTitle, title);
    copy_into(&mut d.szInfo, text);
    // SAFETY: estructura completa con su tamaño.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
}

pub fn remove(hwnd: HWND) {
    let d = data(hwnd);
    // SAFETY: estructura completa con su tamaño.
    unsafe { Shell_NotifyIconW(NIM_DELETE, &d) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_texts_are_cut_and_terminated() {
        let mut buf = [1u16; 5];
        copy_into(&mut buf, "Lince buscador");
        assert_eq!(String::from_utf16_lossy(&buf[..4]), "Linc");
        assert_eq!(buf[4], 0);
        copy_into(&mut buf, "ñ");
        assert_eq!(&buf[..2], &[u16::from(b'\xF1'), 0]);
    }
}
