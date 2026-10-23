//! Argumentos de Lince.exe. Sin argumentos se indexan todos los volúmenes
//! NTFS; `--path`, `--volume` e `--image` eligen otras fuentes.

use std::path::PathBuf;

use lince_core::Source;

/// Marca que lleva la copia de Lince relanzada con permisos de administrador,
/// para no volver a pedirlos en bucle.
pub const RELAUNCH_FLAG: &str = "--elevado";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiArgs {
    pub sources: Vec<Source>,
    /// El usuario eligió las fuentes (no se piden permisos de administrador).
    pub explicit_sources: bool,
    pub relaunched: bool,
}

/// Interpreta los argumentos (sin el nombre del programa). Lo desconocido se
/// ignora: una ventana no tiene dónde mostrar errores de línea de comandos.
pub fn parse_args(args: &[String]) -> GuiArgs {
    let mut out = GuiArgs {
        sources: Vec::new(),
        explicit_sources: false,
        relaunched: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            RELAUNCH_FLAG => out.relaunched = true,
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
    fn relaunch_flag_and_unknown_arguments() {
        let a = parse(&["--elevado", "--raro", "--path"]);
        assert!(a.relaunched);
        assert!(a.sources.is_empty());
    }
}
