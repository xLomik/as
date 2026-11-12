//! Lo que se dicen la caja de Lince en la barra de tareas (una DLL que
//! carga el Explorador) y Lince.exe: órdenes por `WM_COPYDATA` y los nombres
//! que ambos deben compartir.
//!
//! La caja manda lo escrito y las teclas; Lince busca y muestra los
//! resultados encima. Para abrir algo, Lince le pide a la caja que lo abra:
//! así los programas se abren desde el Explorador, como usuario normal, y
//! no con los permisos de administrador de Lince.

/// Clase de la ventana de Lince que vive junto al reloj.
pub const MAIN_CLASS: &str = "LinceVentanaPrincipal";
/// Clase de la ventana de la caja dentro de la barra de tareas.
pub const BAND_CLASS: &str = "LinceBandaTareas";
/// CLSID de la caja (barra de herramientas de la barra de tareas).
pub const BAND_CLSID: &str = "{E5F9334C-A814-4B74-87CD-CD761F4F70A2}";
pub const BAND_CLSID_U128: u128 = 0xE5F9334C_A814_4B74_87CD_CD761F4F70A2;
/// Clave (en HKCU) donde Lince anota dónde está Lince.exe, para que la caja
/// pueda abrirlo si no está en marcha.
pub const SETTINGS_KEY: &str = r"Software\Lince";
pub const EXE_VALUE: &str = "Exe";
/// Lince.exe que abre la tarea de inicio (solo existe si la tarea existe).
pub const STARTUP_EXE_VALUE: &str = "InicioExe";
/// Nombre de la tarea de inicio en el Programador de tareas.
pub const TASK_NAME: &str = "Lince";
/// Lo más largo que se manda escrito (más no tiene sentido en la caja).
pub const MAX_TEXT: usize = 260;

/// Prefijo de las órdenes en `dwData`, para no confundirlas con otras.
const TAG: usize = 0x4C4E_0000; // «LN»

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Caja → Lince: lo escrito (texto UTF-16).
    Text,
    /// Caja → Lince: una tecla de las de [`Key`].
    Key,
    /// Caja → Lince: esconder los resultados (se fue de la caja).
    Hide,
    /// Lince → caja: abrir este archivo o carpeta (texto UTF-16).
    Open,
    /// Lince → caja: abrir la carpeta que lo contiene, con él elegido.
    Reveal,
}

impl Command {
    const ALL: [Command; 5] = [
        Command::Text,
        Command::Key,
        Command::Hide,
        Command::Open,
        Command::Reveal,
    ];

    pub const fn id(self) -> usize {
        TAG | (self as usize + 1)
    }

    pub fn from_id(id: usize) -> Option<Command> {
        Command::ALL.into_iter().find(|c| c.id() == id)
    }
}

/// Teclas que la caja pasa a Lince para moverse por los resultados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Down,
    Up,
    PageDown,
    PageUp,
    Enter,
    /// Ctrl+Enter: abrir la carpeta.
    CtrlEnter,
    Escape,
}

impl Key {
    const ALL: [Key; 7] = [
        Key::Down,
        Key::Up,
        Key::PageDown,
        Key::PageUp,
        Key::Enter,
        Key::CtrlEnter,
        Key::Escape,
    ];

    pub fn payload(self) -> [u8; 4] {
        (self as u32 + 1).to_le_bytes()
    }

    pub fn from_payload(bytes: &[u8]) -> Option<Key> {
        let n = u32::from_le_bytes(bytes.try_into().ok()?);
        Key::ALL.into_iter().find(|k| *k as u32 + 1 == n)
    }
}

/// Texto en UTF-16 (sin el 0 final), recortado a [`MAX_TEXT`] caracteres.
pub fn text_payload(text: &str) -> Vec<u8> {
    text.chars()
        .take(MAX_TEXT)
        .collect::<String>()
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect()
}

pub fn payload_text(bytes: &[u8]) -> String {
    let (units, _) = bytes.as_chunks::<2>();
    let units: Vec<u16> = units.iter().map(|&b| u16::from_le_bytes(b)).collect();
    String::from_utf16_lossy(&units)
        .trim_end_matches('\0')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_round_trip_and_are_tagged() {
        for c in Command::ALL {
            assert_eq!(Command::from_id(c.id()), Some(c));
            assert_eq!(c.id() & 0xFFFF_0000, TAG);
        }
        assert_eq!(Command::from_id(0), None);
        assert_eq!(Command::from_id(1), None);
    }

    #[test]
    fn keys_round_trip() {
        for k in Key::ALL {
            assert_eq!(Key::from_payload(&k.payload()), Some(k));
        }
        assert_eq!(Key::from_payload(&[0, 0, 0, 0]), None);
        assert_eq!(Key::from_payload(&[1, 0]), None);
    }

    #[test]
    fn text_round_trips_with_accents_and_is_capped() {
        let s = r"C:\Planos\Señalización camión.dwg";
        assert_eq!(payload_text(&text_payload(s)), s);
        let long = "x".repeat(MAX_TEXT + 50);
        assert_eq!(payload_text(&text_payload(&long)).len(), MAX_TEXT);
        // Un 0 final (C) no ensucia el texto.
        let mut with_nul = text_payload("abc");
        with_nul.extend([0, 0]);
        assert_eq!(payload_text(&with_nul), "abc");
    }

    #[test]
    fn clsid_text_and_number_agree() {
        let n = format!("{BAND_CLSID_U128:032X}");
        let text = format!(
            "{{{}-{}-{}-{}-{}}}",
            &n[..8],
            &n[8..12],
            &n[12..16],
            &n[16..20],
            &n[20..]
        );
        assert_eq!(text, BAND_CLSID);
    }
}
