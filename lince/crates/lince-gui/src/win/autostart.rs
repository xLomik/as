//! Iniciar Lince al encender el equipo, con el Programador de tareas
//! (`schtasks`): la tarea lo abre escondido junto al reloj al iniciar
//! sesión, con permisos de administrador y sin preguntar.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lince_gui::{task_xml, TASK_NAME};

use super::explorer_menu::is_temporary;
use super::registry::{delete_value, get_string, set_string};

/// Dónde se anota qué Lince.exe abre la tarea (para corregirla si se mueve,
/// y para que la caja de la barra de tareas sepa que puede usarla).
use lince_protocolo::{SETTINGS_KEY as KEY, STARTUP_EXE_VALUE as VALUE};
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `schtasks.exe` de System32 (nunca uno que esté en el PATH).
fn schtasks(args: &[&str]) -> io::Result<Output> {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    Command::new(PathBuf::from(root).join("System32").join("schtasks.exe"))
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

fn failure(out: &Output) -> io::Error {
    let text = String::from_utf8_lossy(if out.stderr.is_empty() {
        &out.stdout
    } else {
        &out.stderr
    });
    io::Error::other(text.trim().to_string())
}

pub fn is_installed() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME]).is_ok_and(|o| o.status.success())
}

/// Registra la tarea (necesita permisos de administrador).
pub fn install(exe: &Path) -> io::Result<()> {
    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(d), Ok(u)) if !d.is_empty() => format!(r"{d}\{u}"),
        (_, Ok(u)) => u,
        _ => return Err(io::Error::other("no se sabe qué usuario inició sesión")),
    };
    let mut bytes = vec![0xFF, 0xFE]; // UTF-16 con BOM, como lo pide schtasks
    bytes.extend(
        task_xml(exe, &user)
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    // En la carpeta temporal de Windows, no en la del usuario: un archivo
    // que crea ahí un proceso con permisos de administrador solo lo pueden
    // cambiar los administradores, así ningún programa sin permisos puede
    // cambiar qué se ejecutará. (Dejarlo abierto sin compartir no sirve:
    // schtasks no podría leerlo.)
    let root = std::env::var_os("SystemRoot").map(PathBuf::from);
    let dir = root
        .map(|r| r.join("Temp"))
        .filter(|d| d.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let file = dir.join(format!("lince-inicio-{}-{nanos}.xml", std::process::id()));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)?
        .write_all(&bytes)?;
    let path = file.to_string_lossy().into_owned();
    let out = schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &path, "/F"]);
    let _ = std::fs::remove_file(&file);
    let out = out?;
    if !out.status.success() {
        return Err(failure(&out));
    }
    set_string(KEY, Some(VALUE), &exe.to_string_lossy())?;
    Ok(())
}

pub fn uninstall() -> io::Result<()> {
    if !is_installed() {
        return Ok(());
    }
    let out = schtasks(&["/Delete", "/TN", TASK_NAME, "/F"])?;
    if !out.status.success() {
        return Err(failure(&out));
    }
    delete_value(KEY, VALUE)
}

/// Si Lince se movió de carpeta, la tarea abriría un programa que ya no
/// está: se apunta a esta copia.
pub fn repair(exe: &Path) {
    let Some(old) = get_string(KEY, Some(VALUE)).map(PathBuf::from) else {
        return;
    };
    if !old.exists() && !is_temporary(exe) && is_installed() {
        let _ = install(exe);
    }
}
