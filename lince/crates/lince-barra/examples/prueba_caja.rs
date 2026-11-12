//! Banco de pruebas de la caja sin el Explorador: carga la DLL como lo hace
//! la barra de tareas (DllGetClassObject → IDeskBand), le da una ventana
//! «barra» falsa como sitio, escribe en la caja y comprueba que lo escrito,
//! las teclas y el foco llegan a una ventana «Lince» falsa.
//!
//! Uso: `prueba_caja <ruta de lince_barra.dll> [--lince]`. Con `--lince`
//! habla con el Lince.exe de verdad (que debe estar abierto junto al reloj):
//! escribe «notepad» y comprueba que los resultados aparecen encima de la
//! caja, que la flecha mueve la elección y que Esc los esconde.
//! Sale con 0 si todo va bien.

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    if let Err(e) = windows_main::run() {
        eprintln!("FALLO: {e}");
        std::process::exit(1);
    }
    if std::env::args().any(|a| a == "--lince") {
        println!("OK: la caja y Lince.exe: resultados encima, flecha y Esc");
    } else {
        println!("OK: la caja carga, se dibuja y habla con Lince");
    }
}

#[cfg(windows)]
#[allow(non_snake_case)]
mod windows_main {
    use std::cell::RefCell;
    use std::ffi::c_void;

    use lince_protocolo::{payload_text, Command, Key, BAND_CLASS, BAND_CLSID_U128, MAIN_CLASS};
    use windows::core::{implement, w, Interface, Ref, BOOL, GUID, HRESULT, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Com::{CoInitializeEx, IClassFactory, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::DataExchange::COPYDATASTRUCT;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
    use windows::Win32::System::Ole::{IObjectWithSite, IOleWindow, IOleWindow_Impl};
    use windows::Win32::UI::Controls::{LVM_GETITEMCOUNT, LVM_GETNEXTITEM, LVNI_FOCUSED};
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
    use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_DOWN};
    use windows::Win32::UI::Shell::{
        IDeskBand, IDockingWindow, IInputObjectSite, IInputObjectSite_Impl, DBIM_ACTUAL,
        DBIM_MINSIZE, DBIM_TITLE, DESKBANDINFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowExW, FindWindowW,
        IsWindowVisible, PeekMessageW, RegisterClassExW, SendMessageW, SetWindowTextW, ShowWindow,
        TranslateMessage, MSG, PM_REMOVE, SW_SHOW, WINDOW_EX_STYLE, WM_COPYDATA, WM_KEYDOWN,
        WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
    };

    thread_local! {
        /// Lo que recibió la ventana «Lince» falsa.
        static GOT: RefCell<Vec<(Command, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
        /// Avisos de foco que recibió el sitio falso.
        static FOCUS: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
    }

    type GetClassObject =
        unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn fail(msg: &str) -> String {
        msg.to_string()
    }

    unsafe extern "system" fn fake_lince(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_COPYDATA {
            let d = unsafe { &*(lp.0 as *const COPYDATASTRUCT) };
            let bytes = if d.lpData.is_null() {
                Vec::new()
            } else {
                unsafe { std::slice::from_raw_parts(d.lpData as *const u8, d.cbData as usize) }
                    .to_vec()
            };
            if let Some(cmd) = Command::from_id(d.dwData) {
                GOT.with(|g| g.borrow_mut().push((cmd, bytes)));
                return LRESULT(1);
            }
        }
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }

    unsafe extern "system" fn plain_window(
        hwnd: HWND,
        msg: u32,
        wp: WPARAM,
        lp: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }

    /// Sitio falso: la «barra de tareas» que contiene la caja.
    #[implement(IOleWindow, IInputObjectSite)]
    struct FakeSite {
        parent: HWND,
    }

    impl IOleWindow_Impl for FakeSite_Impl {
        fn GetWindow(&self) -> windows::core::Result<HWND> {
            Ok(self.parent)
        }
        fn ContextSensitiveHelp(&self, _: BOOL) -> windows::core::Result<()> {
            Ok(())
        }
    }

    impl IInputObjectSite_Impl for FakeSite_Impl {
        fn OnFocusChangeIS(
            &self,
            _obj: Ref<windows::core::IUnknown>,
            focused: BOOL,
        ) -> windows::core::Result<()> {
            FOCUS.with(|f| f.borrow_mut().push(focused.as_bool()));
            Ok(())
        }
    }

    /// Espera (atendiendo mensajes) hasta que `cond` se cumpla.
    unsafe fn wait_for(secs: u64, what: &str, cond: impl Fn() -> bool) -> Result<(), String> {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        while !cond() {
            if std::time::Instant::now() > end {
                return Err(format!("no pasó: {what}"));
            }
            let mut msg = MSG::default();
            while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Ok(())
    }

    /// Con el Lince de verdad: resultados encima de la caja.
    unsafe fn with_real_lince(edit: HWND) -> Result<(), String> {
        let popup_class = wide("LinceBarraRapida");
        let popup = unsafe { FindWindowW(PCWSTR(popup_class.as_ptr()), None) }
            .map_err(|_| fail("Lince no está abierto junto al reloj"))?;
        let list = unsafe { FindWindowExW(Some(popup), None, w!("SysListView32"), None) }
            .map_err(|_| fail("la ventana de resultados no tiene lista"))?;
        let count = || unsafe { SendMessageW(list, LVM_GETITEMCOUNT, None, None) }.0;
        let focused = || {
            unsafe {
                SendMessageW(
                    list,
                    LVM_GETNEXTITEM,
                    Some(WPARAM(usize::MAX)),
                    Some(LPARAM(LVNI_FOCUSED as isize)),
                )
            }
            .0
        };
        let text = wide("notepad");
        unsafe { SetWindowTextW(edit, PCWSTR(text.as_ptr())) }.map_err(|e| e.to_string())?;
        unsafe {
            wait_for(90, "resultados encima de la caja", || {
                IsWindowVisible(popup).as_bool() && count() > 1
            })
            .map_err(|e| {
                format!(
                    "{e} (ventana visible: {}, resultados: {})",
                    IsWindowVisible(popup).as_bool(),
                    count()
                )
            })?;
            SendMessageW(
                edit,
                WM_KEYDOWN,
                Some(WPARAM(VK_DOWN.0 as usize)),
                Some(LPARAM(0)),
            );
            wait_for(5, "la flecha mueve la elección", || focused() == 1)?;
            SendMessageW(
                edit,
                WM_KEYDOWN,
                Some(WPARAM(VK_ESCAPE.0 as usize)),
                Some(LPARAM(0)),
            );
            wait_for(5, "Esc esconde los resultados", || {
                !IsWindowVisible(popup).as_bool()
            })?;
        }
        println!("Lince mostró {} resultados de notepad", count());
        Ok(())
    }

    pub fn run() -> Result<(), String> {
        let dll = std::env::args()
            .nth(1)
            .ok_or_else(|| fail("falta la ruta de la DLL"))?;
        let real = std::env::args().any(|a| a == "--lince");
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;

            // Ventana «Lince» falsa que recibe las órdenes.
            let main_class = wide(if real {
                "LinceFalsoSinUsar"
            } else {
                MAIN_CLASS
            });
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(fake_lince),
                hInstance: module.into(),
                lpszClassName: PCWSTR(main_class.as_ptr()),
                ..Default::default()
            };
            RegisterClassExW(&wc);
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(main_class.as_ptr()),
                w!("Lince falso"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                100,
                100,
                None,
                None,
                Some(module.into()),
                None,
            )
            .map_err(|e| e.to_string())?;

            // «Barra de tareas» falsa: una ventana normal.
            let bar_class = w!("BarraDeTareasFalsa");
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(plain_window),
                hInstance: module.into(),
                lpszClassName: bar_class,
                ..Default::default()
            };
            RegisterClassExW(&wc);
            let parent = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                bar_class,
                w!("Barra"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                600,
                60,
                None,
                None,
                Some(module.into()),
                None,
            )
            .map_err(|e| e.to_string())?;
            let _ = ShowWindow(parent, SW_SHOW);

            // Como el Explorador: DllGetClassObject → IClassFactory → IDeskBand.
            let path = wide(&dll);
            let lib =
                LoadLibraryW(PCWSTR(path.as_ptr())).map_err(|e| format!("cargar la DLL: {e}"))?;
            let proc = GetProcAddress(lib, windows::core::s!("DllGetClassObject"))
                .ok_or_else(|| fail("la DLL no exporta DllGetClassObject"))?;
            let get: GetClassObject = std::mem::transmute(proc);
            let clsid = GUID::from_u128(BAND_CLSID_U128);
            let mut raw = std::ptr::null_mut();
            get(&clsid, &IClassFactory::IID, &mut raw)
                .ok()
                .map_err(|e| format!("DllGetClassObject: {e}"))?;
            let factory = IClassFactory::from_raw(raw);
            let band: IDeskBand = factory
                .CreateInstance(None)
                .map_err(|e| format!("CreateInstance: {e}"))?;

            let mut info = DESKBANDINFO {
                dwMask: DBIM_MINSIZE | DBIM_ACTUAL | DBIM_TITLE,
                ..Default::default()
            };
            band.GetBandInfo(0, 0, &mut info)
                .map_err(|e| format!("GetBandInfo: {e}"))?;
            if info.ptMinSize.x < 100 || info.ptActual.x < info.ptMinSize.x {
                return Err(format!(
                    "tamaños raros: mín {}x{} real {}x{}",
                    info.ptMinSize.x, info.ptMinSize.y, info.ptActual.x, info.ptActual.y
                ));
            }
            if info.dwMask & DBIM_TITLE != 0 {
                return Err(fail("la caja no debe llevar título"));
            }

            let site: IOleWindow = FakeSite { parent }.into();
            let with_site: IObjectWithSite = band.cast().map_err(|e| e.to_string())?;
            with_site
                .SetSite(&site)
                .map_err(|e| format!("SetSite: {e}"))?;
            let docking: IDockingWindow = band.cast().map_err(|e| e.to_string())?;
            docking.ShowDW(true).map_err(|e| e.to_string())?;
            let band_class = wide(BAND_CLASS);
            let band_hwnd = FindWindowExW(Some(parent), None, PCWSTR(band_class.as_ptr()), None)
                .map_err(|_| fail("la caja no creó su ventana dentro de la barra"))?;
            if band.GetWindow().map_err(|e| e.to_string())? != band_hwnd {
                return Err(fail(
                    "IOleWindow::GetWindow no devuelve la ventana de la caja",
                ));
            }
            let edit = FindWindowExW(Some(band_hwnd), None, w!("EDIT"), None)
                .map_err(|_| fail("la caja no tiene campo de texto"))?;

            if real {
                let r = with_real_lince(edit);
                with_site.SetSite(None).map_err(|e| e.to_string())?;
                return r;
            }

            // Escribir manda el texto; Flecha abajo, la tecla; el foco se avisa.
            let text = wide("informe ñandú");
            SetWindowTextW(edit, PCWSTR(text.as_ptr())).map_err(|e| e.to_string())?;
            SendMessageW(
                edit,
                WM_KEYDOWN,
                Some(WPARAM(VK_DOWN.0 as usize)),
                Some(LPARAM(0)),
            );
            let _ = SetFocus(Some(edit));

            let got = GOT.with(|g| g.borrow().clone());
            let texts: Vec<String> = got
                .iter()
                .filter(|(c, _)| *c == Command::Text)
                .map(|(_, b)| payload_text(b))
                .collect();
            if texts.last().map(String::as_str) != Some("informe ñandú") {
                return Err(format!("Lince no recibió lo escrito: {got:?}"));
            }
            if !got
                .iter()
                .any(|(c, b)| *c == Command::Key && Key::from_payload(b) == Some(Key::Down))
            {
                return Err(format!("Lince no recibió la flecha: {got:?}"));
            }
            if !FOCUS.with(|f| f.borrow().contains(&true)) {
                return Err(fail("la barra no se enteró del foco"));
            }

            // Quitar el sitio destruye la ventana de la caja.
            with_site.SetSite(None).map_err(|e| e.to_string())?;
            if FindWindowExW(Some(parent), None, PCWSTR(band_class.as_ptr()), None).is_ok() {
                return Err(fail("la ventana de la caja quedó viva"));
            }
        }
        Ok(())
    }
}
