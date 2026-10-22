//! Sintaxis de consultas.
//!
//! Términos separados por espacios (todos deben cumplirse). Un término con
//! `*` o `?` debe coincidir con el nombre completo; sin comodines basta con
//! que aparezca dentro. `"..."` agrupa espacios y `!` niega un término.
//!
//! Filtros: `ext:pdf;docx`, `size:>10mb`, `size:1kb..2mb`, `dm:2026-10`,
//! `dm:>=2026-01-01`, `dc:` (creación), `file:`, `folder:`, `path:texto`,
//! `regex:expresión`.

use std::fmt;

use chrono::{Datelike, NaiveDate};
use regex::{Regex, RegexBuilder};

use crate::fold_into;
use crate::time::local_day_start;

/// Patrón sobre el nombre en minúsculas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    Substring(Vec<u8>),
    Wildcard(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Any,
    Files,
    Folders,
}

/// Consulta ya interpretada.
#[derive(Debug, Clone)]
pub struct Query {
    pub terms: Vec<Pattern>,
    pub not_terms: Vec<Pattern>,
    /// Subcadenas que debe contener la ruta completa (en minúsculas).
    pub path_terms: Vec<Vec<u8>>,
    /// Extensiones permitidas, en minúsculas y sin punto.
    pub exts: Vec<Vec<u8>>,
    /// Rango de tamaño inclusivo; solo archivos.
    pub size: Option<(u64, u64)>,
    /// Rangos de fecha `[desde, hasta)` en FILETIME.
    pub modified: Option<(u64, u64)>,
    pub created: Option<(u64, u64)>,
    pub kind: Kind,
    pub regex: Option<Regex>,
}

/// Error de sintaxis, con un mensaje para el usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError(pub String);

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for QueryError {}

impl Pattern {
    fn new(text: &str) -> Pattern {
        let mut folded = Vec::with_capacity(text.len());
        fold_into(text, &mut folded);
        if folded.iter().any(|&b| b == b'*' || b == b'?') {
            Pattern::Wildcard(folded)
        } else {
            Pattern::Substring(folded)
        }
    }

    /// ¿Coincide con un nombre ya pasado a minúsculas?
    pub fn matches(&self, name: &[u8]) -> bool {
        match self {
            Pattern::Substring(s) => memchr::memmem::find(name, s).is_some(),
            Pattern::Wildcard(p) => wildcard_match(p, name),
        }
    }

    /// El trozo literal más largo, para preseleccionar candidatos con SIMD.
    pub fn longest_literal(&self) -> &[u8] {
        match self {
            Pattern::Substring(s) => s,
            Pattern::Wildcard(p) => p
                .split(|&b| b == b'*' || b == b'?')
                .max_by_key(|s| s.len())
                .unwrap_or(&[]),
        }
    }
}

/// Comodines sobre UTF-8: `*` cualquier secuencia, `?` un carácter.
fn wildcard_match(p: &[u8], t: &[u8]) -> bool {
    let char_len = |b: u8| match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    };
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        match p.get(pi) {
            Some(b'*') => {
                star = Some((pi, ti));
                pi += 1;
            }
            Some(b'?') => {
                pi += 1;
                ti += char_len(t[ti]);
            }
            Some(&c) if c == t[ti] => {
                pi += 1;
                ti += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    let next = st + char_len(t[st]);
                    star = Some((sp, next));
                    pi = sp + 1;
                    ti = next;
                }
                None => return false,
            },
        }
    }
    p[pi.min(p.len())..].iter().all(|&b| b == b'*') && ti == t.len()
}

/// Interpreta el texto de búsqueda.
pub fn parse_query(text: &str) -> Result<Query, QueryError> {
    let mut q = Query {
        terms: Vec::new(),
        not_terms: Vec::new(),
        path_terms: Vec::new(),
        exts: Vec::new(),
        size: None,
        modified: None,
        created: None,
        kind: Kind::Any,
        regex: None,
    };
    for token in tokenize(text) {
        if let Some(rest) = token.strip_prefix('!') {
            if !rest.is_empty() {
                q.not_terms.push(Pattern::new(rest));
            }
            continue;
        }
        let Some((prefix, value)) = token.split_once(':') else {
            q.terms.push(Pattern::new(&token));
            continue;
        };
        match prefix.to_ascii_lowercase().as_str() {
            "ext" => {
                for e in value.split([';', ',']).map(|e| e.trim_start_matches('.')) {
                    if !e.is_empty() {
                        let mut f = Vec::new();
                        fold_into(e, &mut f);
                        q.exts.push(f);
                    }
                }
            }
            "size" => q.size = Some(parse_size_filter(value)?),
            "dm" => q.modified = Some(parse_date_filter(value, "dm")?),
            "dc" => q.created = Some(parse_date_filter(value, "dc")?),
            "file" | "folder" => {
                q.kind = if prefix.eq_ignore_ascii_case("file") {
                    Kind::Files
                } else {
                    Kind::Folders
                };
                if !value.is_empty() {
                    q.terms.push(Pattern::new(value));
                }
            }
            "path" => {
                if !value.is_empty() {
                    let mut f = Vec::new();
                    fold_into(value, &mut f);
                    q.path_terms.push(f);
                }
            }
            "regex" => {
                let re = RegexBuilder::new(value)
                    .case_insensitive(true)
                    .build()
                    .map_err(|e| QueryError(format!("expresión regular inválida: {e}")))?;
                q.regex = Some(re);
            }
            // Un prefijo desconocido es parte del nombre.
            _ => q.terms.push(Pattern::new(&token)),
        }
    }
    Ok(q)
}

fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in text.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

enum Op {
    Eq,
    Gt,
    Ge,
    Lt,
    Le,
}

fn split_op(v: &str) -> (Op, &str) {
    for (p, op) in [
        (">=", Op::Ge),
        ("<=", Op::Le),
        (">", Op::Gt),
        ("<", Op::Lt),
        ("=", Op::Eq),
    ] {
        if let Some(rest) = v.strip_prefix(p) {
            return (op, rest);
        }
    }
    (Op::Eq, v)
}

fn parse_size_filter(v: &str) -> Result<(u64, u64), QueryError> {
    let err = || {
        QueryError(format!(
            "tamaño inválido en «size:{v}» (ejemplos: size:>10mb, size:<1kb, size:1kb..2mb)"
        ))
    };
    if let Some((a, b)) = v.split_once("..") {
        let (lo, hi) = (
            parse_size(a).ok_or_else(err)?,
            parse_size(b).ok_or_else(err)?,
        );
        return Ok((lo.min(hi), lo.max(hi)));
    }
    let (op, rest) = split_op(v);
    let n = parse_size(rest).ok_or_else(err)?;
    Ok(match op {
        Op::Eq => (n, n),
        Op::Gt => (n.saturating_add(1), u64::MAX),
        Op::Ge => (n, u64::MAX),
        Op::Lt if n == 0 => (1, 0),
        Op::Lt => (0, n - 1),
        Op::Le => (0, n),
    })
}

/// `10`, `10kb`, `1.5mb`, `2g`... (múltiplos de 1024).
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_ascii_lowercase();
    let split = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    if split == 0 {
        return None;
    }
    let (num, unit) = s.split_at(split);
    let mult: u64 = match unit {
        "" | "b" => 1,
        "k" | "kb" => 1 << 10,
        "m" | "mb" => 1 << 20,
        "g" | "gb" => 1 << 30,
        "t" | "tb" => 1 << 40,
        _ => return None,
    };
    let value: f64 = num.parse().ok()?;
    Some((value * mult as f64).round() as u64)
}

fn parse_date_filter(v: &str, prefix: &str) -> Result<(u64, u64), QueryError> {
    let err = || {
        QueryError(format!(
            "fecha inválida en «{prefix}:{v}» (ejemplos: {prefix}:2026, {prefix}:2026-10, \
             {prefix}:>=2026-01-15, {prefix}:22/10/2026, {prefix}:2026-01..2026-03)"
        ))
    };
    if let Some((a, b)) = v.split_once("..") {
        let (start, _) = parse_period(a).ok_or_else(err)?;
        let (_, end) = parse_period(b).ok_or_else(err)?;
        return Ok((start, end));
    }
    let (op, rest) = split_op(v);
    let (start, end) = parse_period(rest).ok_or_else(err)?;
    Ok(match op {
        Op::Eq => (start, end),
        Op::Gt => (end, u64::MAX),
        Op::Ge => (start, u64::MAX),
        Op::Lt => (0, start),
        Op::Le => (0, end),
    })
}

/// Periodo local `[inicio, fin)` de `AAAA`, `AAAA-MM`, `AAAA-MM-DD` o `DD/MM/AAAA`.
fn parse_period(s: &str) -> Option<(u64, u64)> {
    let nums: Option<Vec<u32>> = s.split(['-', '/']).map(|p| p.parse().ok()).collect();
    let nums = nums?;
    let (first, last) = match (nums.as_slice(), s.contains('/')) {
        (&[y], false) => (
            NaiveDate::from_ymd_opt(y as i32, 1, 1)?,
            next_year(y as i32)?,
        ),
        (&[y, m], false) => {
            let d = NaiveDate::from_ymd_opt(y as i32, m, 1)?;
            (d, d.checked_add_months(chrono::Months::new(1))?)
        }
        (&[y, m, d], false) | (&[d, m, y], true) => {
            let d = NaiveDate::from_ymd_opt(y as i32, m, d)?;
            (d, d.succ_opt()?)
        }
        _ => return None,
    };
    let start = local_day_start(first.year(), first.month(), first.day())?;
    let end = local_day_start(last.year(), last.month(), last.day())?;
    Some((start, end))
}

fn next_year(y: i32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(y + 1, 1, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_basics() {
        assert!(wildcard_match(b"*.txt", b"notas.txt"));
        assert!(!wildcard_match(b"*.txt", b"notas.txt.bak"));
        assert!(wildcard_match(b"a*b*c", b"axxbyyc"));
        assert!(!wildcard_match(b"a?", b"a"));
        assert!(wildcard_match("ma?ana".as_bytes(), "mañana".as_bytes()));
        assert!(wildcard_match(b"*", b""));
    }

    #[test]
    fn sizes_with_units() {
        assert_eq!(parse_size("10"), Some(10));
        assert_eq!(parse_size("1kb"), Some(1024));
        assert_eq!(parse_size("1.5MB"), Some(1_572_864));
        assert_eq!(parse_size("abc"), None);
        assert_eq!(parse_size("10xb"), None);
        assert_eq!(parse_size(""), None);
    }

    #[test]
    fn longest_literal_of_wildcard() {
        assert_eq!(Pattern::new("*.jpeg").longest_literal(), b".jpeg");
        assert_eq!(Pattern::new("f?to*").longest_literal(), b"to");
    }
}
