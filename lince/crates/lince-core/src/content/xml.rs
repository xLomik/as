//! Word y PowerPoint modernos (.docx, .pptx) y OpenDocument (.odt, .odp):
//! un zip con el texto repartido en XML.

use std::io::{BufRead, BufReader, Read, Seek};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;
use zip::ZipArchive;

use super::{err, open, ContentError};

/// Qué elementos llevan texto y cuáles separan palabras.
struct Rules {
    /// Solo el texto dentro de estos elementos (`None`: todo el texto).
    text: Option<&'static [&'static str]>,
    /// Al cerrarse (o vacíos) equivalen a un espacio: párrafos, celdas,
    /// tabuladores y saltos de línea.
    breaks: &'static [&'static str],
}

/// `w:t` (Word), `a:t` (PowerPoint y dibujos), `m:t` (ecuaciones).
const OFFICE: Rules = Rules {
    text: Some(&["t"]),
    breaks: &["p", "tab", "br", "cr", "tc"],
};

const ODF: Rules = Rules {
    text: None,
    breaks: &[
        "p",
        "h",
        "tab",
        "s",
        "line-break",
        "table-cell",
        "list-item",
    ],
};

pub fn office(path: &Path) -> Result<String, ContentError> {
    from_zip(path, is_office_text, &OFFICE)
}

pub fn open_document(path: &Path) -> Result<String, ContentError> {
    from_zip(path, |name| name == "content.xml", &ODF)
}

/// Partes con texto: el documento de Word con encabezados, pies y notas
/// (no plantillas de bloques), y las diapositivas con sus notas (no los
/// patrones, que repiten «Haga clic para…»).
fn is_office_text(name: &str) -> bool {
    let word = name
        .strip_prefix("word/")
        .is_some_and(|rest| !rest.contains('/') && rest.ends_with(".xml"));
    let slides = ["ppt/slides/", "ppt/notesSlides/"].iter().any(|dir| {
        name.strip_prefix(dir)
            .is_some_and(|rest| !rest.contains('/'))
    }) && name.ends_with(".xml");
    word || slides
}

fn from_zip(
    path: &Path,
    wanted: impl Fn(&str) -> bool,
    rules: &Rules,
) -> Result<String, ContentError> {
    let mut zip = ZipArchive::new(BufReader::new(open(path)?)).map_err(err)?;
    // Nombres con caracteres inválidos no son partes de texto: se saltan.
    let parts: Vec<String> = zip
        .file_names()
        .filter_map(Result::ok)
        .filter(|n| wanted(n))
        .map(String::from)
        .collect();
    // Sin partes de texto (p. ej. una plantilla de solo estilos) queda vacío.
    let mut out = String::new();
    for name in parts {
        read_part(&mut zip, &name, rules, &mut out)?;
        out.push('\n');
    }
    Ok(out)
}

fn read_part<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    name: &str,
    rules: &Rules,
    out: &mut String,
) -> Result<(), ContentError> {
    let part = zip.by_name(name).map_err(err)?;
    xml_text(BufReader::new(part), rules, out)
}

fn xml_text(src: impl BufRead, rules: &Rules, out: &mut String) -> Result<(), ContentError> {
    let mut reader = Reader::from_reader(src);
    let mut buf = Vec::new();
    // Profundidad dentro de elementos de texto (`w:t` no se anida, pero así
    // no importa).
    let mut inside = 0usize;
    loop {
        let collect = rules.text.is_none() || inside > 0;
        match reader.read_event_into(&mut buf).map_err(err)? {
            Event::Start(e) => {
                if rules.text.is_some_and(|t| has(t, e.local_name().as_ref())) {
                    inside += 1;
                }
            }
            Event::End(e) => {
                let name = e.local_name();
                if rules.text.is_some_and(|t| has(t, name.as_ref())) {
                    inside = inside.saturating_sub(1);
                }
                if has(rules.breaks, name.as_ref()) {
                    out.push(' ');
                }
            }
            Event::Empty(e) => {
                if has(rules.breaks, e.local_name().as_ref()) {
                    out.push(' ');
                }
            }
            Event::Text(t) if collect => out.push_str(t.as_ref()),
            Event::CData(t) if collect => out.push_str(t.as_ref()),
            Event::GeneralRef(r) if collect => {
                let ch = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c),
                    _ => match r.as_ref() {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        _ => None,
                    },
                };
                out.extend(ch);
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
        buf.clear();
    }
}

fn has(list: &[&str], name: &str) -> bool {
    list.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(xml: &str, rules: &Rules) -> String {
        let mut out = String::new();
        xml_text(xml.as_bytes(), rules, &mut out).unwrap();
        out
    }

    #[test]
    fn word_runs_join_and_paragraphs_separate() {
        let xml = r#"<w:document xmlns:w="w"><w:body>
            <w:p><w:r><w:t>par</w:t></w:r><w:r><w:t>tido &amp; m&#225;s</w:t></w:r></w:p>
            <w:p><w:r><w:t>uno</w:t><w:tab/><w:t>dos</w:t></w:r></w:p>
            <w:p><w:pPr><w:rStyle w:val="NoEsTexto"/></w:pPr></w:p>
            </w:body></w:document>"#;
        let t = text(xml, &OFFICE);
        assert!(t.contains("partido & más"), "{t:?}");
        assert!(t.contains("uno dos"), "{t:?}");
        assert!(!t.contains("NoEsTexto"));
        assert!(!t.contains("másuno"), "{t:?}");
    }

    #[test]
    fn open_document_takes_all_text() {
        let xml = r#"<office:body><text:p>Hola <text:span>mun</text:span>do</text:p><text:p>adiós</text:p></office:body>"#;
        let t = text(xml, &ODF);
        assert!(t.contains("Hola mundo"), "{t:?}");
        assert!(!t.contains("mundoadiós"), "{t:?}");
    }

    #[test]
    fn office_parts_with_text() {
        for yes in [
            "word/document.xml",
            "word/header1.xml",
            "word/footnotes.xml",
            "ppt/slides/slide12.xml",
            "ppt/notesSlides/notesSlide1.xml",
        ] {
            assert!(is_office_text(yes), "{yes}");
        }
        for no in [
            "word/_rels/document.xml.rels",
            "word/glossary/document.xml",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slides/_rels/slide1.xml.rels",
            "[Content_Types].xml",
        ] {
            assert!(!is_office_text(no), "{no}");
        }
    }
}
