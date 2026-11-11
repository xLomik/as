//! Lince en segundo plano: el icono junto al reloj, cerrar la ventana la
//! esconde (el índice sigue cargado), un atajo de teclado abre la barra
//! rápida y, si el índice es viejo, se vuelve a leer sin dejar de buscar.

use std::time::{Duration, Instant};

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, VK_SPACE,
};
use windows_sys::Win32::UI::Shell::NIN_SELECT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IsIconic, IsWindowVisible, RegisterWindowMessageW, SetForegroundWindow, ShowWindow, SW_HIDE,
    SW_MINIMIZE, SW_RESTORE, SW_SHOW, TPM_BOTTOMALIGN, WM_CLOSE, WM_CONTEXTMENU, WM_HOTKEY,
    WM_LBUTTONDBLCLK, WM_NULL,
};

use super::super::{instance, tray};
use super::*;

pub(super) const IDM_TRAY_OPEN: u16 = 213;
pub(super) const IDM_TRAY_BAR: u16 = 214;
const HOTKEY_BAR: i32 = 1;
const HOTKEY_TEXT: &str = "Ctrl+Alt+Espacio";
/// Al abrir Lince con un índice más viejo que esto, se vuelve a leer.
const REFRESH_AFTER: Duration = Duration::from_secs(15 * 60);

/// Mensajes registrados (los números los da Windows al arrancar).
pub(super) struct Messages {
    show: u32,
    quit: u32,
    /// El Explorador se reinició: hay que volver a poner el icono.
    taskbar_created: u32,
}

impl Messages {
    pub(super) fn register() -> Messages {
        Messages {
            show: instance::show_message(),
            quit: instance::quit_message(),
            // SAFETY: texto constante terminado en 0.
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        }
    }
}

/// Estado de la copia en segundo plano.
pub(super) struct Background {
    pub(super) enabled: bool,
    pub(super) icon: HICON,
    pub(super) messages: Messages,
    tray: Cell<bool>,
    hotkey: Cell<bool>,
    indexing: Cell<bool>,
    indexed_at: Cell<Option<Instant>>,
}

impl Background {
    pub(super) fn new(enabled: bool, icon: HICON) -> Background {
        Background {
            enabled,
            icon,
            messages: Messages::register(),
            tray: Cell::new(false),
            hotkey: Cell::new(false),
            indexing: Cell::new(false),
            indexed_at: Cell::new(None),
        }
    }
}

impl App {
    pub(super) unsafe fn start_background(&self) {
        if !self.bg.enabled {
            return;
        }
        instance::accept_messages(self.hwnd);
        self.bg
            .tray
            .set(tray::add(self.hwnd, self.bg.icon, "Lince"));
        let ok = RegisterHotKey(
            self.hwnd,
            HOTKEY_BAR,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            u32::from(VK_SPACE),
        );
        self.bg.hotkey.set(ok != 0);
    }

    pub(super) unsafe fn background_message(
        &self,
        msg: u32,
        wp: WPARAM,
        lp: LPARAM,
    ) -> Option<LRESULT> {
        if !self.bg.enabled {
            return None;
        }
        let m = &self.bg.messages;
        match msg {
            WM_CLOSE => self.hide_to_tray(),
            WM_HOTKEY if wp == HOTKEY_BAR as usize => self.toggle_bar(),
            tray::WM_TRAY => self.on_tray(lp),
            x if x == m.show => self.show_window(),
            x if x == m.quit => self.quit(),
            x if x == m.taskbar_created => {
                self.bg
                    .tray
                    .set(tray::add(self.hwnd, self.bg.icon, "Lince"));
            }
            _ => return None,
        }
        Some(0)
    }

    pub(super) unsafe fn background_destroyed(&self) {
        if self.bg.tray.get() {
            tray::remove(self.hwnd);
        }
        if self.bg.hotkey.get() {
            UnregisterHotKey(self.hwnd, HOTKEY_BAR);
        }
    }

    /// Cerrar la ventana la esconde: el índice queda en memoria.
    unsafe fn hide_to_tray(&self) {
        if !self.bg.tray.get() {
            self.bg
                .tray
                .set(tray::add(self.hwnd, self.bg.icon, "Lince"));
        }
        if !self.bg.tray.get() {
            // Sin icono no habría cómo volver: se minimiza.
            ShowWindow(self.hwnd, SW_MINIMIZE);
            return;
        }
        self.hide_bar();
        ShowWindow(self.hwnd, SW_HIDE);
        self.first_time_notice();
    }

    /// La primera vez se explica dónde quedó Lince.
    unsafe fn first_time_notice(&self) {
        let Some(marker) = self
            .folders_file
            .as_ref()
            .map(|f| f.with_file_name("aviso-segundo-plano.txt"))
        else {
            return;
        };
        if marker.exists() {
            return;
        }
        let key = if self.bg.hotkey.get() {
            format!(" o pulse {HOTKEY_TEXT}")
        } else {
            String::new()
        };
        tray::notify(
            self.hwnd,
            "Lince sigue abierto",
            &format!(
                "Está aquí, junto al reloj, con todo listo. Haga clic en el icono{key} \
                 para buscar. Para cerrarlo del todo: clic derecho en el icono y «Salir \
                 de Lince»."
            ),
        );
        if let Some(dir) = marker.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&marker, "Lince ya avisó que queda en segundo plano.\r\n");
    }

    pub(super) unsafe fn show_window(&self) {
        let how = if IsIconic(self.hwnd) != 0 {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        ShowWindow(self.hwnd, how);
        SetForegroundWindow(self.hwnd);
        SetFocus(self.edit);
        self.maybe_refresh();
    }

    pub(super) unsafe fn quit(&self) {
        DestroyWindow(self.hwnd);
    }

    pub(super) fn window_visible(&self) -> bool {
        // SAFETY: ventana propia.
        unsafe { IsWindowVisible(self.hwnd) != 0 }
    }

    unsafe fn on_tray(&self, lp: LPARAM) {
        match u32::from(loword(lp as usize)) {
            NIN_SELECT | tray::NIN_KEYSELECT => self.toggle_bar(),
            WM_LBUTTONDBLCLK => self.show_window(),
            WM_CONTEXTMENU => self.tray_menu(),
            _ => {}
        }
    }

    unsafe fn tray_menu(&self) {
        let menu = CreatePopupMenu();
        let item = |id: u16, text: &str| {
            let t = wide(text);
            if id == 0 {
                AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
            } else {
                AppendMenuW(menu, MF_STRING, usize::from(id), t.as_ptr());
            }
        };
        if self.bg.hotkey.get() {
            item(IDM_TRAY_BAR, &format!("&Buscar…\t{HOTKEY_TEXT}"));
        } else {
            item(IDM_TRAY_BAR, "&Buscar…");
        }
        item(IDM_TRAY_OPEN, "&Abrir Lince");
        item(0, "");
        item(IDM_REFRESH, "&Volver a leer los discos");
        item(0, "");
        item(IDM_EXIT, "&Salir de Lince");
        SetMenuDefaultItem(menu, u32::from(IDM_TRAY_OPEN), 0);
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        // Sin esto el menú no se cierra al hacer clic fuera.
        SetForegroundWindow(self.hwnd);
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            0,
            self.hwnd,
            ptr::null(),
        );
        PostMessageW(self.hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }

    /// Con un índice viejo se vuelve a leer en segundo plano; mientras
    /// tanto se sigue buscando en el actual.
    pub(super) unsafe fn maybe_refresh(&self) {
        if !self.bg.enabled || self.bg.indexing.get() {
            return;
        }
        if self
            .bg
            .indexed_at
            .get()
            .is_some_and(|t| t.elapsed() > REFRESH_AFTER)
        {
            self.engine.reindex(self.current_sources());
        }
    }

    pub(super) fn background_indexing(&self) {
        self.bg.indexing.set(true);
    }

    pub(super) fn background_indexed(&self, entries: usize) {
        self.bg.indexing.set(false);
        self.bg.indexed_at.set(Some(Instant::now()));
        if self.bg.tray.get() {
            tray::set_tip(
                self.hwnd,
                &format!("Lince · {} objetos", thousands(entries as u64)),
            );
        }
    }
}
