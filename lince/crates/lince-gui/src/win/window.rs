//! Ventana principal: caja de búsqueda, lista virtual de resultados y barra
//! de estado.
//!
//! La lista puede llamar de vuelta a esta ventana mientras se le envían
//! mensajes (o mientras hay un diálogo modal abierto), así que el estado vive
//! en `Cell`/`RefCell` y nunca se mantiene un préstamo durante una llamada a
//! Windows. Los resultados se comparten con `Rc` para poder soltarlos.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;

use lince_core::{seconds, thousands, write_csv, Method, Source};
use lince_gui::{cell_text, Column, Engine, Event, Results, COLUMNS};
use windows_sys::core::{w, PCWSTR};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DeleteObject, InvalidateRect, HFONT, HGDIOBJ,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{
    SetWindowTheme, EM_SETCUEBANNER, EM_SETSEL, HDF_SORTDOWN, HDF_SORTUP, HDITEMW, HDI_FORMAT,
    HDM_GETITEMW, HDM_SETITEMW, LVCFMT_LEFT, LVCFMT_RIGHT, LVCF_FMT, LVCF_SUBITEM, LVCF_TEXT,
    LVCF_WIDTH, LVCOLUMNW, LVIF_IMAGE, LVIF_TEXT, LVIS_FOCUSED, LVIS_SELECTED, LVITEMW,
    LVM_ENSUREVISIBLE, LVM_GETHEADER, LVM_GETITEMCOUNT, LVM_GETNEXTITEM, LVM_INSERTCOLUMNW,
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETIMAGELIST, LVM_SETITEMCOUNT, LVM_SETITEMSTATE,
    LVNI_SELECTED, LVN_COLUMNCLICK, LVN_GETDISPINFOW, LVN_KEYDOWN, LVSIL_SMALL,
    LVS_EX_DOUBLEBUFFER, LVS_EX_FULLROWSELECT, LVS_EX_HEADERDRAGDROP, LVS_OWNERDATA, LVS_REPORT,
    LVS_SHAREIMAGELISTS, LVS_SHOWSELALWAYS, NMHDR, NMLISTVIEW, NMLVDISPINFOW, NMLVKEYDOWN,
    NM_DBLCLK, NM_RCLICK, SBARS_SIZEGRIP, SB_SETPARTS, SB_SETTEXTW, STATUSCLASSNAMEW, WC_LISTVIEWW,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, SetFocus, VK_CONTROL, VK_DOWN, VK_NEXT,
};
use windows_sys::Win32::UI::Shell::{
    DefSubclassProc, SHGetFileInfoW, SetWindowSubclass, SHFILEINFOW, SHGFI_SMALLICON,
    SHGFI_SYSICONINDEX, SHGFI_USEFILEATTRIBUTES,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, GetClientRect,
    GetCursorPos, GetWindowLongPtrW, GetWindowRect, KillTimer, MoveWindow, PostMessageW,
    PostQuitMessage, SendMessageW, SetMenuDefaultItem, SetTimer, SetWindowLongPtrW, SetWindowPos,
    SetWindowTextW, TrackPopupMenu, BS_PUSHBUTTON, CBN_SELCHANGE, CBS_DROPDOWNLIST, CB_ADDSTRING,
    CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL, CB_SETITEMHEIGHT, CREATESTRUCTW, EN_CHANGE,
    ES_AUTOHSCROLL, GWLP_USERDATA, HMENU, IDCANCEL, IDOK, MB_ICONERROR, MB_ICONINFORMATION,
    MB_ICONWARNING, MF_SEPARATOR, MF_STRING, MINMAXINFO, NONCLIENTMETRICSW,
    SPI_GETNONCLIENTMETRICS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WM_APP, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_DPICHANGED, WM_GETMINMAXINFO,
    WM_KEYDOWN, WM_NCDESTROY, WM_NOTIFY, WM_SETFONT, WM_SIZE, WM_TIMER, WS_CHILD, WS_EX_CLIENTEDGE,
    WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

use super::util::{
    message_box, open_path, save_csv_dialog, set_clipboard, show_in_folder, wide, window_text,
};

/// Mensaje con un `Box<Event>` del motor en `lParam`.
pub const WM_ENGINE: u32 = WM_APP + 1;

const ID_EDIT: u16 = 101;
const ID_COMBO: u16 = 102;
const ID_EXPORT: u16 = 103;
const ID_LIST: u16 = 104;
const ID_STATUS: u16 = 105;

pub const IDM_OPEN: u16 = 201;
pub const IDM_FOLDER: u16 = 202;
pub const IDM_COPY_PATH: u16 = 203;
pub const IDM_COPY_NAME: u16 = 204;
pub const IDM_EXPORT: u16 = 205;
pub const IDM_REFRESH: u16 = 206;
pub const IDM_FOCUS_SEARCH: u16 = 207;

const TIMER_SEARCH: usize = 1;
/// Espera tras la última tecla antes de buscar.
const DEBOUNCE_MS: u32 = 90;

const CUE: &str = "Escriba para buscar…   ej.: informe    *.pdf    size:>10mb    dm:2026-10";
const ALL_VOLUMES: &str = "Todas las unidades";

/// Datos que se pasan al crear la ventana.
pub struct Config {
    pub sources: Vec<Source>,
}

pub const CLASS_NAME: PCWSTR = w!("LinceVentanaPrincipal");

struct App {
    hwnd: HWND,
    edit: HWND,
    combo: HWND,
    export: HWND,
    list: HWND,
    status: HWND,
    dpi: Cell<u32>,
    fonts: Cell<(HFONT, HFONT)>,
    engine: Engine,
    sources: Vec<Source>,
    labels: RefCell<Vec<String>>,
    generation: Cell<u64>,
    shown: Cell<u64>,
    results: RefCell<Option<Rc<Results>>>,
    sort: Cell<(Column, bool)>,
    icons: RefCell<HashMap<String, i32>>,
}

/// Procedimiento de la ventana principal.
///
/// # Safety
/// Solo lo llama Windows con mensajes de la clase registrada en `run`.
pub unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_CREATE {
        let cs = &*(lp as *const CREATESTRUCTW);
        let cfg = Box::from_raw(cs.lpCreateParams as *mut Config);
        let app = Box::into_raw(Box::new(App::new(hwnd, *cfg)));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app as isize);
        (*app).start();
        return 0;
    }
    let app = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const App;
    if app.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    if msg == WM_NCDESTROY {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        drop(Box::from_raw(app as *mut App));
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    match (*app).handle(msg, wp, lp) {
        Some(r) => r,
        None => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn loword(v: usize) -> u16 {
    (v & 0xFFFF) as u16
}

fn hiword(v: usize) -> u16 {
    ((v >> 16) & 0xFFFF) as u16
}

unsafe fn send(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    SendMessageW(hwnd, msg, wp, lp)
}

unsafe fn create_control(
    parent: HWND,
    ex_style: u32,
    class: PCWSTR,
    text: &str,
    style: u32,
    id: u16,
) -> HWND {
    let text = wide(text);
    CreateWindowExW(
        ex_style,
        class,
        text.as_ptr(),
        style,
        0,
        0,
        0,
        0,
        parent,
        id as usize as HMENU,
        GetModuleHandleW(ptr::null()),
        ptr::null(),
    )
}

impl App {
    /// Crea los controles. Se llama dentro de WM_CREATE, antes de guardar el
    /// puntero, así que los mensajes que lleguen mientras tanto van a
    /// DefWindowProc.
    unsafe fn new(hwnd: HWND, cfg: Config) -> App {
        let edit = create_control(
            hwnd,
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            "",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            ID_EDIT,
        );
        let combo = create_control(
            hwnd,
            0,
            w!("COMBOBOX"),
            "",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
            ID_COMBO,
        );
        let export = create_control(
            hwnd,
            0,
            w!("BUTTON"),
            "Exportar a Excel…",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
            ID_EXPORT,
        );
        let list = create_control(
            hwnd,
            0,
            WC_LISTVIEWW,
            "",
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | LVS_REPORT
                | LVS_OWNERDATA
                | LVS_SHOWSELALWAYS
                | LVS_SHAREIMAGELISTS,
            ID_LIST,
        );
        let status = create_control(
            hwnd,
            0,
            STATUSCLASSNAMEW,
            "",
            WS_CHILD | WS_VISIBLE | SBARS_SIZEGRIP,
            ID_STATUS,
        );

        // HWND no es Send: se pasa como número al hilo del motor.
        let target = hwnd as isize;
        let engine = Engine::start(move |ev| {
            let raw = Box::into_raw(Box::new(ev));
            // SAFETY: si la ventana ya no existe PostMessage falla y el
            // evento se libera aquí; si no, lo libera el hilo de la ventana.
            unsafe {
                if PostMessageW(target as HWND, WM_ENGINE, 0, raw as LPARAM) == 0 {
                    drop(Box::from_raw(raw));
                }
            }
        });

        let app = App {
            hwnd,
            edit,
            combo,
            export,
            list,
            status,
            dpi: Cell::new(GetDpiForWindow(hwnd).max(96)),
            fonts: Cell::new((ptr::null_mut(), ptr::null_mut())),
            engine,
            sources: cfg.sources,
            labels: RefCell::new(Vec::new()),
            generation: Cell::new(0),
            shown: Cell::new(0),
            results: RefCell::new(None),
            sort: Cell::new((Column::Name, false)),
            icons: RefCell::new(HashMap::new()),
        };
        app.setup_controls();
        app
    }

    fn scale(&self, v: i32) -> i32 {
        v * self.dpi.get() as i32 / 96
    }

    unsafe fn setup_controls(&self) {
        let cue = wide(CUE);
        send(self.edit, EM_SETCUEBANNER, 1, cue.as_ptr() as isize);
        SetWindowSubclass(self.edit, Some(edit_proc), 1, self.list as usize);

        let all = wide(ALL_VOLUMES);
        send(self.combo, CB_ADDSTRING, 0, all.as_ptr() as isize);
        send(self.combo, CB_SETCURSEL, 0, 0);

        send(
            self.list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | LVS_EX_HEADERDRAGDROP) as isize,
        );
        SetWindowTheme(self.list, w!("Explorer"), ptr::null());
        for (k, col) in COLUMNS.iter().enumerate() {
            let mut title = wide(col.title());
            let mut c: LVCOLUMNW = std::mem::zeroed();
            c.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_FMT | LVCF_SUBITEM;
            c.fmt = if col.right_aligned() {
                LVCFMT_RIGHT
            } else {
                LVCFMT_LEFT
            };
            c.cx = self.scale(col.width());
            c.pszText = title.as_mut_ptr();
            c.iSubItem = k as i32;
            send(
                self.list,
                LVM_INSERTCOLUMNW,
                k,
                &c as *const LVCOLUMNW as isize,
            );
        }
        // Lista de iconos del sistema (compartida: la lista no debe destruirla).
        let mut sfi: SHFILEINFOW = std::mem::zeroed();
        let himl = SHGetFileInfoW(
            w!("a.txt"),
            FILE_ATTRIBUTE_NORMAL,
            &mut sfi,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_SYSICONINDEX | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES,
        );
        send(
            self.list,
            LVM_SETIMAGELIST,
            LVSIL_SMALL as usize,
            himl as isize,
        );

        self.set_fonts();
        self.update_sort_arrows();
        SetWindowPos(
            self.hwnd,
            ptr::null_mut(),
            0,
            0,
            self.scale(1150),
            self.scale(720),
            SWP_NOMOVE | SWP_NOZORDER,
        );
    }

    unsafe fn start(&self) {
        self.engine.reindex(self.sources.clone());
        self.request_search();
        SetFocus(self.edit);
    }

    unsafe fn handle(&self, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_SIZE => {
                self.layout();
                Some(0)
            }
            WM_GETMINMAXINFO => {
                let mmi = &mut *(lp as *mut MINMAXINFO);
                mmi.ptMinTrackSize = POINT {
                    x: self.scale(640),
                    y: self.scale(360),
                };
                Some(0)
            }
            WM_COMMAND => {
                self.on_command(loword(wp), hiword(wp));
                Some(0)
            }
            WM_TIMER if wp == TIMER_SEARCH => {
                KillTimer(self.hwnd, TIMER_SEARCH);
                self.request_search();
                Some(0)
            }
            WM_NOTIFY => self.on_notify(lp),
            WM_ENGINE => {
                let ev = *Box::from_raw(lp as *mut Event);
                self.on_event(ev);
                Some(0)
            }
            WM_DPICHANGED => {
                self.dpi.set(u32::from(loword(wp)).max(96));
                self.set_fonts();
                let r = &*(lp as *const RECT);
                SetWindowPos(
                    self.hwnd,
                    ptr::null_mut(),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                Some(0)
            }
            WM_DESTROY => {
                let (a, b) = self.fonts.get();
                DeleteObject(a as HGDIOBJ);
                DeleteObject(b as HGDIOBJ);
                PostQuitMessage(0);
                Some(0)
            }
            _ => None,
        }
    }

    unsafe fn set_fonts(&self) {
        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS,
            ncm.cbSize,
            &mut ncm as *mut NONCLIENTMETRICSW as *mut c_void,
            0,
            self.dpi.get(),
        );
        let normal = CreateFontIndirectW(&ncm.lfMessageFont);
        let mut big_lf = ncm.lfMessageFont;
        big_lf.lfHeight = big_lf.lfHeight * 5 / 4;
        let big = CreateFontIndirectW(&big_lf);
        for h in [self.combo, self.export, self.list, self.status] {
            send(h, WM_SETFONT, normal as usize, 1);
        }
        send(self.edit, WM_SETFONT, big as usize, 1);
        let (old_a, old_b) = self.fonts.replace((normal, big));
        if !old_a.is_null() {
            DeleteObject(old_a as HGDIOBJ);
            DeleteObject(old_b as HGDIOBJ);
        }
        self.layout();
    }

    unsafe fn layout(&self) {
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(self.hwnd, &mut rc);
        send(self.status, WM_SIZE, 0, 0);
        let mut sr: RECT = std::mem::zeroed();
        GetWindowRect(self.status, &mut sr);
        let status_h = sr.bottom - sr.top;

        let pad = self.scale(8);
        let h = self.scale(30);
        let combo_w = self.scale(190);
        let btn_w = self.scale(150);
        let w = rc.right;
        let edit_w = (w - 4 * pad - combo_w - btn_w).max(self.scale(120));
        MoveWindow(self.edit, pad, pad, edit_w, h, 1);
        MoveWindow(
            self.combo,
            2 * pad + edit_w,
            pad,
            combo_w,
            self.scale(300),
            1,
        );
        // Alto del combo cerrado igual al de la caja de búsqueda.
        send(
            self.combo,
            CB_SETITEMHEIGHT,
            usize::MAX,
            (h - self.scale(6)) as isize,
        );
        MoveWindow(self.export, 3 * pad + edit_w + combo_w, pad, btn_w, h, 1);
        let top = 2 * pad + h;
        MoveWindow(self.list, 0, top, w, (rc.bottom - top - status_h).max(0), 1);
        let parts = [(w - self.scale(430)).max(self.scale(200)), -1];
        send(self.status, SB_SETPARTS, 2, parts.as_ptr() as isize);
    }

    unsafe fn set_status(&self, part: usize, text: &str) {
        let t = wide(text);
        send(self.status, SB_SETTEXTW, part, t.as_ptr() as isize);
    }

    fn current(&self) -> Option<Rc<Results>> {
        self.results.borrow().clone()
    }

    unsafe fn request_search(&self) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let text = window_text(self.edit);
        let (col, desc) = self.sort.get();
        let sel = send(self.combo, CB_GETCURSEL, 0, 0);
        let volume = if sel > 0 {
            self.labels.borrow().get(sel as usize - 1).cloned()
        } else {
            None
        };
        self.engine
            .search(generation, &text, col.sort_key(), desc, volume);
    }

    unsafe fn on_command(&self, id: u16, code: u16) {
        match id {
            ID_EDIT if u32::from(code) == EN_CHANGE => {
                SetTimer(self.hwnd, TIMER_SEARCH, DEBOUNCE_MS, None);
            }
            ID_COMBO if u32::from(code) == CBN_SELCHANGE => self.request_search(),
            ID_EXPORT | IDM_EXPORT => self.export(),
            IDM_OPEN => self.open_selected(),
            IDM_FOLDER => self.show_folder(),
            IDM_COPY_PATH => self.copy(true),
            IDM_COPY_NAME => self.copy(false),
            IDM_REFRESH => self.engine.reindex(self.sources.clone()),
            IDM_FOCUS_SEARCH => {
                SetFocus(self.edit);
                send(self.edit, EM_SETSEL, 0, -1);
            }
            // Enter (IsDialogMessage): abre en la lista, busca ya en la caja.
            x if i32::from(x) == IDOK => {
                if GetFocus() == self.list {
                    self.open_selected();
                } else {
                    KillTimer(self.hwnd, TIMER_SEARCH);
                    self.request_search();
                }
            }
            // Esc: borra la búsqueda.
            x if i32::from(x) == IDCANCEL
                && GetFocus() == self.edit
                && !window_text(self.edit).is_empty() =>
            {
                SetWindowTextW(self.edit, w!(""));
            }
            _ => {}
        }
    }

    unsafe fn on_notify(&self, lp: LPARAM) -> Option<LRESULT> {
        let hdr = &*(lp as *const NMHDR);
        if hdr.hwndFrom != self.list {
            return None;
        }
        match hdr.code {
            LVN_GETDISPINFOW => {
                self.fill_item(&mut *(lp as *mut NMLVDISPINFOW));
                Some(0)
            }
            LVN_COLUMNCLICK => {
                let nm = &*(lp as *const NMLISTVIEW);
                if let Some(&col) = COLUMNS.get(nm.iSubItem as usize) {
                    let (cur, desc) = self.sort.get();
                    let desc = if cur == col {
                        !desc
                    } else {
                        // Tamaño y fecha: primero lo más grande / lo más nuevo.
                        matches!(col, Column::Size | Column::Modified)
                    };
                    self.sort.set((col, desc));
                    self.update_sort_arrows();
                    self.request_search();
                }
                Some(0)
            }
            NM_DBLCLK => {
                self.open_selected();
                Some(0)
            }
            NM_RCLICK => {
                self.context_menu();
                Some(0)
            }
            LVN_KEYDOWN => {
                let key = (*(lp as *const NMLVKEYDOWN)).wVKey;
                if GetKeyState(i32::from(VK_CONTROL)) < 0 {
                    match key {
                        k if k == u16::from(b'C') => self.copy(true),
                        k if k == u16::from(b'A') => self.select_all(),
                        _ => {}
                    }
                }
                Some(0)
            }
            _ => None,
        }
    }

    unsafe fn fill_item(&self, info: &mut NMLVDISPINFOW) {
        let Some(r) = self.current() else { return };
        let item = &mut info.item;
        let Some(&hit) = r.hits.get(item.iItem as usize) else {
            return;
        };
        if item.mask & LVIF_TEXT != 0 && !item.pszText.is_null() && item.cchTextMax > 0 {
            let col = COLUMNS
                .get(item.iSubItem as usize)
                .copied()
                .unwrap_or(Column::Name);
            let text: Vec<u16> = cell_text(&r.indexes, hit, col).encode_utf16().collect();
            let n = text.len().min(item.cchTextMax as usize - 1);
            ptr::copy_nonoverlapping(text.as_ptr(), item.pszText, n);
            *item.pszText.add(n) = 0;
        }
        if item.mask & LVIF_IMAGE != 0 && item.iSubItem == 0 {
            let (src, i) = hit;
            let ix = &r.indexes[src];
            item.iImage = self.icon_for(ix.name(i), ix.is_dir(i));
        }
    }

    /// Icono por extensión (rápido: no lee el archivo).
    unsafe fn icon_for(&self, name: &str, is_dir: bool) -> i32 {
        let key = if is_dir {
            "\\carpeta".to_string()
        } else {
            match name.rfind('.') {
                Some(dot) => name[dot..].to_lowercase(),
                None => String::new(),
            }
        };
        if let Some(&i) = self.icons.borrow().get(&key) {
            return i;
        }
        let probe = wide(&format!("a{key}"));
        let attrs = if is_dir {
            FILE_ATTRIBUTE_DIRECTORY
        } else {
            FILE_ATTRIBUTE_NORMAL
        };
        let mut sfi: SHFILEINFOW = std::mem::zeroed();
        SHGetFileInfoW(
            probe.as_ptr(),
            attrs,
            &mut sfi,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_SYSICONINDEX | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES,
        );
        self.icons.borrow_mut().insert(key, sfi.iIcon);
        sfi.iIcon
    }

    unsafe fn on_event(&self, ev: Event) {
        match ev {
            Event::Indexing { labels } => {
                self.set_status(1, &format!("Indexando {}…", labels.join(", ")));
            }
            Event::Indexed {
                entries,
                elapsed,
                stats,
                warnings,
            } => {
                let labels: Vec<String> = stats.iter().map(|s| s.label.clone()).collect();
                self.fill_volumes(labels);
                let walked = stats
                    .iter()
                    .any(|s| matches!(s.method, Method::Walk { .. }));
                let mut text = format!(
                    "{} objetos indexados en {}",
                    thousands(entries as u64),
                    seconds(elapsed)
                );
                if walked {
                    text.push_str(" · sin administrador (más lento)");
                }
                self.set_status(1, &text);
                if stats.is_empty() {
                    message_box(
                        self.hwnd,
                        &format!("No se pudo leer ninguna unidad:\n\n{}", warnings.join("\n")),
                        MB_ICONERROR,
                    );
                } else if !warnings.is_empty() {
                    message_box(self.hwnd, &warnings.join("\n\n"), MB_ICONWARNING);
                }
            }
            Event::Results(r) => {
                if r.generation < self.shown.get() {
                    return;
                }
                self.shown.set(r.generation);
                let n = r.hits.len();
                let status = match &r.error {
                    Some(e) => format!("⚠ {e}"),
                    None => format!(
                        "{} resultados ({} ms)",
                        thousands(n as u64),
                        r.elapsed.as_millis()
                    ),
                };
                *self.results.borrow_mut() = Some(Rc::new(r));
                send(self.list, LVM_SETITEMCOUNT, n, 0);
                let mut none: LVITEMW = std::mem::zeroed();
                none.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
                send(
                    self.list,
                    LVM_SETITEMSTATE,
                    usize::MAX,
                    &none as *const LVITEMW as isize,
                );
                if n > 0 {
                    send(self.list, LVM_ENSUREVISIBLE, 0, 0);
                }
                InvalidateRect(self.list, ptr::null(), 0);
                self.set_status(0, &status);
            }
        }
    }

    unsafe fn fill_volumes(&self, labels: Vec<String>) {
        let sel = send(self.combo, CB_GETCURSEL, 0, 0);
        let previous = if sel > 0 {
            self.labels.borrow().get(sel as usize - 1).cloned()
        } else {
            None
        };
        send(self.combo, CB_RESETCONTENT, 0, 0);
        let all = wide(ALL_VOLUMES);
        send(self.combo, CB_ADDSTRING, 0, all.as_ptr() as isize);
        for l in &labels {
            let t = wide(l);
            send(self.combo, CB_ADDSTRING, 0, t.as_ptr() as isize);
        }
        let keep = previous
            .and_then(|p| labels.iter().position(|l| *l == p))
            .map_or(0, |k| k + 1);
        send(self.combo, CB_SETCURSEL, keep, 0);
        *self.labels.borrow_mut() = labels;
    }

    unsafe fn update_sort_arrows(&self) {
        let header = send(self.list, LVM_GETHEADER, 0, 0) as HWND;
        let (active, desc) = self.sort.get();
        for (k, col) in COLUMNS.iter().enumerate() {
            let mut item: HDITEMW = std::mem::zeroed();
            item.mask = HDI_FORMAT;
            send(header, HDM_GETITEMW, k, &mut item as *mut HDITEMW as isize);
            item.fmt &= !(HDF_SORTUP | HDF_SORTDOWN);
            if *col == active {
                item.fmt |= if desc { HDF_SORTDOWN } else { HDF_SORTUP };
            }
            send(header, HDM_SETITEMW, k, &item as *const HDITEMW as isize);
        }
    }

    /// Filas seleccionadas (como máximo 100 000).
    unsafe fn selected_rows(&self) -> Vec<usize> {
        let mut rows = Vec::new();
        let mut from = usize::MAX; // -1: desde el principio
        while rows.len() < 100_000 {
            let next = send(self.list, LVM_GETNEXTITEM, from, LVNI_SELECTED as isize);
            if next < 0 {
                break;
            }
            rows.push(next as usize);
            from = next as usize;
        }
        rows
    }

    unsafe fn selected_texts(&self, full_path: bool) -> Vec<String> {
        let rows = self.selected_rows();
        let Some(r) = self.current() else {
            return Vec::new();
        };
        rows.iter()
            .filter_map(|&row| r.hits.get(row))
            .map(|&(s, i)| {
                if full_path {
                    r.indexes[s].path(i)
                } else {
                    r.indexes[s].name(i).to_string()
                }
            })
            .collect()
    }

    unsafe fn open_selected(&self) {
        let Some(path) = self.selected_texts(true).into_iter().next() else {
            return;
        };
        if !open_path(self.hwnd, &path) {
            message_box(
                self.hwnd,
                &format!("No se pudo abrir:\n{path}"),
                MB_ICONWARNING,
            );
        }
    }

    unsafe fn show_folder(&self) {
        if let Some(path) = self.selected_texts(true).into_iter().next() {
            show_in_folder(self.hwnd, &path);
        }
    }

    unsafe fn copy(&self, full_path: bool) {
        let texts = self.selected_texts(full_path);
        if !texts.is_empty() && set_clipboard(self.hwnd, &texts.join("\r\n")) {
            let what = if texts.len() == 1 {
                "Copiado al portapapeles".to_string()
            } else {
                format!(
                    "{} elementos copiados al portapapeles",
                    thousands(texts.len() as u64)
                )
            };
            self.set_status(0, &what);
        }
    }

    unsafe fn select_all(&self) {
        let mut all: LVITEMW = std::mem::zeroed();
        all.stateMask = LVIS_SELECTED;
        all.state = LVIS_SELECTED;
        send(
            self.list,
            LVM_SETITEMSTATE,
            usize::MAX,
            &all as *const LVITEMW as isize,
        );
    }

    unsafe fn context_menu(&self) {
        if self.selected_rows().is_empty() {
            return;
        }
        let menu = CreatePopupMenu();
        let items: [(u16, &str); 7] = [
            (IDM_OPEN, "&Abrir\tEnter"),
            (IDM_FOLDER, "Abrir &carpeta contenedora"),
            (0, ""),
            (IDM_COPY_PATH, "Copiar &ruta completa\tCtrl+C"),
            (IDM_COPY_NAME, "Copiar &nombre"),
            (0, ""),
            (IDM_EXPORT, "&Exportar resultados a Excel…\tCtrl+E"),
        ];
        let texts: Vec<Vec<u16>> = items.iter().map(|(_, t)| wide(t)).collect();
        for ((id, _), text) in items.iter().zip(&texts) {
            if *id == 0 {
                AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
            } else {
                AppendMenuW(menu, MF_STRING, usize::from(*id), text.as_ptr());
            }
        }
        SetMenuDefaultItem(menu, u32::from(IDM_OPEN), 0);
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            self.hwnd,
            ptr::null(),
        );
        DestroyMenu(menu);
        if cmd > 0 {
            self.on_command(cmd as u16, 0);
        }
    }

    unsafe fn export(&self) {
        let Some(r) = self.current().filter(|r| !r.hits.is_empty()) else {
            message_box(
                self.hwnd,
                "No hay resultados para exportar.",
                MB_ICONINFORMATION,
            );
            return;
        };
        let Some(path) = save_csv_dialog(self.hwnd, "resultados-lince.csv") else {
            return;
        };
        match write_csv(&path, &r.indexes, &r.hits) {
            Ok(()) => message_box(
                self.hwnd,
                &format!(
                    "Se exportaron {} resultados a:\n{}",
                    thousands(r.hits.len() as u64),
                    path.display()
                ),
                MB_ICONINFORMATION,
            ),
            Err(e) => message_box(
                self.hwnd,
                &format!("No se pudo guardar {}:\n{e}", path.display()),
                MB_ICONERROR,
            ),
        }
    }
}

/// Flecha abajo en la caja de búsqueda: pasa a la lista con el primer
/// resultado seleccionado.
unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    list: usize,
) -> LRESULT {
    if msg == WM_KEYDOWN && (wp == usize::from(VK_DOWN) || wp == usize::from(VK_NEXT)) {
        let list = list as HWND;
        if send(list, LVM_GETITEMCOUNT, 0, 0) > 0 {
            SetFocus(list);
            let mut first: LVITEMW = std::mem::zeroed();
            first.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
            first.state = LVIS_SELECTED | LVIS_FOCUSED;
            send(list, LVM_SETITEMSTATE, 0, &first as *const LVITEMW as isize);
            send(list, LVM_ENSUREVISIBLE, 0, 0);
            return 0;
        }
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}
