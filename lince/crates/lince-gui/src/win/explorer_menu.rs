//! Entrada «Buscar aquí con Lince» del clic derecho del Explorador. Se
//! escribe en el registro del usuario (HKCU), que no pide permisos de
//! administrador ni afecta a otros usuarios del equipo.

use std::io;
use std::path::{Path, PathBuf};
use std::ptr;

use lince_gui::{command_exe, is_within, menu_command, MENU_ENTRIES};
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegGetValueW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
};
use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

use super::util::wide;

/// Programa al que apunta la entrada, si está instalada.
pub fn installed_exe() -> Option<PathBuf> {
    read_default(&format!(r"{}\command", MENU_ENTRIES[0].key)).and_then(|c| command_exe(&c))
}

pub fn install(exe: &Path) -> io::Result<()> {
    for e in &MENU_ENTRIES {
        set_value(e.key, Some("MUIVerb"), e.text)?;
        set_value(e.key, Some("Icon"), &format!("{},0", exe.display()))?;
        set_value(
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
        let key = wide(e.key);
        // SAFETY: clave terminada en 0 bajo HKCU.
        let r = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
        if r != ERROR_FILE_NOT_FOUND {
            check(r)?;
        }
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

fn set_value(key: &str, name: Option<&str>, value: &str) -> io::Result<()> {
    let key = wide(key);
    let name = name.map(wide);
    let value = wide(value);
    let mut h: HKEY = ptr::null_mut();
    // SAFETY: textos terminados en 0; la clave abierta se cierra siempre.
    unsafe {
        check(RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            0,
            ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            ptr::null(),
            &mut h,
            ptr::null_mut(),
        ))?;
        let r = RegSetValueExW(
            h,
            name.as_ref().map_or(ptr::null(), |n| n.as_ptr()),
            0,
            REG_SZ,
            value.as_ptr().cast(),
            (value.len() * 2) as u32,
        );
        RegCloseKey(h);
        check(r)
    }
}

/// Valor por defecto de una clave de HKCU.
fn read_default(key: &str) -> Option<String> {
    let key = wide(key);
    let mut buf = vec![0u16; 4096];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: el tamaño indicado es el del buffer.
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            ptr::null(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if r != ERROR_SUCCESS {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..len]))
}

fn check(r: WIN32_ERROR) -> io::Result<()> {
    if r == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(r as i32))
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
