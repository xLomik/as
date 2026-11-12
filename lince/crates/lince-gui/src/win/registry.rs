//! Lectura y escritura de textos en el registro del usuario (HKCU), y
//! lectura de la configuración del equipo (HKLM).

use std::io;
use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegDeleteTreeW, RegGetValueW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    RRF_RT_REG_SZ,
};

use super::util::wide;

/// Crea la clave si hace falta. `name: None` es el valor por defecto.
pub fn set_string(key: &str, name: Option<&str>, value: &str) -> io::Result<()> {
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

/// Un valor de texto (`name: None`: el valor por defecto).
pub fn get_string(key: &str, name: Option<&str>) -> Option<String> {
    get(HKEY_CURRENT_USER, key, name)
}

/// Un valor de texto de la configuración del equipo (HKLM).
pub fn get_machine_string(key: &str, name: &str) -> Option<String> {
    get(HKEY_LOCAL_MACHINE, key, Some(name))
}

fn get(root: HKEY, key: &str, name: Option<&str>) -> Option<String> {
    let key = wide(key);
    let name = name.map(wide);
    let mut buf = vec![0u16; 4096];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: el tamaño indicado es el del buffer.
    let r = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            name.as_ref().map_or(ptr::null(), |n| n.as_ptr()),
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

/// Borra la clave con todo lo que tenga; no existir no es un error.
pub fn delete_tree(key: &str) -> io::Result<()> {
    let key = wide(key);
    // SAFETY: clave terminada en 0 bajo HKCU.
    let r = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
    if r == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    check(r)
}

/// Borra un valor; no existir no es un error.
pub fn delete_value(key: &str, name: &str) -> io::Result<()> {
    let (key, name) = (wide(key), wide(name));
    // SAFETY: textos terminados en 0 bajo HKCU.
    let r = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
    if r == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    check(r)
}
