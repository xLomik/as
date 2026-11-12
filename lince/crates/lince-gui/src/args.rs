//! Argumentos de Lince.exe. Sin argumentos se indexan todos los volúmenes
//! NTFS; `--path`, `--volume` e `--image` eligen otras fuentes y
//! `--carpeta` (la del clic derecho del Explorador) limita a una carpeta.

use std::path::{Path, PathBuf};

use lince_core::Source;

use crate::explorer::{clean_shell_path, FOLDER_FLAG};
use crate::startup::STARTUP_FLAG;

/// Marca que lleva la copia de Lince relanzada con permisos de administrador,
/// para no volver a pedirlos en bucle.
pub const RELAUNCH_FLAG: &str = "--elevado";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiArgs {
    pub sources: Vec<Source>,
    /// Carpetas que se suman a los volúmenes (las unidades de red que la
    /// copia sin elevar le pasa a la elevada, que no las ve).
    pub extra: Vec<PathBuf>,
    /// El usuario eligió las fuentes (no se piden permisos de administrador).
    pub explicit_sources: bool,
    pub relaunched: bool,
    /// Buscar solo en esta carpeta (clic derecho del Explorador).
    pub folder: Option<PathBuf>,
    /// Instalar o quitar el clic derecho del Explorador y salir.
    pub menu: Option<SetupAction>,
    /// Cerrar del todo la copia que está en segundo plano.
    pub quit: bool,
    /// Abierto por la tarea de inicio: queda escondido junto al reloj.
    pub startup: bool,
    /// Activar o quitar el inicio con Windows y salir.
    pub autostart: Option<SetupAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Instalar o quitar algo (clic derecho, inicio con Windows...).
pub enum SetupAction {
    Install,
    Remove,
}

/// Interpreta los argumentos (sin el nombre del programa). Lo desconocido se
/// ignora: una ventana no tiene dónde mostrar errores de línea de comandos.
pub fn parse_args(args: &[String]) -> GuiArgs {
    let mut out = GuiArgs {
        sources: Vec::new(),
        extra: Vec::new(),
        explicit_sources: false,
        relaunched: false,
        folder: None,
        menu: None,
        quit: false,
        startup: false,
        autostart: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            RELAUNCH_FLAG => out.relaunched = true,
            "--instalar-menu" => out.menu = Some(SetupAction::Install),
            "--quitar-menu" => out.menu = Some(SetupAction::Remove),
            "--salir" => out.quit = true,
            STARTUP_FLAG => out.startup = true,
            "--instalar-inicio" => out.autostart = Some(SetupAction::Install),
            "--quitar-inicio" => out.autostart = Some(SetupAction::Remove),
            FOLDER_FLAG => {
                let Some(v) = it.next() else { break };
                out.folder = Some(clean_shell_path(v));
            }
            "--extra" => {
                let Some(v) = it.next() else { break };
                out.extra.push(PathBuf::from(v));
            }
            "--image" | "--path" | "--volume" => {
                let Some(v) = it.next() else { break };
                out.sources.push(match a.as_str() {
                    "--image" => Source::Image(PathBuf::from(v)),
                    "--path" => Source::Path(PathBuf::from(v)),
                    _ => Source::Volume(v.clone()),
                });
            }
            _ => {}
        }
    }
    out.explicit_sources = !out.sources.is_empty();
    out
}

/// Parámetros para relanzar Lince elevado, pasando las unidades de red y la
/// carpeta elegida en el Explorador.
pub fn relaunch_params(extra: &[PathBuf], folder: Option<&Path>) -> String {
    let mut out = RELAUNCH_FLAG.to_string();
    for p in extra {
        out.push_str(" --extra ");
        out.push_str(&quote_arg(&p.to_string_lossy()));
    }
    if let Some(f) = folder {
        out.push_str(&format!(" {FOLDER_FLAG} "));
        out.push_str(&quote_arg(&f.to_string_lossy()));
    }
    out
}

/// Entrecomilla un argumento según las reglas de la línea de comandos de
/// Windows (`CommandLineToArgvW`): las barras invertidas solo son especiales
/// justo antes de una comilla.
pub fn quote_arg(s: &str) -> String {
    if !s.is_empty() && !s.contains([' ', '\t', '"']) {
        return s.to_string();
    }
    let mut out = String::from('"');
    let mut slashes = 0;
    for c in s.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', slashes));
                out.push(c);
                slashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(a: &[&str]) -> GuiArgs {
        parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn no_arguments_means_all_volumes() {
        let a = parse(&[]);
        assert!(a.sources.is_empty());
        assert!(!a.explicit_sources);
        assert!(!a.relaunched);
    }

    #[test]
    fn explicit_sources_in_order() {
        let a = parse(&["--image", "x.img", "--path", "D:\\datos", "--volume", "e:"]);
        assert_eq!(
            a.sources,
            vec![
                Source::Image(PathBuf::from("x.img")),
                Source::Path(PathBuf::from("D:\\datos")),
                Source::Volume("e:".into()),
            ]
        );
        assert!(a.explicit_sources);
    }

    #[test]
    fn extra_folders_add_to_the_volumes() {
        let a = parse(&[
            "--elevado",
            "--extra",
            r"\\srv\datos",
            "--extra",
            r"\\srv\otra cosa",
        ]);
        assert!(!a.explicit_sources);
        assert!(a.sources.is_empty());
        assert_eq!(
            a.extra,
            vec![
                PathBuf::from(r"\\srv\datos"),
                PathBuf::from(r"\\srv\otra cosa")
            ]
        );
    }

    /// Reglas de Windows: comillas si hay espacios, y las barras antes de
    /// una comilla se duplican (si no, `"C:\"` se comería la comilla).
    #[test]
    fn arguments_are_quoted_for_windows() {
        assert_eq!(quote_arg(r"\\srv\datos"), r"\\srv\datos");
        assert_eq!(quote_arg(r"\\srv\otra cosa"), r#""\\srv\otra cosa""#);
        assert_eq!(quote_arg(r"C:\Mis datos\"), r#""C:\Mis datos\\""#);
        assert_eq!(quote_arg(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote_arg(""), r#""""#);
    }

    #[test]
    fn relaunch_parameters_carry_network_drives() {
        let extra = [
            PathBuf::from(r"\\srv\datos"),
            PathBuf::from(r"\\srv\otra cosa"),
        ];
        assert_eq!(
            relaunch_params(&extra, None),
            r#"--elevado --extra \\srv\datos --extra "\\srv\otra cosa""#
        );
        let back = parse(&split_for_test(&relaunch_params(&extra, None)));
        assert!(back.relaunched);
        assert_eq!(back.extra, extra);
    }

    /// Separación mínima de la línea de comandos (sin barras especiales),
    /// suficiente para comprobar la ida y vuelta de la prueba anterior.
    fn split_for_test(cmd: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut rest = cmd.trim();
        while !rest.is_empty() {
            if let Some(r) = rest.strip_prefix('"') {
                let end = r.find('"').unwrap();
                out.push(&r[..end]);
                rest = r[end + 1..].trim_start();
            } else {
                let end = rest.find(' ').unwrap_or(rest.len());
                out.push(&rest[..end]);
                rest = rest[end..].trim_start();
            }
        }
        out
    }

    #[test]
    fn folder_and_explorer_menu_options() {
        let a = parse(&["--carpeta", r"C:\Users\Ana\Mis Documentos"]);
        assert_eq!(
            a.folder,
            Some(PathBuf::from(r"C:\Users\Ana\Mis Documentos"))
        );
        assert!(a.sources.is_empty());
        assert_eq!(a.menu, None);
        assert_eq!(parse(&["--instalar-menu"]).menu, Some(SetupAction::Install));
        assert_eq!(parse(&["--quitar-menu"]).menu, Some(SetupAction::Remove));
    }

    #[test]
    fn startup_options() {
        let a = parse(&["--inicio"]);
        assert!(a.startup);
        assert!(!a.explicit_sources);
        assert_eq!(
            parse(&["--instalar-inicio"]).autostart,
            Some(SetupAction::Install)
        );
        assert_eq!(
            parse(&["--quitar-inicio"]).autostart,
            Some(SetupAction::Remove)
        );
        assert_eq!(parse(&[]).autostart, None);
    }

    #[test]
    fn quit_closes_the_background_copy() {
        assert!(parse(&["--salir"]).quit);
        assert!(!parse(&[]).quit);
    }

    #[test]
    fn relaunch_parameters_keep_the_folder() {
        let folder = PathBuf::from(r"C:\");
        let params = relaunch_params(&[], Some(&folder));
        assert_eq!(params, r"--elevado --carpeta C:\");
        let back = parse(&split_for_test(&params));
        assert!(back.relaunched);
        assert_eq!(back.folder, Some(folder));
    }

    #[test]
    fn relaunch_flag_and_unknown_arguments() {
        let a = parse(&["--elevado", "--raro", "--path"]);
        assert!(a.relaunched);
        assert!(a.sources.is_empty());
    }
}
