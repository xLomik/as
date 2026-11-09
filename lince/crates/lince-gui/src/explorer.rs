//! Entrada «Buscar aquí con Lince» del clic derecho del Explorador de
//! archivos: textos, comando y arreglo de rutas. El registro de Windows se
//! escribe en `win/explorer_menu.rs`.

use std::path::{Path, PathBuf};

/// Opción con la carpeta donde buscar (la que pasa el Explorador).
pub const FOLDER_FLAG: &str = "--carpeta";

/// Una entrada del menú: su clave bajo `HKEY_CURRENT_USER`, el texto que
/// muestra y el marcador con que el Explorador le pasa la ruta.
pub struct MenuEntry {
    pub key: &'static str,
    pub text: &'static str,
    pub placeholder: &'static str,
}

pub const MENU_ENTRIES: [MenuEntry; 3] = [
    // Clic derecho en el fondo de una carpeta abierta.
    MenuEntry {
        key: r"Software\Classes\Directory\Background\shell\Lince",
        text: "Buscar aquí con Lince",
        placeholder: "%V",
    },
    // Clic derecho sobre una carpeta.
    MenuEntry {
        key: r"Software\Classes\Directory\shell\Lince",
        text: "Buscar en esta carpeta con Lince",
        placeholder: "%1",
    },
    // Clic derecho sobre una unidad (C:, Z:...).
    MenuEntry {
        key: r"Software\Classes\Drive\shell\Lince",
        text: "Buscar en esta unidad con Lince",
        placeholder: "%1",
    },
];

/// Lo que ejecuta el Explorador: `"C:\...\Lince.exe" --carpeta "%V"`.
pub fn menu_command(exe: &Path, placeholder: &str) -> String {
    format!("\"{}\" {FOLDER_FLAG} \"{placeholder}\"", exe.display())
}

/// Programa de un comando registrado (lo primero entre comillas).
pub fn command_exe(command: &str) -> Option<PathBuf> {
    let rest = command.trim_start().strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(PathBuf::from(&rest[..end]))
}

/// Ruta tal como llega del Explorador. En la raíz de una unidad llega
/// `"C:\"` y, por las reglas de la línea de comandos de Windows, `\"` es una
/// comilla escrita: el argumento queda `C:"`. Ninguna ruta lleva comillas,
/// así que la comilla final vuelve a ser la barra.
pub fn clean_shell_path(arg: &str) -> PathBuf {
    let s = arg.trim();
    match s.strip_suffix('"') {
        Some(head) => PathBuf::from(format!("{}\\", head.trim_end_matches(['"', '\\']))),
        None => PathBuf::from(s),
    }
}

/// `C:` si `path` es la raíz de una unidad (`C:\` o `C:`).
pub fn drive_root(path: &Path) -> Option<String> {
    let s = path.to_string_lossy();
    let s = s.trim_end_matches(['\\', '/']);
    let b = s.as_bytes();
    (b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':').then(|| s.to_ascii_uppercase())
}

/// ¿`path` es `dir` o está dentro? Sin distinguir mayúsculas ni barra
/// final, como Windows.
pub fn is_within(path: &Path, dir: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_lowercase()
    };
    let (path, dir) = (norm(path), norm(dir));
    path.strip_prefix(&dir)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['\\', '/']))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_args;

    #[test]
    fn the_command_quotes_program_and_folder() {
        let exe = Path::new(r"C:\Program Files\Lince\Lince.exe");
        let cmd = menu_command(exe, "%V");
        assert_eq!(cmd, r#""C:\Program Files\Lince\Lince.exe" --carpeta "%V""#);
        assert_eq!(command_exe(&cmd).as_deref(), Some(exe));
        assert_eq!(command_exe("sin comillas"), None);
    }

    #[test]
    fn every_entry_is_under_the_user_classes() {
        for e in &MENU_ENTRIES {
            assert!(e.key.starts_with(r"Software\Classes\"), "{}", e.key);
            assert!(e.key.ends_with(r"\shell\Lince"), "{}", e.key);
            assert!(e.placeholder == "%V" || e.placeholder == "%1");
        }
    }

    /// Lo que de verdad llega a Lince desde el Explorador (ya separado en
    /// argumentos por Windows).
    #[test]
    fn explorer_paths_are_repaired() {
        assert_eq!(clean_shell_path(r#"C:""#), PathBuf::from(r"C:\"));
        assert_eq!(clean_shell_path(r#"z:""#), PathBuf::from(r"z:\"));
        assert_eq!(
            clean_shell_path(r"C:\Users\Ana\Mis Documentos"),
            PathBuf::from(r"C:\Users\Ana\Mis Documentos")
        );
        assert_eq!(
            clean_shell_path(r"\\srv\datos\Planos 2026"),
            PathBuf::from(r"\\srv\datos\Planos 2026")
        );
    }

    #[test]
    fn folder_from_the_explorer_reaches_the_arguments() {
        let args: Vec<String> = ["--carpeta", r#"D:""#].map(String::from).to_vec();
        assert_eq!(parse_args(&args).folder, Some(PathBuf::from(r"D:\")));
    }

    #[test]
    fn drive_roots_are_recognised() {
        assert_eq!(drive_root(Path::new(r"c:\")).as_deref(), Some("C:"));
        assert_eq!(drive_root(Path::new("D:")).as_deref(), Some("D:"));
        assert_eq!(drive_root(Path::new(r"C:\Users")), None);
        assert_eq!(drive_root(Path::new(r"\\srv\datos")), None);
    }

    #[test]
    fn within_compares_whole_folder_names() {
        let tmp = Path::new(r"C:\Users\Ana\AppData\Local\Temp");
        assert!(is_within(
            Path::new(r"c:\users\ana\appdata\local\temp\Temp1_Lince.zip\Lince.exe"),
            tmp
        ));
        assert!(is_within(tmp, tmp));
        assert!(!is_within(
            Path::new(r"C:\Users\Ana\AppData\Local\Temporal\x"),
            tmp
        ));
        assert!(!is_within(Path::new(r"D:\Lince\Lince.exe"), tmp));
    }
}
