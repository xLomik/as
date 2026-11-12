//! La caja de Lince dentro de la barra de tareas (Windows 10): copiar la
//! DLL a una carpeta propia, registrarla solo para este usuario y pedir al
//! Explorador que la muestre. Windows 11 ya no admite estas barras.

use std::ffi::c_void;
use std::io;
use std::path::{Path, PathBuf};
use std::ptr;

use lince_protocolo::{BAND_CLSID, BAND_CLSID_U128, EXE_VALUE, SETTINGS_KEY};
use windows_sys::core::{GUID, HRESULT};
use windows_sys::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows_sys::Win32::UI::Shell::TrayDeskBand;

use super::registry::{delete_tree, get_machine_string, set_string};

/// Nombre de la DLL junto a Lince.exe.
pub const DLL_NAME: &str = "lince_barra.dll";
const CLSID: GUID = GUID::from_u128(BAND_CLSID_U128);
const IID_ITRAYDESKBAND: GUID = GUID::from_u128(0x6D67E846_5B9C_4DB8_9CBC_DDE12F4254F1);
/// Barras de herramientas de la barra de tareas (y su variante vertical).
const CATEGORIES: [&str; 2] = [
    "{00021492-0000-0000-C000-000000000046}",
    "{00021493-0000-0000-C000-000000000046}",
];

fn clsid_key() -> String {
    format!(r"Software\Classes\CLSID\{BAND_CLSID}")
}

/// ¿Esta versión de Windows admite barras en la barra de tareas?
/// (Windows 11 es la compilación 22000 o posterior.)
pub fn supported() -> bool {
    get_machine_string(
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "CurrentBuildNumber",
    )
    .and_then(|b| b.trim().parse::<u32>().ok())
    .is_some_and(|build| build < 22000)
}

/// La DLL que viene junto a Lince.exe.
pub fn bundled_dll(exe: &Path) -> PathBuf {
    exe.with_file_name(DLL_NAME)
}

/// `ITrayDeskBand` del Explorador, llamado por su tabla de funciones.
struct Tray(*mut c_void);

#[repr(C)]
struct TrayVtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    show: unsafe extern "system" fn(*mut c_void, *const GUID) -> HRESULT,
    hide: unsafe extern "system" fn(*mut c_void, *const GUID) -> HRESULT,
    is_shown: unsafe extern "system" fn(*mut c_void, *const GUID) -> HRESULT,
    changed: unsafe extern "system" fn(*mut c_void) -> HRESULT,
}

impl Tray {
    fn new() -> io::Result<Tray> {
        let mut p = ptr::null_mut();
        // SAFETY: COM ya está iniciado en este hilo (ver `run`).
        let hr = unsafe {
            CoCreateInstance(
                &TrayDeskBand,
                ptr::null_mut(),
                CLSCTX_ALL,
                &IID_ITRAYDESKBAND,
                &mut p,
            )
        };
        if hr < 0 || p.is_null() {
            return Err(io::Error::other(format!(
                "la barra de tareas no responde (0x{hr:08X})"
            )));
        }
        Ok(Tray(p))
    }

    fn vtbl(&self) -> &TrayVtbl {
        // SAFETY: un objeto COM empieza por el puntero a su tabla.
        unsafe { &**(self.0 as *const *const TrayVtbl) }
    }

    fn show(&self) -> HRESULT {
        // SAFETY: objeto vivo y CLSID válido.
        unsafe { (self.vtbl().show)(self.0, &CLSID) }
    }

    fn hide(&self) -> HRESULT {
        // SAFETY: objeto vivo y CLSID válido.
        unsafe { (self.vtbl().hide)(self.0, &CLSID) }
    }

    fn is_shown(&self) -> bool {
        // SAFETY: objeto vivo y CLSID válido. S_OK = se ve; S_FALSE = no.
        unsafe { (self.vtbl().is_shown)(self.0, &CLSID) == 0 }
    }

    /// El Explorador vuelve a leer qué barras hay registradas.
    fn registration_changed(&self) {
        // SAFETY: objeto vivo.
        unsafe { (self.vtbl().changed)(self.0) };
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: referencia propia.
        unsafe { (self.vtbl().release)(self.0) };
    }
}

pub fn is_shown() -> bool {
    Tray::new().is_ok_and(|t| t.is_shown())
}

/// Registra la caja (copiando la DLL que viene con Lince) y pide mostrarla.
/// `Ok(false)`: registrada, pero el Explorador no la mostró solo.
pub fn install(exe: &Path) -> io::Result<bool> {
    let dll = install_dll(&bundled_dll(exe))?;
    let key = clsid_key();
    // El texto por defecto es el nombre en «Barras de herramientas».
    set_string(&key, None, "Lince")?;
    let inproc = format!(r"{key}\InprocServer32");
    set_string(&inproc, None, &dll.to_string_lossy())?;
    set_string(&inproc, Some("ThreadingModel"), "Apartment")?;
    set_string(
        &format!(r"{key}\Implemented Categories\{}", CATEGORIES[0]),
        None,
        "",
    )?;
    set_string(SETTINGS_KEY, Some(EXE_VALUE), &exe.to_string_lossy())?;
    forget_cached_bands();
    let tray = Tray::new()?;
    tray.registration_changed();
    Ok(tray.show() >= 0 && tray.is_shown())
}

pub fn uninstall() -> io::Result<()> {
    if let Ok(tray) = Tray::new() {
        tray.hide();
    }
    delete_tree(&clsid_key())?;
    forget_cached_bands();
    if let Ok(tray) = Tray::new() {
        tray.registration_changed();
    }
    Ok(())
}

/// El Explorador guarda la lista de barras: se borra para que vea la nueva.
fn forget_cached_bands() {
    let base = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Discardable\PostSetup";
    for dir in ["Component Categories", "Component Categories64"] {
        for cat in CATEGORIES {
            let _ = delete_tree(&format!(r"{base}\{dir}\{cat}"));
        }
    }
}

/// Copia la DLL a `%LOCALAPPDATA%\Lince\barra` con un nombre según su
/// contenido: la que esté cargando el Explorador no se puede reemplazar,
/// pero una versión nueva va a otro archivo. Las viejas se borran si se
/// puede.
fn install_dll(src: &Path) -> io::Result<PathBuf> {
    let bytes = std::fs::read(src).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("falta {DLL_NAME} junto a Lince.exe ({})", src.display()),
        )
    })?;
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("no se encontró LOCALAPPDATA"))?;
    let dir = base.join("Lince").join("barra");
    std::fs::create_dir_all(&dir)?;
    let name = format!("lince_barra-{:016x}.dll", fnv64(&bytes));
    let dst = dir.join(&name);
    if !dst.exists() {
        let tmp = dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &dst)?;
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("lince_barra-") && n != name {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(dst)
}

/// FNV-1a de 64 bits: para nombrar la copia según su contenido.
fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}
