//! Barra rápida: una caja para escribir que aparece abajo a la derecha,
//! sobre la barra de tareas (clic en el icono de Lince junto al reloj o
//! Ctrl+Alt+Espacio). Enter abre el resultado elegido, Ctrl+Enter su
//! carpeta y Esc (o hacer clic fuera) la esconde.
//!
//! La misma ventana muestra los resultados de la caja que vive dentro de la
//! barra de tareas: entonces aparece justo encima de ella, sin quitarle el
//! foco, solo con la lista; las teclas llegan desde la caja y lo que hay
//! que abrir se le devuelve a la caja (que lo abre como usuario normal).

use lince_protocolo::{payload_text, text_payload, Command, Key, BAND_CLASS};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MonitorFromRect, COLOR_WINDOW, HBRUSH, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::UI::Controls::{
    EMF_CENTERED, LVM_SETCOLUMNWIDTH, LVNI_FOCUSED, LVN_GETEMPTYMARKUP, LVS_NOSORTHEADER,
    LVS_SINGLESEL, NMLVEMPTYMARKUP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindow, IsWindowVisible, LoadCursorW, RegisterClassExW,
    SendMessageTimeoutW, SetForegroundWindow, SetWindowLongPtrW as SetLong, ShowWindow,
    GWL_EXSTYLE, GW_OWNER, HWND_TOPMOST, IDC_ARROW, SMTO_ABORTIFHUNG, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, WA_INACTIVE, WM_ACTIVATE, WM_COPYDATA, WNDCLASSEXW,
    WS_BORDER, WS_EX_CONTROLPARENT, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use std::time::{Duration, Instant};

use super::*;

const BAR_CLASS: PCWSTR = w!("LinceBarraRapida");
const ID_BAR_EDIT: u16 = 301;
const ID_BAR_LIST: u16 = 302;
const ID_BAR_FULL: u16 = 303;
const TIMER_BAR: usize = 2;
const BAR_DEBOUNCE_MS: u32 = 60;
const BAR_CUE: &str = "Buscar archivos y carpetas…";
/// Columnas de la barra (las de la ventana, sin tamaño ni fecha).
const BAR_COLUMNS: [Column; 2] = [Column::Name, Column::Folder];

pub(super) struct Bar {
    pub(super) hwnd: HWND,
    /// Cuándo se escondió por hacer clic fuera: un clic en el icono la
    /// esconde así y no debe volver a abrirla en el mismo clic.
    hidden_at: Cell<Option<Instant>>,
    pub(super) edit: HWND,
    pub(super) list: HWND,
    pub(super) full: HWND,
    generation: Cell<u64>,
    results: RefCell<Option<Rc<Results>>>,
    /// La caja de la barra de tareas a la que sirve ahora (nula: barra
    /// rápida flotante con su propia caja).
    band: Cell<HWND>,
}

impl Bar {
    /// Crea la barra (escondida), propiedad de la ventana principal.
    pub(super) unsafe fn create(owner: HWND) -> Bar {
        let hinst = GetModuleHandleW(ptr::null());
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(bar_proc),
            hInstance: hinst,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as usize as HBRUSH,
            lpszClassName: BAR_CLASS,
            ..std::mem::zeroed()
        };
        RegisterClassExW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_CONTROLPARENT,
            BAR_CLASS,
            w!("Lince"),
            WS_POPUP | WS_BORDER,
            0,
            0,
            0,
            0,
            owner,
            ptr::null_mut(),
            hinst,
            ptr::null(),
        );
        let edit = create_control(
            hwnd,
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            "",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            ID_BAR_EDIT,
        );
        let full = create_control(
            hwnd,
            0,
            w!("BUTTON"),
            "Abrir en Lince",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
            ID_BAR_FULL,
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
                | LVS_SINGLESEL
                | LVS_NOSORTHEADER
                | LVS_SHOWSELALWAYS
                | LVS_SHAREIMAGELISTS,
            ID_BAR_LIST,
        );
        let cue = wide(BAR_CUE);
        send(edit, EM_SETCUEBANNER, 1, cue.as_ptr() as isize);
        SetWindowSubclass(edit, Some(edit_proc), 1, list as usize);
        send(
            list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize,
        );
        SetWindowTheme(list, w!("Explorer"), ptr::null());
        for (k, col) in BAR_COLUMNS.iter().enumerate() {
            let mut title = wide(col.title());
            let mut c: LVCOLUMNW = std::mem::zeroed();
            c.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_FMT | LVCF_SUBITEM;
            c.fmt = LVCFMT_LEFT;
            c.cx = 200;
            c.pszText = title.as_mut_ptr();
            c.iSubItem = k as i32;
            send(list, LVM_INSERTCOLUMNW, k, &c as *const LVCOLUMNW as isize);
        }
        Bar {
            hwnd,
            hidden_at: Cell::new(None),
            edit,
            list,
            full,
            generation: Cell::new(0),
            results: RefCell::new(None),
            band: Cell::new(ptr::null_mut()),
        }
    }
}

/// Los mensajes de la barra los atiende la ventana principal (dueña).
unsafe extern "system" fn bar_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let owner = GetWindow(hwnd, GW_OWNER);
    let app = if owner.is_null() {
        ptr::null()
    } else {
        GetWindowLongPtrW(owner, GWLP_USERDATA) as *const App
    };
    if !app.is_null() {
        if let Some(r) = (*app).bar_message(msg, wp, lp) {
            return r;
        }
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

impl App {
    pub(super) unsafe fn bar_message(&self, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_SIZE => {
                self.bar_layout();
                Some(0)
            }
            // Clic fuera (o se abrió lo encontrado): se esconde.
            WM_ACTIVATE if u32::from(loword(wp)) == WA_INACTIVE => {
                if IsWindowVisible(self.bar.hwnd) != 0 {
                    self.bar.hidden_at.set(Some(Instant::now()));
                }
                self.hide_bar();
                Some(0)
            }
            WM_TIMER if wp == TIMER_BAR => {
                KillTimer(self.bar.hwnd, TIMER_BAR);
                self.bar_search();
                Some(0)
            }
            WM_COMMAND => {
                match (loword(wp), hiword(wp)) {
                    (ID_BAR_EDIT, code) if u32::from(code) == EN_CHANGE => {
                        SetTimer(self.bar.hwnd, TIMER_BAR, BAR_DEBOUNCE_MS, None);
                    }
                    (ID_BAR_FULL, _) => self.bar_to_window(),
                    (id, _) if i32::from(id) == IDOK => {
                        self.bar_open(GetKeyState(i32::from(VK_CONTROL)) < 0);
                    }
                    (id, _) if i32::from(id) == IDCANCEL => self.hide_bar(),
                    _ => {}
                }
                Some(0)
            }
            WM_NOTIFY => {
                let hdr = &*(lp as *const NMHDR);
                if hdr.hwndFrom != self.bar.list {
                    return None;
                }
                match hdr.code {
                    LVN_GETDISPINFOW => {
                        self.bar_fill_item(&mut *(lp as *mut NMLVDISPINFOW));
                        Some(0)
                    }
                    NM_DBLCLK => {
                        self.bar_open(false);
                        Some(0)
                    }
                    LVN_GETEMPTYMARKUP => {
                        let m = &mut *(lp as *mut NMLVEMPTYMARKUP);
                        m.dwFlags = EMF_CENTERED;
                        let text = if window_text(self.bar.edit).trim().is_empty() {
                            "Escriba lo que busca.\nEnter abre · Ctrl+Enter abre la carpeta · \
                             Esc cierra"
                        } else {
                            "Sin resultados."
                        };
                        let t: Vec<u16> = text.encode_utf16().collect();
                        let n = t.len().min(m.szMarkup.len() - 1);
                        m.szMarkup[..n].copy_from_slice(&t[..n]);
                        m.szMarkup[n] = 0;
                        Some(1)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Abajo a la derecha de la pantalla donde está el ratón, sobre la barra
    /// de tareas.
    pub(super) unsafe fn show_bar(&self) {
        self.set_band(ptr::null_mut());
        let (w, h) = (self.scale(720), self.scale(460));
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(MonitorFromPoint(pt, MONITOR_DEFAULTTOPRIMARY), &mut mi);
        let gap = self.scale(10);
        let x = (mi.rcWork.right - w - gap).max(mi.rcWork.left);
        let y = (mi.rcWork.bottom - h - gap).max(mi.rcWork.top);
        SetWindowPos(self.bar.hwnd, HWND_TOPMOST, x, y, w, h, SWP_SHOWWINDOW);
        SetForegroundWindow(self.bar.hwnd);
        SetFocus(self.bar.edit);
        send(self.bar.edit, EM_SETSEL, 0, -1);
        self.maybe_refresh();
    }

    pub(super) unsafe fn hide_bar(&self) {
        ShowWindow(self.bar.hwnd, SW_HIDE);
    }

    pub(super) unsafe fn toggle_bar(&self) {
        let open = IsWindowVisible(self.bar.hwnd) != 0 && GetForegroundWindow() == self.bar.hwnd;
        let just_closed = self
            .bar
            .hidden_at
            .get()
            .is_some_and(|t| t.elapsed() < Duration::from_millis(400));
        if just_closed {
            return;
        }
        if open {
            self.hide_bar();
        } else {
            self.show_bar();
        }
    }

    unsafe fn bar_layout(&self) {
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(self.bar.hwnd, &mut rc);
        let (w, h) = (rc.right, rc.bottom);
        let pad = self.scale(6);
        if !self.bar.band.get().is_null() {
            // Encima de la caja de la barra de tareas: solo la lista.
            let list_w = w - 2 * pad;
            MoveWindow(self.bar.list, pad, pad, list_w, (h - 2 * pad).max(0), 1);
            self.bar_columns(list_w);
            return;
        }
        let eh = self.scale(32);
        let btn_w = self.scale(130);
        let ey = h - pad - eh;
        MoveWindow(self.bar.edit, pad, ey, (w - 3 * pad - btn_w).max(0), eh, 1);
        MoveWindow(self.bar.full, w - pad - btn_w, ey, btn_w, eh, 1);
        let list_w = w - 2 * pad;
        MoveWindow(self.bar.list, pad, pad, list_w, (ey - 2 * pad).max(0), 1);
        self.bar_columns(list_w);
    }

    unsafe fn bar_columns(&self, list_w: i32) {
        let name_w = self.scale(250);
        send(self.bar.list, LVM_SETCOLUMNWIDTH, 0, name_w as isize);
        let rest = (list_w - name_w - self.scale(24)).max(self.scale(100));
        send(self.bar.list, LVM_SETCOLUMNWIDTH, 1, rest as isize);
    }

    /// Lo más reciente primero: lo que se busca rápido suele ser lo último
    /// en que se trabajó.
    unsafe fn bar_search(&self) {
        self.bar_search_text(&window_text(self.bar.edit));
    }

    unsafe fn bar_search_text(&self, text: &str) {
        let generation = self.bar.generation.get() + 1;
        self.bar.generation.set(generation);
        if text.trim().is_empty() {
            *self.bar.results.borrow_mut() = None;
            send(self.bar.list, LVM_SETITEMCOUNT, 0, 0);
            InvalidateRect(self.bar.list, ptr::null(), 1);
            return;
        }
        self.engine.search_from(
            Client::Bar,
            generation,
            text,
            "",
            SortKey::Modified,
            true,
            None,
        );
    }

    pub(super) unsafe fn bar_results(&self, r: Results) {
        // Llega tarde: ya se escribió otra cosa (o se borró la caja).
        if r.generation != self.bar.generation.get() {
            return;
        }
        let n = r.hits.len();
        *self.bar.results.borrow_mut() = Some(Rc::new(r));
        send(self.bar.list, LVM_SETITEMCOUNT, n, 0);
        if n > 0 {
            // El primero queda elegido: Enter lo abre.
            let mut first: LVITEMW = std::mem::zeroed();
            first.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
            first.state = LVIS_SELECTED | LVIS_FOCUSED;
            send(
                self.bar.list,
                LVM_SETITEMSTATE,
                0,
                &first as *const LVITEMW as isize,
            );
            send(self.bar.list, LVM_ENSUREVISIBLE, 0, 0);
        }
        InvalidateRect(self.bar.list, ptr::null(), 1);
    }

    unsafe fn bar_fill_item(&self, info: &mut NMLVDISPINFOW) {
        let Some(r) = self.bar.results.borrow().clone() else {
            return;
        };
        let item = &mut info.item;
        let Some(&hit) = r.hits.get(item.iItem as usize) else {
            return;
        };
        if item.mask & LVIF_TEXT != 0 && !item.pszText.is_null() && item.cchTextMax > 0 {
            let col = BAR_COLUMNS
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

    /// Abre el resultado elegido (o el primero); con `folder`, su carpeta.
    unsafe fn bar_open(&self, folder: bool) {
        let Some(r) = self.bar.results.borrow().clone() else {
            return;
        };
        let row = send(
            self.bar.list,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_FOCUSED as isize,
        );
        let Some(&(src, i)) = r.hits.get(row.max(0) as usize) else {
            return;
        };
        let path = r.indexes[src].path(i);
        self.hide_bar();
        let band = self.bar.band.get();
        if !band.is_null() {
            let cmd = if folder {
                Command::Reveal
            } else {
                Command::Open
            };
            self.send_to_band(band, cmd, &path);
            return;
        }
        let opened = if folder {
            show_in_folder(self.hwnd, &path)
        } else {
            open_path(self.hwnd, &path)
        };
        if !opened {
            message_box(
                ptr::null_mut(),
                &format!("No se pudo abrir:\n{path}"),
                MB_ICONWARNING,
            );
        }
    }

    /// Floating (`band` nula) o encima de la caja de la barra de tareas.
    unsafe fn set_band(&self, band: HWND) {
        let was = self.bar.band.replace(band);
        if was.is_null() == band.is_null() {
            return;
        }
        let floating = band.is_null();
        let how = if floating { SW_SHOW } else { SW_HIDE };
        ShowWindow(self.bar.edit, how);
        ShowWindow(self.bar.full, how);
        // Encima de la caja nunca se activa: el foco sigue en la caja.
        let mut ex = WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_CONTROLPARENT;
        if !floating {
            ex |= WS_EX_NOACTIVATE;
        }
        SetLong(self.bar.hwnd, GWL_EXSTYLE, ex as isize);
        self.bar_layout();
    }

    /// Orden de la caja de la barra de tareas (`band` es su ventana).
    /// `false` si no viene de la caja.
    pub(super) unsafe fn band_message(&self, band: HWND, data: &COPYDATASTRUCT) -> bool {
        if !is_band_window(band) {
            return false;
        }
        let bytes = if data.lpData.is_null() {
            &[][..]
        } else {
            std::slice::from_raw_parts(data.lpData as *const u8, data.cbData as usize)
        };
        match Command::from_id(data.dwData) {
            Some(Command::Text) => {
                let text = payload_text(bytes);
                self.set_band(band);
                self.bar_search_text(&text);
                if text.trim().is_empty() {
                    self.hide_bar();
                } else {
                    self.show_over_band(band);
                }
            }
            Some(Command::Key) => match Key::from_payload(bytes) {
                Some(Key::Enter) => self.bar_open(false),
                Some(Key::CtrlEnter) => self.bar_open(true),
                Some(Key::Escape) => self.hide_bar(),
                Some(k) => self.band_move(k),
                None => return false,
            },
            Some(Command::Hide) => {
                if !self.bar.band.get().is_null() {
                    self.hide_bar();
                }
            }
            _ => return false,
        }
        true
    }

    /// Justo encima de la caja (o debajo si la barra de tareas está arriba,
    /// al lado si está a un costado), sin activarse.
    unsafe fn show_over_band(&self, band: HWND) {
        let mut r: RECT = std::mem::zeroed();
        GetWindowRect(band, &mut r);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST), &mut mi);
        let (work, screen) = (mi.rcWork, mi.rcMonitor);
        let w = (r.right - r.left).max(self.scale(640));
        let h = self.scale(420);
        let gap = self.scale(4);
        let (x, y) = if r.top >= work.bottom - gap || r.top > (screen.top + screen.bottom) / 2 {
            (r.left, r.top - h - gap) // barra abajo (la normal)
        } else if r.bottom <= work.top + gap || r.right - r.left > r.bottom - r.top {
            (r.left, r.bottom + gap) // barra arriba
        } else if r.right <= work.left + gap {
            (r.right + gap, r.top) // barra a la izquierda
        } else {
            (r.left - w - gap, r.top) // barra a la derecha
        };
        let x = x.clamp(screen.left, (screen.right - w).max(screen.left));
        let y = y.clamp(screen.top, (screen.bottom - h).max(screen.top));
        SetWindowPos(
            self.bar.hwnd,
            HWND_TOPMOST,
            x,
            y,
            w,
            h,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
        );
        self.maybe_refresh();
    }

    /// Flechas y Re Pág/Av Pág de la caja mueven la elección en la lista.
    unsafe fn band_move(&self, key: Key) {
        let n = send(self.bar.list, LVM_GETITEMCOUNT, 0, 0);
        if n <= 0 {
            return;
        }
        let cur = send(
            self.bar.list,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_FOCUSED as isize,
        );
        let next = match key {
            Key::Down => cur + 1,
            Key::Up => cur - 1,
            Key::PageDown => cur + 10,
            Key::PageUp => cur - 10,
            _ => cur,
        }
        .clamp(0, n - 1);
        let mut item: LVITEMW = std::mem::zeroed();
        item.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
        item.state = LVIS_SELECTED | LVIS_FOCUSED;
        send(
            self.bar.list,
            LVM_SETITEMSTATE,
            next as usize,
            &item as *const LVITEMW as isize,
        );
        send(self.bar.list, LVM_ENSUREVISIBLE, next as usize, 0);
    }

    /// Le pide a la caja que abra `path` (con tiempo límite).
    unsafe fn send_to_band(&self, band: HWND, cmd: Command, path: &str) {
        let payload = text_payload(path);
        let data = COPYDATASTRUCT {
            dwData: cmd.id(),
            cbData: payload.len() as u32,
            lpData: payload.as_ptr() as *mut c_void,
        };
        let mut result = 0;
        SendMessageTimeoutW(
            band,
            WM_COPYDATA,
            self.hwnd as usize,
            &data as *const COPYDATASTRUCT as isize,
            SMTO_ABORTIFHUNG,
            2000,
            &mut result,
        );
    }

    /// Pasa lo escrito a la ventana completa (filtros, contenido, exportar).
    unsafe fn bar_to_window(&self) {
        let text = wide(&window_text(self.bar.edit));
        SetWindowTextW(self.edit, text.as_ptr());
        self.hide_bar();
        self.show_window();
    }
}

/// ¿Es la ventana de la caja de Lince en la barra de tareas?
unsafe fn is_band_window(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }
    let mut buf = [0u16; 64];
    let n = GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    String::from_utf16_lossy(&buf[..n.max(0) as usize]) == BAND_CLASS
}
