//! Envoltorios pequeños de la API de Windows.

use std::path::PathBuf;
use std::ptr;

use windows_sys::core::w;
use windows_sys::Win32::Foundation::{GlobalFree, HWND};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::UI::Controls::Dialogs::{
    GetSaveFileNameW, OFN_EXPLORER, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows_sys::Win32::UI::Shell::{
    IsUserAnAdmin, SHBrowseForFolderW, SHGetPathFromIDListW, ShellExecuteW, BIF_EDITBOX,
    BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowTextLengthW, GetWindowTextW, MessageBoxW, IDYES, MB_ICONQUESTION, MB_YESNO,
    MESSAGEBOX_STYLE, SW_SHOWNORMAL,
};

/// Texto terminado en 0 para la API de Windows.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn window_text(hwnd: HWND) -> String {
    // SAFETY: el buffer tiene el tamaño que se le indica a GetWindowTextW.
    unsafe {
        let len = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut buf = vec![0u16; len + 1];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buf[..n])
    }
}

pub fn message_box(hwnd: HWND, text: &str, style: MESSAGEBOX_STYLE) {
    // SAFETY: textos terminados en 0 que viven durante la llamada.
    unsafe {
        MessageBoxW(hwnd, wide(text).as_ptr(), w!("Lince"), style);
    }
}

/// Pregunta sí/no; `true` si el usuario eligió «Sí».
pub fn ask_yes_no(hwnd: HWND, text: &str) -> bool {
    // SAFETY: textos terminados en 0 que viven durante la llamada.
    unsafe {
        MessageBoxW(
            hwnd,
            wide(text).as_ptr(),
            w!("Lince"),
            MB_YESNO | MB_ICONQUESTION,
        ) == IDYES
    }
}

/// Diálogo para elegir una carpeta. Tiene un campo de texto donde también se
/// puede escribir una ruta de red (`\\servidor\carpeta`).
pub fn pick_folder(hwnd: HWND, title: &str) -> Option<PathBuf> {
    let title = wide(title);
    let mut path = vec![0u16; 32_768];
    // SAFETY: estructura inicializada a cero; el PIDL devuelto se libera con
    // CoTaskMemFree y `path` tiene el tamaño máximo de una ruta.
    unsafe {
        let mut bi: BROWSEINFOW = std::mem::zeroed();
        bi.hwndOwner = hwnd;
        bi.lpszTitle = title.as_ptr();
        bi.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX;
        let pidl = SHBrowseForFolderW(&bi);
        if pidl.is_null() {
            return None;
        }
        let ok = SHGetPathFromIDListW(pidl, path.as_mut_ptr());
        CoTaskMemFree(pidl as *const _);
        if ok == 0 {
            return None;
        }
    }
    let len = path.iter().position(|&c| c == 0).unwrap_or(0);
    (len > 0).then(|| PathBuf::from(String::from_utf16_lossy(&path[..len])))
}

/// Archivo con las carpetas agregadas: `%APPDATA%\Lince\carpetas.txt`.
pub fn folders_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Lince").join("carpetas.txt"))
}

fn shell_execute(hwnd: HWND, verb: &str, file: &str, params: Option<&str>) -> bool {
    let (verb, file) = (wide(verb), wide(file));
    let params = params.map(wide);
    // SAFETY: textos terminados en 0; ShellExecuteW devuelve > 32 si tuvo éxito.
    let r = unsafe {
        ShellExecuteW(
            hwnd,
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ref().map_or(ptr::null(), |p| p.as_ptr()),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    r as isize > 32
}

/// Abre el archivo o la carpeta con su programa asociado.
pub fn open_path(hwnd: HWND, path: &str) -> bool {
    shell_execute(hwnd, "open", path, None)
}

/// Abre el Explorador en la carpeta del archivo, con el archivo seleccionado.
pub fn show_in_folder(hwnd: HWND, path: &str) -> bool {
    shell_execute(
        hwnd,
        "open",
        "explorer.exe",
        Some(&format!("/select,\"{path}\"")),
    )
}

pub fn set_clipboard(hwnd: HWND, text: &str) -> bool {
    let data = wide(text);
    let bytes = data.len() * 2;
    // SAFETY: el bloque global tiene `bytes` bytes; si SetClipboardData lo
    // acepta, pasa a ser del sistema y no se libera aquí.
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            return false;
        }
        EmptyClipboard();
        let mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        let mut ok = false;
        if !mem.is_null() {
            let dst = GlobalLock(mem) as *mut u16;
            if !dst.is_null() {
                ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
                GlobalUnlock(mem);
                ok = !SetClipboardData(u32::from(CF_UNICODETEXT), mem).is_null();
            }
            if !ok {
                GlobalFree(mem);
            }
        }
        CloseClipboard();
        ok
    }
}

/// Diálogo «Guardar como» para un CSV.
pub fn save_csv_dialog(hwnd: HWND, default_name: &str) -> Option<PathBuf> {
    let mut file = vec![0u16; 1024];
    let name = wide(default_name);
    file[..name.len()].copy_from_slice(&name);
    let filter: Vec<u16> = "CSV para Excel (*.csv)\0*.csv\0Todos los archivos\0*.*\0\0"
        .encode_utf16()
        .collect();
    // SAFETY: estructura inicializada a cero con su tamaño y buffers válidos
    // durante la llamada modal.
    unsafe {
        let mut ofn: OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = hwnd;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = file.as_mut_ptr();
        ofn.nMaxFile = file.len() as u32;
        ofn.lpstrDefExt = w!("csv");
        ofn.Flags = OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST;
        if GetSaveFileNameW(&mut ofn) == 0 {
            return None;
        }
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    Some(PathBuf::from(String::from_utf16_lossy(&file[..len])))
}

pub fn is_elevated() -> bool {
    // SAFETY: sin argumentos.
    unsafe { IsUserAnAdmin() != 0 }
}

/// Vuelve a abrir Lince pidiendo permisos de administrador (UAC). Devuelve
/// `false` si el usuario lo rechazó o falló.
pub fn relaunch_elevated(params: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    shell_execute(
        ptr::null_mut(),
        "runas",
        &exe.to_string_lossy(),
        Some(params),
    )
}
