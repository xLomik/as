//! Arranque de la ventana en Windows.

mod util;
mod window;

use std::ptr;

use lince_core::Source;
use lince_gui::{parse_args, RELAUNCH_FLAG};
use windows_sys::Win32::Graphics::Gdi::{UpdateWindow, COLOR_BTNFACE, HBRUSH};
use windows_sys::Win32::System::Com::{
    CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_BAR_CLASSES, ICC_LISTVIEW_CLASSES, ICC_STANDARD_CLASSES,
    INITCOMMONCONTROLSEX,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_F5;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateAcceleratorTableW, CreateWindowExW, DispatchMessageW, GetMessageW, GetSystemMetrics,
    IsDialogMessageW, LoadCursorW, LoadIconW, LoadImageW, RegisterClassExW, ShowWindow,
    TranslateAcceleratorW, TranslateMessage, ACCEL, CW_USEDEFAULT, FCONTROL, FVIRTKEY, HICON,
    IDC_ARROW, IMAGE_ICON, LR_DEFAULTCOLOR, MB_ICONERROR, MSG, SM_CXSMICON, SM_CYSMICON,
    SW_SHOWDEFAULT, WNDCLASSEXW, WS_CLIPCHILDREN, WS_EX_CONTROLPARENT, WS_OVERLAPPEDWINDOW,
};

use util::{is_elevated, message_box, relaunch_elevated, wide};
use window::{Config, CLASS_NAME, IDM_EXPORT, IDM_FOCUS_SEARCH, IDM_REFRESH};

/// Identificador del icono en `res/lince.rc`.
const ICON_ID: usize = 1;

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = parse_args(&args);

    // Como UltraSearch: se piden permisos de administrador para leer la MFT.
    // Si el usuario los niega, sigue sin ellos (recorrido de carpetas).
    if !args.explicit_sources
        && !args.relaunched
        && !is_elevated()
        && relaunch_elevated(RELAUNCH_FLAG)
    {
        return;
    }
    let mut sources = args.sources;
    if sources.is_empty() {
        sources = lince_core::ntfs_volumes()
            .into_iter()
            .map(Source::Volume)
            .collect();
    }
    if sources.is_empty() {
        sources.push(Source::Path("C:\\".into()));
    }

    // SAFETY: llamadas de inicialización con estructuras válidas; el bucle de
    // mensajes usa un MSG propio.
    unsafe {
        CoInitializeEx(
            ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        );
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_BAR_CLASSES | ICC_STANDARD_CLASSES,
        };
        InitCommonControlsEx(&icc);

        let hinst = GetModuleHandleW(ptr::null());
        let icon = LoadIconW(hinst, ICON_ID as *const u16);
        let small = LoadImageW(
            hinst,
            ICON_ID as *const u16,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        ) as HICON;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window::wndproc),
            hInstance: hinst,
            hIcon: icon,
            hIconSm: small,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as usize as HBRUSH,
            lpszClassName: CLASS_NAME,
            ..std::mem::zeroed()
        };
        RegisterClassExW(&wc);

        let title = wide("Lince — buscador de archivos");
        let cfg = Box::into_raw(Box::new(Config { sources }));
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            CLASS_NAME,
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1150,
            720,
            ptr::null_mut(),
            ptr::null_mut(),
            hinst,
            cfg.cast(),
        );
        if hwnd.is_null() {
            message_box(
                ptr::null_mut(),
                "No se pudo crear la ventana de Lince.",
                MB_ICONERROR,
            );
            return;
        }
        ShowWindow(hwnd, SW_SHOWDEFAULT);
        UpdateWindow(hwnd);

        let ctrl = FVIRTKEY | FCONTROL;
        let accels = [
            ACCEL {
                fVirt: FVIRTKEY,
                key: VK_F5,
                cmd: IDM_REFRESH,
            },
            ACCEL {
                fVirt: ctrl,
                key: u16::from(b'F'),
                cmd: IDM_FOCUS_SEARCH,
            },
            ACCEL {
                fVirt: ctrl,
                key: u16::from(b'L'),
                cmd: IDM_FOCUS_SEARCH,
            },
            ACCEL {
                fVirt: ctrl,
                key: u16::from(b'E'),
                cmd: IDM_EXPORT,
            },
        ];
        let haccel = CreateAcceleratorTableW(accels.as_ptr(), accels.len() as i32);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            if TranslateAcceleratorW(hwnd, haccel, &msg) != 0 {
                continue;
            }
            // Tab entre controles, Enter y Esc.
            if IsDialogMessageW(hwnd, &msg) != 0 {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
