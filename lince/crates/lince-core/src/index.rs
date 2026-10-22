//! Índice compacto: una entrada de 40 bytes por nombre, todos los nombres en un
//! único buffer y una copia en minúsculas separada por `\0` para buscar con
//! SIMD sobre memoria contigua.

/// La entrada no tiene carpeta padre conocida (raíz o huérfana).
pub const NO_PARENT: u32 = u32::MAX;

pub const FLAG_DIR: u16 = 0x01;
pub const FLAG_HIDDEN: u16 = 0x02;
pub const FLAG_SYSTEM: u16 = 0x04;
/// Raíz del volumen o de la carpeta recorrida. No aparece en resultados.
pub const FLAG_ROOT: u16 = 0x08;

/// Datos de un nombre del índice. Las fechas están en FILETIME (100 ns desde 1601).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub parent: u32,
    pub name_off: u32,
    pub name_len: u16,
    pub flags: u16,
    pub size: u64,
    pub modified: u64,
    pub created: u64,
}

/// Más niveles que esto solo puede ser un ciclo de carpetas padre (disco corrupto).
const MAX_DEPTH: usize = 4096;

/// Segundos entre 1601-01-01 y 1970-01-01.
pub(crate) const EPOCH_DIFF_SECS: i64 = 11_644_473_600;

/// Convierte FILETIME a segundos Unix (redondeando hacia abajo).
pub fn filetime_to_unix(ft: u64) -> i64 {
    (ft / 10_000_000) as i64 - EPOCH_DIFF_SECS
}

/// Índice de solo lectura.
pub struct Index {
    label: String,
    sep: char,
    entries: Vec<Entry>,
    names: String,
    folded: Vec<u8>,
    /// Inicio de cada nombre en `folded`, más un centinela final.
    folded_starts: Vec<u32>,
}

impl Index {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Prefijo de las rutas: `C:` o la carpeta recorrida.
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn separator(&self) -> char {
        self.sep
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn entry(&self, i: u32) -> &Entry {
        &self.entries[i as usize]
    }

    pub fn name(&self, i: u32) -> &str {
        let e = self.entry(i);
        &self.names[e.name_off as usize..e.name_off as usize + usize::from(e.name_len)]
    }

    pub fn is_dir(&self, i: u32) -> bool {
        self.entry(i).flags & FLAG_DIR != 0
    }

    /// Todos los nombres en minúsculas, cada uno seguido de `\0`.
    pub fn folded(&self) -> &[u8] {
        &self.folded
    }

    pub fn folded_name(&self, i: u32) -> &[u8] {
        let i = i as usize;
        &self.folded[self.folded_starts[i] as usize..self.folded_starts[i + 1] as usize - 1]
    }

    /// Posición en [`Index::folded`] donde empieza la entrada `i`
    /// (`i == len()` da el final del buffer).
    pub fn folded_start(&self, i: usize) -> usize {
        self.folded_starts[i] as usize
    }

    /// Entrada a la que pertenece la posición `off` de [`Index::folded`].
    pub fn entry_at_folded(&self, off: usize) -> u32 {
        (self.folded_starts.partition_point(|&s| s as usize <= off) - 1) as u32
    }

    /// Nombres desde la raíz hasta `i` (la raíz no se incluye). Una entrada
    /// sin raíz conocida cuelga de `?`.
    pub fn components(&self, i: u32) -> Vec<&str> {
        let mut out = Vec::new();
        let mut cur = i;
        loop {
            let e = self.entry(cur);
            if e.flags & FLAG_ROOT != 0 {
                break;
            }
            out.push(self.name(cur));
            if e.parent == NO_PARENT || out.len() > MAX_DEPTH {
                if out.len() > MAX_DEPTH {
                    out = vec![self.name(i)];
                }
                out.push("?");
                break;
            }
            cur = e.parent;
        }
        out.reverse();
        out
    }

    /// Ruta completa, p. ej. `C:\Docs\a.txt`.
    pub fn path(&self, i: u32) -> String {
        let mut s = self.label.clone();
        if !s.ends_with(self.sep) {
            s.push(self.sep);
        }
        for (k, c) in self.components(i).iter().enumerate() {
            if k > 0 {
                s.push(self.sep);
            }
            s.push_str(c);
        }
        s
    }
}

/// Construye un [`Index`] entrada por entrada.
pub struct IndexBuilder {
    label: String,
    sep: char,
    entries: Vec<Entry>,
    names: String,
}

impl IndexBuilder {
    pub fn new(label: &str, sep: char) -> Self {
        Self::with_capacity(label, sep, 0)
    }

    pub fn with_capacity(label: &str, sep: char, entries: usize) -> Self {
        IndexBuilder {
            label: label.to_string(),
            sep,
            entries: Vec::with_capacity(entries),
            names: String::with_capacity(entries * 16),
        }
    }

    /// Añade un nombre y devuelve su posición.
    pub fn push(
        &mut self,
        name: &str,
        parent: u32,
        flags: u16,
        size: u64,
        modified: u64,
        created: u64,
    ) -> u32 {
        let mut name = name;
        while name.len() > usize::from(u16::MAX) {
            // Imposible en NTFS (255 caracteres), pero nunca cortar a mitad de carácter.
            let mut cut = usize::from(u16::MAX);
            while !name.is_char_boundary(cut) {
                cut -= 1;
            }
            name = &name[..cut];
        }
        let idx = self.entries.len() as u32;
        self.entries.push(Entry {
            parent,
            name_off: self.names.len() as u32,
            name_len: name.len() as u16,
            flags,
            size,
            modified,
            created,
        });
        self.names.push_str(name);
        idx
    }

    pub fn set_parent(&mut self, i: u32, parent: u32) {
        self.entries[i as usize].parent = parent;
    }

    /// Termina el índice; las minúsculas se calculan en paralelo.
    pub fn finish(self) -> Index {
        use rayon::prelude::*;
        const CHUNK: usize = 16 * 1024;
        let names = &self.names;
        let parts: Vec<(Vec<u8>, Vec<u32>)> = self
            .entries
            .par_chunks(CHUNK)
            .map(|chunk| {
                let mut folded = Vec::with_capacity(chunk.len() * 16);
                let mut starts = Vec::with_capacity(chunk.len());
                for e in chunk {
                    starts.push(folded.len() as u32);
                    let name = &names[e.name_off as usize..][..usize::from(e.name_len)];
                    crate::fold_into(name, &mut folded);
                    folded.push(0);
                }
                (folded, starts)
            })
            .collect();
        let total: usize = parts.iter().map(|(f, _)| f.len()).sum();
        let mut folded = Vec::with_capacity(total);
        let mut folded_starts = Vec::with_capacity(self.entries.len() + 1);
        for (f, starts) in parts {
            let base = folded.len() as u32;
            folded_starts.extend(starts.into_iter().map(|s| s + base));
            folded.extend_from_slice(&f);
        }
        folded_starts.push(folded.len() as u32);
        Index {
            label: self.label,
            sep: self.sep,
            entries: self.entries,
            names: self.names,
            folded,
            folded_starts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Index {
        let mut b = IndexBuilder::new("C:", '\\');
        let root = b.push("", NO_PARENT, FLAG_DIR | FLAG_ROOT, 0, 0, 0);
        let docs = b.push("Docs", root, FLAG_DIR, 0, 0, 0);
        let a = b.push("Canción.TXT", NO_PARENT, 0, 8, 1, 2);
        b.set_parent(a, docs);
        b.push("huérfano.dat", NO_PARENT, 0, 1, 0, 0);
        b.finish()
    }

    #[test]
    fn stores_names_and_metadata() {
        let ix = sample();
        assert_eq!(ix.len(), 4);
        assert_eq!(ix.name(2), "Canción.TXT");
        assert_eq!(ix.entry(2).size, 8);
        assert_eq!(ix.entry(2).modified, 1);
        assert!(ix.is_dir(1));
        assert!(!ix.is_dir(2));
    }

    #[test]
    fn builds_full_paths() {
        let ix = sample();
        assert_eq!(ix.path(0), "C:\\");
        assert_eq!(ix.path(1), "C:\\Docs");
        assert_eq!(ix.path(2), "C:\\Docs\\Canción.TXT");
        assert_eq!(ix.components(2), vec!["Docs", "Canción.TXT"]);
    }

    #[test]
    fn orphans_hang_from_a_marker() {
        assert_eq!(sample().path(3), "C:\\?\\huérfano.dat");
    }

    #[test]
    fn folded_buffer_maps_back_to_entries() {
        let ix = sample();
        assert_eq!(ix.folded_name(2), "canción.txt".as_bytes());
        let off = memfind(ix.folded(), "canción".as_bytes()).unwrap();
        assert_eq!(ix.entry_at_folded(off), 2);
        // Cada nombre termina en \0: una búsqueda no cruza nombres.
        assert_eq!(ix.folded()[off + "canción.txt".len()], 0);
    }

    #[test]
    fn parent_cycles_do_not_hang() {
        let mut b = IndexBuilder::new("C:", '\\');
        let x = b.push("x", NO_PARENT, FLAG_DIR, 0, 0, 0);
        let y = b.push("y", x, FLAG_DIR, 0, 0, 0);
        b.set_parent(x, y);
        let ix = b.finish();
        assert!(ix.path(y).starts_with("C:\\?\\"));
    }

    #[test]
    fn filetime_converts_to_unix_seconds() {
        assert_eq!(filetime_to_unix(116_444_736_000_000_000), 0);
        assert_eq!(filetime_to_unix(116_444_736_015_000_000), 1);
    }

    fn memfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }
}
