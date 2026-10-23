//! Arranque de la ventana en Windows.

mod util;
mod window;

use std::path::PathBuf;
use std::ptr;

use lince_core::Source;
use lince_gui::{parse_args, relaunch_params};
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
    AppendMenuW, CreateAcceleratorTableW, CreateMenu, CreatePopupMenu, CreateWindowExW,
    DispatchMessageW, GetMessageW, GetSystemMetrics, IsDialogMessageW, LoadCursorW, LoadIconW,
    LoadImageW, RegisterClassExW, ShowWindow, TranslateAcceleratorW, TranslateMessage, ACCEL,
    CW_USEDEFAULT, FCONTROL, FVIRTKEY, HICON, HMENU, IDC_ARROW, IMAGE_ICON, LR_DEFAULTCOLOR,
    MB_ICONERROR, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, SM_CXSMICON, SM_CYSMICON, SW_SHOWDEFAULT,
    WNDCLASSEXW, WS_CLIPCHILDREN, WS_EX_CONTROLPARENT, WS_OVERLAPPEDWINDOW,
};

use util::{is_elevated, message_box, relaunch_elevated, wide};
use window::{
    Config, CLASS_NAME, IDM_ADD_FOLDER, IDM_CLEAR_FOLDERS, IDM_EXIT, IDM_EXPORT, IDM_FOCUS_SEARCH,
    IDM_HELP, IDM_REFRESH,
};

/// Identificador del icono en `res/lince.rc`.
const ICON_ID: usize = 1;

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = parse_args(&args);

    // Como UltraSearch: se piden permisos de administrador para leer la MFT.
    // La copia elevada no ve las unidades de red que el usuario conectó sin
    // elevar (Windows las separa por sesión), así que se le pasan sus rutas
    // reales (\\servidor\recurso). Si el usuario niega el permiso, se sigue
    // sin él (recorrido de carpetas).
    if !args.explicit_sources && !args.relaunched && !is_elevated() {
        let network: Vec<PathBuf> = lince_core::remote_drives()
            .into_iter()
            .filter_map(|d| d.unc.map(PathBuf::from))
            .collect();
        if relaunch_elevated(&relaunch_params(&network)) {
            return;
        }
    }
    let (base_sources, folders_file) = if args.explicit_sources {
        (args.sources, None)
    } else {
        (base_sources(&args.extra), util::folders_file())
    };

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
        let cfg = Box::into_raw(Box::new(Config {
            base_sources,
            folders_file,
        }));
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
            main_menu(),
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

/// Volúmenes NTFS y unidades de red visibles, más las rutas de red que pasó
/// la copia sin elevar (sin repetir las que este proceso ya ve con letra).
fn base_sources(extra: &[PathBuf]) -> Vec<Source> {
    let mut sources = lince_core::default_sources();
    let seen: Vec<String> = lince_core::remote_drives()
        .into_iter()
        .filter_map(|d| d.unc.map(|u| u.to_lowercase()))
        .collect();
    for p in extra {
        let unc = p.to_string_lossy().to_lowercase();
        if !seen.contains(&unc) {
            sources.push(Source::Path(p.clone()));
        }
    }
    if sources.is_empty() {
        sources.push(Source::Path("C:\\".into()));
    }
    sources
}

/// Barra de menú: Archivo, Buscar en, Ayuda.
fn main_menu() -> HMENU {
    let item = |menu: HMENU, id: u16, text: &str| {
        let t = wide(text);
        // SAFETY: menú válido y texto terminado en 0 (AppendMenuW lo copia).
        unsafe {
            if id == 0 {
                AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
            } else {
                AppendMenuW(menu, MF_STRING, usize::from(id), t.as_ptr());
            }
        }
    };
    // SAFETY: menús nuevos; los submenús pasan a ser de la barra.
    unsafe {
        let bar = CreateMenu();
        let file = CreatePopupMenu();
        item(file, IDM_EXPORT, "&Exportar resultados a Excel…\tCtrl+E");
        item(file, 0, "");
        item(file, IDM_EXIT, "&Salir");
        let search_in = CreatePopupMenu();
        item(
            search_in,
            IDM_ADD_FOLDER,
            "&Agregar carpeta o unidad de red…",
        );
        item(search_in, IDM_CLEAR_FOLDERS, "&Quitar carpetas agregadas…");
        item(search_in, 0, "");
        item(search_in, IDM_REFRESH, "&Volver a leer los discos\tF5");
        let help = CreatePopupMenu();
        item(help, IDM_HELP, "&Cómo buscar…");
        for (sub, text) in [
            (file, "&Archivo"),
            (search_in, "&Buscar en"),
            (help, "A&yuda"),
        ] {
            let t = wide(text);
            AppendMenuW(bar, MF_POPUP, sub as usize, t.as_ptr());
        }
        bar
    }
}
