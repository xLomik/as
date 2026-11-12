//! Una sola copia de Lince en segundo plano: abrirlo otra vez muestra la
//! que ya está (con su índice cargado) en vez de volver a leer los discos.

use std::ptr;
use std::time::{Duration, Instant};

use windows_sys::core::{w, PCWSTR};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, ChangeWindowMessageFilterEx, FindWindowW, GetWindowThreadProcessId,
    PostMessageW, RegisterWindowMessageW, MSGFLT_ALLOW, WM_COPYDATA,
};

/// Clase de la ventana que vive en segundo plano (sin argumentos).
pub const MAIN_CLASS: PCWSTR = w!("LinceVentanaPrincipal");
/// Clase de las ventanas de una sola carpeta (clic derecho, `--path`...):
/// se cierran del todo y no cuentan como la copia en segundo plano.
pub const FOLDER_CLASS: PCWSTR = w!("LinceVentanaCarpeta");

/// «Muéstrate»: lo manda otra copia de Lince al abrirse.
pub fn show_message() -> u32 {
    // SAFETY: texto constante terminado en 0.
    unsafe { RegisterWindowMessageW(w!("Lince.Mostrar")) }
}

/// «Ciérrate del todo» (`Lince.exe --salir`).
pub fn quit_message() -> u32 {
    // SAFETY: texto constante terminado en 0.
    unsafe { RegisterWindowMessageW(w!("Lince.Salir")) }
}

/// La copia en segundo plano, si hay una.
pub fn running() -> Option<HWND> {
    // SAFETY: clase constante; sin título.
    let hwnd = unsafe { FindWindowW(MAIN_CLASS, ptr::null()) };
    (!hwnd.is_null()).then_some(hwnd)
}

/// Le pide que se muestre y la deja pasar al frente (Windows solo deja
/// hacerlo a quien el usuario acaba de abrir).
pub fn show(hwnd: HWND) {
    // SAFETY: ventana de otro proceso; las llamadas solo leen su id o
    // dejan un mensaje en su cola.
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        AllowSetForegroundWindow(pid);
        PostMessageW(hwnd, show_message(), 0, 0);
    }
}

pub fn quit(hwnd: HWND) {
    // SAFETY: solo deja un mensaje en la cola de esa ventana.
    unsafe { PostMessageW(hwnd, quit_message(), 0, 0) };
}

/// La copia elevada (administrador) acepta estos mensajes de una copia sin
/// elevar y de la caja de la barra de tareas (que vive en el Explorador);
/// si no, Windows los bloquea.
pub fn accept_messages(hwnd: HWND) {
    for m in [show_message(), quit_message(), WM_COPYDATA] {
        // SAFETY: ventana propia y mensaje registrado.
        unsafe { ChangeWindowMessageFilterEx(hwnd, m, MSGFLT_ALLOW, ptr::null_mut()) };
    }
}

/// ¿La copia abierta tiene permisos de administrador? Si no se puede saber
/// (justo pasa cuando ella los tiene y esta no), se supone que sí.
pub fn is_elevated_copy(hwnd: HWND) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: proceso y token se cierran; el tamaño es el del dato.
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return true;
        }
        let mut token = ptr::null_mut();
        let opened = OpenProcessToken(process, TOKEN_QUERY, &mut token) != 0;
        CloseHandle(process);
        if !opened {
            return true;
        }
        let mut info = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            (&mut info as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        ) != 0;
        CloseHandle(token);
        !ok || info.TokenIsElevated != 0
    }
}

/// Espera (como mucho `limit`) a que la copia abierta termine.
pub fn wait_gone(limit: Duration) {
    let end = Instant::now() + limit;
    while running().is_some() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(100));
    }
}
