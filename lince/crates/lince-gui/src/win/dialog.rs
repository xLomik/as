//! Diálogo para escribir (o buscar) la ruta de una carpeta, incluida una de
//! red como `\\servidor\carpeta`. Se hace a mano porque el diálogo clásico
//! de carpetas no garantiza respetar una ruta de red escrita.

use std::cell::{Cell, RefCell};
use std::ptr;

use windows_sys::core::w;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::HFONT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::EM_SETCUEBANNER;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, GetWindowRect, IsDialogMessageW, PostQuitMessage, RegisterClassExW,
    SendMessageW, SetForegroundWindow, SetWindowLongPtrW, SetWindowTextW, ShowWindow,
    TranslateMessage, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, ES_AUTOHSCROLL, GWLP_USERDATA, HMENU,
    IDCANCEL, IDOK, MSG, SW_SHOW, WM_CLOSE, WM_COMMAND, WM_GETFONT, WM_QUIT, WM_SETFONT,
    WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME,
    WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

use super::util::{pick_folder, wide, window_text};

const CLASS: windows_sys::core::PCWSTR = w!("LinceDialogoRuta");
const ID_BROWSE: u16 = 3;

struct State {
    edit: HWND,
    result: RefCell<Option<String>>,
    done: Cell<bool>,
}

/// Pide una ruta. `None` si el usuario cancela.
pub fn ask_path(parent: HWND, title: &str, prompt: &str) -> Option<String> {
    // SAFETY: ventana modal propia; el estado vive en esta función mientras
    // dura el bucle de mensajes y se desengancha antes de destruir la ventana.
    unsafe {
        let hinst = GetModuleHandleW(ptr::null());
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc_),
            hInstance: hinst,
            hbrBackground: (windows_sys::Win32::Graphics::Gdi::COLOR_BTNFACE + 1) as usize
                as windows_sys::Win32::Graphics::Gdi::HBRUSH,
            lpszClassName: CLASS,
            ..std::mem::zeroed()
        };
        RegisterClassExW(&wc); // si ya existe, falla sin importar

        let dpi = GetDpiForWindow(parent).max(96) as i32;
        let s = |v: i32| v * dpi / 96;
        let (w, h) = (s(560), s(170));
        let mut pr: RECT = std::mem::zeroed();
        GetWindowRect(parent, &mut pr);
        let x = pr.left + ((pr.right - pr.left) - w) / 2;
        let y = pr.top + s(80);
        let title = wide(title);
        let dlg = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            CLASS,
            title.as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            w,
            h,
            parent,
            ptr::null_mut(),
            hinst,
            ptr::null(),
        );
        if dlg.is_null() {
            return None;
        }
        let font = SendMessageW(parent, WM_GETFONT, 0, 0) as HFONT;
        let child = |class: windows_sys::core::PCWSTR,
                     text: &str,
                     style: u32,
                     ex: u32,
                     id: u16,
                     rect: (i32, i32, i32, i32)| {
            let t = wide(text);
            let hw = CreateWindowExW(
                ex,
                class,
                t.as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                rect.0,
                rect.1,
                rect.2,
                rect.3,
                dlg,
                id as usize as HMENU,
                hinst,
                ptr::null(),
            );
            SendMessageW(hw, WM_SETFONT, font as usize, 1);
            hw
        };
        let pad = s(12);
        let inner = w - 2 * pad - s(16);
        child(w!("STATIC"), prompt, 0, 0, 0, (pad, pad, inner, s(36)));
        let edit = child(
            w!("EDIT"),
            "",
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            WS_EX_CLIENTEDGE,
            100,
            (pad, pad + s(40), inner - s(110), s(26)),
        );
        let cue = wide(r"\\servidor\carpeta   o   Z:\Proyectos");
        SendMessageW(edit, EM_SETCUEBANNER, 1, cue.as_ptr() as isize);
        child(
            w!("BUTTON"),
            "Examinar…",
            WS_TABSTOP | BS_PUSHBUTTON as u32,
            0,
            ID_BROWSE,
            (pad + inner - s(100), pad + s(40), s(100), s(26)),
        );
        let by = pad + s(80);
        child(
            w!("BUTTON"),
            "Agregar",
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            0,
            IDOK as u16,
            (pad + inner - s(210), by, s(100), s(28)),
        );
        child(
            w!("BUTTON"),
            "Cancelar",
            WS_TABSTOP | BS_PUSHBUTTON as u32,
            0,
            IDCANCEL as u16,
            (pad + inner - s(100), by, s(100), s(28)),
        );

        let state = State {
            edit,
            result: RefCell::new(None),
            done: Cell::new(false),
        };
        SetWindowLongPtrW(dlg, GWLP_USERDATA, &state as *const State as isize);
        EnableWindow(parent, 0);
        ShowWindow(dlg, SW_SHOW);
        SetFocus(edit);

        let mut msg: MSG = std::mem::zeroed();
        while !state.done.get() {
            if GetMessageW(&mut msg, ptr::null_mut(), 0, 0) <= 0 {
                // Llegó WM_QUIT: se devuelve a la cola para el bucle principal.
                if msg.message == WM_QUIT {
                    PostQuitMessage(msg.wParam as i32);
                }
                break;
            }
            if IsDialogMessageW(dlg, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        EnableWindow(parent, 1);
        SetWindowLongPtrW(dlg, GWLP_USERDATA, 0);
        DestroyWindow(dlg);
        SetForegroundWindow(parent);
        let result = state.result.borrow_mut().take();
        result
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
    }
}

unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
    if state.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let state = &*state;
    match msg {
        WM_COMMAND => {
            match (wp & 0xFFFF) as i32 {
                IDOK => {
                    let text = window_text(state.edit);
                    if !text.trim().is_empty() {
                        *state.result.borrow_mut() = Some(text);
                        state.done.set(true);
                    }
                }
                IDCANCEL => state.done.set(true),
                id if id == i32::from(ID_BROWSE) => {
                    if let Some(p) = pick_folder(hwnd, "Elija la carpeta donde buscar") {
                        let t = wide(&p.to_string_lossy());
                        SetWindowTextW(state.edit, t.as_ptr());
                    }
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            state.done.set(true);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
