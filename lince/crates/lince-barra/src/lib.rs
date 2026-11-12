//! Caja de Lince dentro de la barra de tareas de Windows 10: una «barra de
//! herramientas» (deskband) que carga el Explorador. Es solo la caja de
//! texto: lo escrito y las teclas van a Lince.exe, que busca y muestra los
//! resultados encima. Lo que hay que abrir lo abre la caja, desde el
//! Explorador, como usuario normal (no con los permisos de Lince).
//!
//! Windows 11 ya no carga barras de herramientas en la barra de tareas.

#![cfg(windows)]
#![allow(non_snake_case)] // nombres de las interfaces COM

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, AtomicU16, Ordering};

use lince_protocolo::{
    payload_text, text_payload, Command, Key, BAND_CLASS, BAND_CLSID_U128, EXE_VALUE, MAIN_CLASS,
    SETTINGS_KEY, STARTUP_EXE_VALUE, TASK_NAME,
};
use windows::core::{implement, w, IUnknownImpl, Interface, Ref, BOOL, GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG, E_NOTIMPL, HINSTANCE,
    HWND, LPARAM, LRESULT, POINTL, RECT, S_FALSE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontIndirectW, DeleteObject, EndPaint, HFONT, HGDIOBJ, PAINTSTRUCT,
};
use windows::Win32::System::Com::{
    IClassFactory, IClassFactory_Impl, IPersistStream, IPersistStream_Impl, IPersist_Impl, IStream,
};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::Ole::{
    IObjectWithSite, IObjectWithSite_Impl, IOleWindow, IOleWindow_Impl,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
use windows::Win32::UI::Controls::{DrawThemeParentBackground, EM_SETCUEBANNER};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, SetFocus, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_NEXT, VK_PRIOR, VK_RETURN,
    VK_UP,
};
use windows::Win32::UI::Shell::{
    DefSubclassProc, IDeskBand, IDeskBand2, IDeskBand2_Impl, IDeskBand_Impl, IDockingWindow,
    IDockingWindow_Impl, IInputObject, IInputObjectSite, IInputObject_Impl, SetWindowSubclass,
    ShellExecuteW, DBIMF_NORMAL, DBIMF_VARIABLEHEIGHT, DBIM_ACTUAL, DBIM_BKCOLOR, DBIM_INTEGRAL,
    DBIM_MAXSIZE, DBIM_MINSIZE, DBIM_MODEFLAGS, DBIM_TITLE, DESKBANDINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, GetClientRect, GetWindowLongPtrW,
    KillTimer, MoveWindow, RegisterClassExW, SendMessageTimeoutW, SendMessageW, SetTimer,
    SetWindowLongPtrW, SetWindowTextW, ShowWindow, SystemParametersInfoW, EN_CHANGE,
    ES_AUTOHSCROLL, GWLP_USERDATA, HMENU, MSG, NONCLIENTMETRICSW, SMTO_ABORTIFHUNG, SMTO_BLOCK,
    SPI_GETNONCLIENTMETRICS, SW_HIDE, SW_SHOW, SW_SHOWNORMAL, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    WINDOW_EX_STYLE, WM_CHAR, WM_COMMAND, WM_COPYDATA, WM_ERASEBKGND, WM_KEYDOWN, WM_KILLFOCUS,
    WM_PAINT, WM_SETFOCUS, WM_SETFONT, WM_SIZE, WM_TIMER, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS,
    WS_VISIBLE,
};

const CLSID: GUID = GUID::from_u128(BAND_CLSID_U128);
const ID_EDIT: usize = 1;
/// Mientras Lince arranca, se reintenta mandarle lo escrito.
const TIMER_RETRY: usize = 1;
const RETRIES: u16 = 30;

static MODULE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
/// Intentos que quedan para mandar lo escrito a un Lince que arranca.
static PENDING: AtomicU16 = AtomicU16::new(0);

#[unsafe(no_mangle)]
extern "system" fn DllMain(module: HINSTANCE, reason: u32, _: *mut c_void) -> BOOL {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason == DLL_PROCESS_ATTACH {
        MODULE.store(module.0, Ordering::Relaxed);
    }
    BOOL(1)
}

/// # Safety
/// La llama COM con punteros válidos.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || clsid.is_null() || iid.is_null() {
        return E_INVALIDARG;
    }
    unsafe { *out = std::ptr::null_mut() };
    if unsafe { *clsid } != CLSID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory.into();
    unsafe { factory.query(iid, out) }
}

/// Nunca se descarga: el Explorador puede tener todavía ventanas suyas.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<windows::core::IUnknown>,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let band: IDeskBand2 = Band::default().into();
        unsafe { band.query(iid, out).ok() }
    }

    fn LockServer(&self, _lock: BOOL) -> windows::core::Result<()> {
        Ok(())
    }
}

/// Lo que comparten la caja (objeto COM) y su ventana.
#[derive(Default)]
struct Shared {
    edit: Cell<HWND>,
    site: RefCell<Option<IInputObjectSite>>,
    /// La caja misma (sin contar referencia), para avisar del foco.
    me: Cell<*mut c_void>,
    font: Cell<HFONT>,
}

#[implement(IDeskBand2, IObjectWithSite, IPersistStream, IInputObject)]
#[derive(Default)]
struct Band {
    shared: Box<Shared>,
    hwnd: Cell<HWND>,
    site: RefCell<Option<windows::core::IUnknown>>,
}

impl Band {
    fn destroy_window(&self) {
        let hwnd = self.hwnd.replace(HWND::default());
        if !hwnd.0.is_null() {
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let _ = DestroyWindow(hwnd);
                let font = self.shared.font.replace(HFONT::default());
                if !font.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(font.0));
                }
            }
        }
        self.shared.edit.set(HWND::default());
    }

    unsafe fn create_window(&self, parent: HWND) -> windows::core::Result<()> {
        let module = HINSTANCE(MODULE.load(Ordering::Relaxed));
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(band_proc),
            hInstance: module,
            lpszClassName: class_name(),
            ..Default::default()
        };
        unsafe { RegisterClassExW(&wc) }; // si ya existe, falla sin importar
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name(),
                w!("Lince"),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                Some(module),
                None,
            )?
        };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, &*self.shared as *const Shared as isize);
        }
        let edit = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!(""),
                WS_CHILD
                    | WS_VISIBLE
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                0,
                0,
                0,
                0,
                Some(hwnd),
                Some(HMENU(ID_EDIT as *mut c_void)),
                Some(module),
                None,
            )?
        };
        self.shared.edit.set(edit);
        let font = message_font(hwnd);
        self.shared.font.set(font);
        unsafe {
            SendMessageW(
                edit,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
            let cue = wide("Buscar con Lince…");
            SendMessageW(
                edit,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(cue.as_ptr() as isize)),
            );
            let _ = SetWindowSubclass(
                edit,
                Some(edit_proc),
                1,
                &*self.shared as *const Shared as usize,
            );
        }
        self.hwnd.set(hwnd);
        Ok(())
    }

    fn dpi(&self) -> i32 {
        let hwnd = self.hwnd.get();
        let dpi = if hwnd.0.is_null() {
            96
        } else {
            unsafe { GetDpiForWindow(hwnd) }
        };
        dpi.max(96) as i32
    }
}

impl Drop for Band {
    fn drop(&mut self) {
        self.destroy_window();
    }
}

impl IOleWindow_Impl for Band_Impl {
    fn GetWindow(&self) -> windows::core::Result<HWND> {
        Ok(self.hwnd.get())
    }

    fn ContextSensitiveHelp(&self, _enter: BOOL) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
}

impl IDockingWindow_Impl for Band_Impl {
    fn ShowDW(&self, show: BOOL) -> windows::core::Result<()> {
        let hwnd = self.hwnd.get();
        if !hwnd.0.is_null() {
            unsafe {
                let _ = ShowWindow(hwnd, if show.as_bool() { SW_SHOW } else { SW_HIDE });
            }
        }
        Ok(())
    }

    fn CloseDW(&self, _reserved: u32) -> windows::core::Result<()> {
        self.destroy_window();
        Ok(())
    }

    fn ResizeBorderDW(
        &self,
        _border: *const RECT,
        _site: Ref<windows::core::IUnknown>,
        _reserved: BOOL,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
}

impl IDeskBand_Impl for Band_Impl {
    fn GetBandInfo(
        &self,
        _band_id: u32,
        _view_mode: u32,
        info: *mut DESKBANDINFO,
    ) -> windows::core::Result<()> {
        let Some(info) = (unsafe { info.as_mut() }) else {
            return Err(E_INVALIDARG.into());
        };
        let dpi = self.dpi();
        let s = |v: i32| v * dpi / 96;
        if info.dwMask & DBIM_MINSIZE != 0 {
            info.ptMinSize = POINTL {
                x: s(200),
                y: s(28),
            };
        }
        if info.dwMask & DBIM_MAXSIZE != 0 {
            info.ptMaxSize = POINTL { x: -1, y: -1 };
        }
        if info.dwMask & DBIM_INTEGRAL != 0 {
            info.ptIntegral = POINTL { x: 1, y: 1 };
        }
        if info.dwMask & DBIM_ACTUAL != 0 {
            info.ptActual = POINTL {
                x: s(320),
                y: s(32),
            };
        }
        // Sin título al lado de la caja ni color de fondo propio.
        info.dwMask &= !(DBIM_TITLE | DBIM_BKCOLOR);
        if info.dwMask & DBIM_MODEFLAGS != 0 {
            info.dwModeFlags = DBIMF_NORMAL | DBIMF_VARIABLEHEIGHT;
        }
        Ok(())
    }
}

impl IDeskBand2_Impl for Band_Impl {
    fn CanRenderComposited(&self) -> windows::core::Result<BOOL> {
        Ok(BOOL(1))
    }

    fn SetCompositionState(&self, _enabled: BOOL) -> windows::core::Result<()> {
        Ok(())
    }

    fn GetCompositionState(&self) -> windows::core::Result<BOOL> {
        Ok(BOOL(1))
    }
}

impl IObjectWithSite_Impl for Band_Impl {
    fn SetSite(&self, site: Ref<windows::core::IUnknown>) -> windows::core::Result<()> {
        self.destroy_window();
        *self.shared.site.borrow_mut() = None;
        *self.site.borrow_mut() = None;
        let Some(site) = site.as_ref() else {
            return Ok(());
        };
        let window: IOleWindow = site.cast()?;
        let parent = unsafe { window.GetWindow()? };
        *self.shared.site.borrow_mut() = site.cast::<IInputObjectSite>().ok();
        *self.site.borrow_mut() = Some(site.clone());
        // Puntero sin referencia: la ventana muere antes que la caja.
        let me: windows::core::IUnknown = self.to_interface();
        self.shared.me.set(me.as_raw());
        unsafe { self.create_window(parent) }
    }

    fn GetSite(&self, iid: *const GUID, out: *mut *mut c_void) -> windows::core::Result<()> {
        match &*self.site.borrow() {
            Some(site) => unsafe { site.query(iid, out).ok() },
            None => Err(E_FAIL.into()),
        }
    }
}

impl IPersist_Impl for Band_Impl {
    fn GetClassID(&self) -> windows::core::Result<GUID> {
        Ok(CLSID)
    }
}

impl IPersistStream_Impl for Band_Impl {
    fn IsDirty(&self) -> HRESULT {
        S_FALSE
    }

    fn Load(&self, _stream: Ref<IStream>) -> windows::core::Result<()> {
        Ok(())
    }

    fn Save(&self, _stream: Ref<IStream>, _clear_dirty: BOOL) -> windows::core::Result<()> {
        Ok(())
    }

    fn GetSizeMax(&self) -> windows::core::Result<u64> {
        Ok(0)
    }
}

impl IInputObject_Impl for Band_Impl {
    fn UIActivateIO(&self, activate: BOOL, _msg: *const MSG) -> windows::core::Result<()> {
        if activate.as_bool() {
            unsafe {
                let _ = SetFocus(Some(self.shared.edit.get()));
            }
        }
        Ok(())
    }

    fn HasFocusIO(&self) -> windows::core::Result<()> {
        if unsafe { GetFocus() } == self.shared.edit.get() {
            Ok(())
        } else {
            Err(S_FALSE.into())
        }
    }

    /// Las teclas de edición (borrar, flechas, Ctrl+V...) llegan a la caja.
    fn TranslateAcceleratorIO(&self, msg: *const MSG) -> windows::core::Result<()> {
        let Some(msg) = (unsafe { msg.as_ref() }) else {
            return Err(S_FALSE.into());
        };
        if msg.hwnd == self.shared.edit.get() {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(msg);
                windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(msg);
            }
            Ok(())
        } else {
            Err(S_FALSE.into())
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn class_name() -> PCWSTR {
    static NAME: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    PCWSTR(NAME.get_or_init(|| wide(BAND_CLASS)).as_ptr())
}

/// Letra del sistema (la de los menús), un poco más grande.
fn message_font(hwnd: HWND) -> HFONT {
    let mut ncm = NONCLIENTMETRICSW {
        cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            ncm.cbSize,
            Some(&mut ncm as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let dpi = GetDpiForWindow(hwnd).max(96) as i32;
        let mut lf = ncm.lfMessageFont;
        lf.lfHeight = -(10 * dpi / 72); // 10 puntos
        CreateFontIndirectW(&lf)
    }
}

fn window_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let n = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// Manda una orden a Lince. `false` si no está en marcha o no responde.
fn send(from: HWND, cmd: Command, payload: &[u8]) -> bool {
    let main = wide(MAIN_CLASS);
    let Ok(target) = (unsafe { FindWindowW(PCWSTR(main.as_ptr()), PCWSTR::null()) }) else {
        return false;
    };
    let data = COPYDATASTRUCT {
        dwData: cmd.id(),
        cbData: payload.len() as u32,
        lpData: payload.as_ptr() as *mut c_void,
    };
    // Con tiempo límite: un Lince colgado no debe congelar la barra.
    let r = unsafe {
        SendMessageTimeoutW(
            target,
            WM_COPYDATA,
            WPARAM(from.0 as usize),
            LPARAM(&data as *const COPYDATASTRUCT as isize),
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            500,
            None,
        )
    };
    r.0 != 0
}

/// Lince no está en marcha: se abre con la tarea de inicio (no pregunta
/// nada); sin tarea, el programa directamente (Windows pedirá permiso).
fn start_lince() {
    if read_setting(STARTUP_EXE_VALUE).is_some() {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let schtasks = wide(&format!(r"{root}\System32\schtasks.exe"));
        let args = wide(&format!("/Run /TN \"{TASK_NAME}\""));
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(schtasks.as_ptr()),
                PCWSTR(args.as_ptr()),
                PCWSTR::null(),
                SW_HIDE,
            );
        }
    } else if let Some(exe) = read_setting(EXE_VALUE) {
        let exe = wide(&exe);
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(exe.as_ptr()),
                w!("--inicio"),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }
}

fn read_setting(name: &str) -> Option<String> {
    let key = wide(SETTINGS_KEY);
    let name = wide(name);
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    };
    if r.is_err() {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..len]))
}

unsafe fn shared<'a>(hwnd: HWND) -> Option<&'a Shared> {
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Shared;
    unsafe { p.as_ref() }
}

/// Manda lo escrito; si Lince no está, lo abre y reintenta un rato.
fn send_text(hwnd: HWND, edit: HWND) {
    let text = window_text(edit);
    let cmd = if text.trim().is_empty() {
        Command::Hide
    } else {
        Command::Text
    };
    if send(hwnd, cmd, &text_payload(&text)) || cmd == Command::Hide {
        PENDING.store(0, Ordering::Relaxed);
        return;
    }
    if PENDING.swap(RETRIES, Ordering::Relaxed) == 0 {
        start_lince();
    }
    unsafe {
        SetTimer(Some(hwnd), TIMER_RETRY, 500, None);
    }
}

unsafe extern "system" fn band_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let Some(sh) = (unsafe { shared(hwnd) }) else {
        return unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
    };
    match msg {
        WM_SIZE => {
            let mut rc = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rc);
            }
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
            let h = (24 * dpi / 96).min(rc.bottom);
            let pad = 4 * dpi / 96;
            unsafe {
                let _ = MoveWindow(
                    sh.edit.get(),
                    pad,
                    (rc.bottom - h) / 2,
                    (rc.right - 2 * pad).max(0),
                    h,
                    true,
                );
            }
            LRESULT(0)
        }
        // El fondo es el de la barra de tareas.
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe {
                let hdc = BeginPaint(hwnd, &mut ps);
                let _ = DrawThemeParentBackground(hwnd, hdc, Some(&ps.rcPaint));
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_COMMAND if (wp.0 & 0xFFFF) == ID_EDIT && ((wp.0 >> 16) as u32) == EN_CHANGE => {
            send_text(hwnd, sh.edit.get());
            LRESULT(0)
        }
        WM_TIMER if wp.0 == TIMER_RETRY => {
            let left = PENDING.load(Ordering::Relaxed);
            if left == 0 {
                unsafe {
                    let _ = KillTimer(Some(hwnd), TIMER_RETRY);
                }
            } else {
                PENDING.store(left - 1, Ordering::Relaxed);
                let text = window_text(sh.edit.get());
                if send(hwnd, Command::Text, &text_payload(&text)) {
                    PENDING.store(0, Ordering::Relaxed);
                }
            }
            LRESULT(0)
        }
        // Lince pide abrir algo: se abre desde aquí, como usuario normal.
        WM_COPYDATA => {
            let data = unsafe { &*(lp.0 as *const COPYDATASTRUCT) };
            let bytes = if data.lpData.is_null() {
                &[][..]
            } else {
                unsafe {
                    std::slice::from_raw_parts(data.lpData as *const u8, data.cbData as usize)
                }
            };
            let path = payload_text(bytes);
            match Command::from_id(data.dwData) {
                Some(Command::Open) if !path.is_empty() => open(&path, false),
                Some(Command::Reveal) if !path.is_empty() => open(&path, true),
                _ => return LRESULT(0),
            }
            LRESULT(1)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn open(path: &str, reveal: bool) {
    let (file, params) = if reveal {
        ("explorer.exe".to_string(), format!("/select,\"{path}\""))
    } else {
        (path.to_string(), String::new())
    };
    let (file, params) = (wide(&file), wide(&params));
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(file.as_ptr()),
            PCWSTR(params.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Avisa a la barra de tareas que la caja ganó o perdió el foco.
fn focus_changed(sh: &Shared, focused: bool) {
    let me = sh.me.get();
    if me.is_null() {
        return;
    }
    if let (Some(site), Some(me)) = (sh.site.borrow().as_ref(), unsafe {
        windows::core::IUnknown::from_raw_borrowed(&me)
    }) {
        unsafe {
            let _ = site.OnFocusChangeIS(me, focused);
        }
    }
}

/// Teclas de la caja: flechas, Enter y Esc se las lleva Lince.
unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    let sh = unsafe { &*(data as *const Shared) };
    let band =
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetParent(hwnd) }.unwrap_or_default();
    match msg {
        WM_SETFOCUS => focus_changed(sh, true),
        WM_KILLFOCUS => {
            focus_changed(sh, false);
            send(band, Command::Hide, &[]);
        }
        WM_KEYDOWN => {
            let key = match wp.0 as u16 {
                k if k == VK_DOWN.0 => Some(Key::Down),
                k if k == VK_UP.0 => Some(Key::Up),
                k if k == VK_NEXT.0 => Some(Key::PageDown),
                k if k == VK_PRIOR.0 => Some(Key::PageUp),
                k if k == VK_RETURN.0 => {
                    let ctrl = unsafe { GetKeyState(i32::from(VK_CONTROL.0)) } < 0;
                    Some(if ctrl { Key::CtrlEnter } else { Key::Enter })
                }
                k if k == VK_ESCAPE.0 => {
                    unsafe {
                        let _ = SetWindowTextW(hwnd, w!(""));
                    }
                    Some(Key::Escape)
                }
                _ => None,
            };
            if let Some(key) = key {
                send(band, Command::Key, &key.payload());
                return LRESULT(0);
            }
        }
        // Sin el pitido de Enter y Esc en una caja de una línea.
        WM_CHAR if wp.0 == 0x0D || wp.0 == 0x1B => return LRESULT(0),
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
}

// Comprobación en tiempo de compilación de que la caja expone las
// interfaces que pide el Explorador.
#[allow(dead_code)]
fn _interfaces(b: IDeskBand2) -> (IDeskBand, IDockingWindow, IInputObject, IObjectWithSite) {
    (
        b.cast().unwrap(),
        b.cast().unwrap(),
        b.cast().unwrap(),
        b.cast().unwrap(),
    )
}
