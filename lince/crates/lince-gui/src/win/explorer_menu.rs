//! Entrada «Buscar aquí con Lince» del clic derecho del Explorador. Se
//! escribe en el registro del usuario (HKCU), que no pide permisos de
//! administrador ni afecta a otros usuarios del equipo.

use std::io;
use std::path::{Path, PathBuf};
use std::ptr;

use lince_gui::{command_exe, is_within, menu_command, MENU_ENTRIES};
use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

use super::registry::{delete_tree, get_string, set_string};

/// Programa al que apunta la entrada, si está instalada.
pub fn installed_exe() -> Option<PathBuf> {
    get_string(&format!(r"{}\command", MENU_ENTRIES[0].key), None).and_then(|c| command_exe(&c))
}

pub fn install(exe: &Path) -> io::Result<()> {
    for e in &MENU_ENTRIES {
        set_string(e.key, Some("MUIVerb"), e.text)?;
        set_string(e.key, Some("Icon"), &format!("{},0", exe.display()))?;
        set_string(
            &format!(r"{}\command", e.key),
            None,
            &menu_command(exe, e.placeholder),
        )?;
    }
    notify_explorer();
    Ok(())
}

pub fn uninstall() -> io::Result<()> {
    for e in &MENU_ENTRIES {
        delete_tree(e.key)?;
    }
    notify_explorer();
    Ok(())
}

/// ¿`exe` está en la carpeta temporal? Pasa al abrir Lince.exe sin sacarlo
/// del .zip: esa copia desaparece y la entrada dejaría de funcionar.
pub fn is_temporary(exe: &Path) -> bool {
    is_within(exe, &std::env::temp_dir())
}

/// Si Lince se movió de carpeta, la entrada apunta a un programa que ya no
/// está: se apunta a esta copia.
pub fn repair(exe: &Path) {
    if let Some(old) = installed_exe() {
        if !old.exists() && !is_temporary(exe) {
            let _ = install(exe);
        }
    }
}

/// El Explorador vuelve a leer los menús sin reiniciar.
fn notify_explorer() {
    // SAFETY: evento global sin datos.
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            ptr::null(),
            ptr::null(),
        )
    };
}
