//! Constructor de registros FILE sintéticos para las pruebas unitarias.

/// Número de secuencia de actualización que se escribe al final de cada sector.
pub const USN: u16 = 0x4242;

const HEADER: usize = 0x30;

fn align8(n: usize) -> usize {
    (n + 7) & !7
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

pub struct RecordBuilder {
    size: usize,
    flags: u16,
    sequence: u16,
    base: u64,
    attrs: Vec<Vec<u8>>,
}

impl RecordBuilder {
    pub fn new(size: usize) -> Self {
        RecordBuilder {
            size,
            flags: 0x0001,
            sequence: 1,
            base: 0,
            attrs: Vec::new(),
        }
    }

    pub fn not_in_use(mut self) -> Self {
        self.flags &= !0x0001;
        self
    }

    pub fn directory(mut self) -> Self {
        self.flags |= 0x0002;
        self
    }

    pub fn sequence(mut self, sequence: u16) -> Self {
        self.sequence = sequence;
        self
    }

    /// Marca el registro como extensión del registro base `frs`.
    pub fn base(mut self, frs: u64) -> Self {
        self.base = frs;
        self
    }

    pub fn std_info(self, created: u64, modified: u64, accessed: u64, attributes: u32) -> Self {
        let mut v = vec![0u8; 72];
        v[0..8].copy_from_slice(&created.to_le_bytes());
        v[8..16].copy_from_slice(&modified.to_le_bytes());
        v[16..24].copy_from_slice(&modified.to_le_bytes());
        v[24..32].copy_from_slice(&accessed.to_le_bytes());
        v[32..36].copy_from_slice(&attributes.to_le_bytes());
        self.resident(0x10, "", &v)
    }

    pub fn file_name(self, parent_frs: u64, parent_seq: u16, namespace: u8, name: &str) -> Self {
        let units = name.encode_utf16().count();
        let mut v = vec![0u8; 0x42];
        let parent = parent_frs | (u64::from(parent_seq) << 48);
        v[0..8].copy_from_slice(&parent.to_le_bytes());
        v[0x40] = units as u8;
        v[0x41] = namespace;
        v.extend(utf16(name));
        self.resident(0x30, "", &v)
    }

    pub fn resident(mut self, type_code: u32, name: &str, value: &[u8]) -> Self {
        let name = utf16(name);
        let value_offset = align8(0x18 + name.len());
        let len = align8(value_offset + value.len());
        let mut a = vec![0u8; len];
        a[0..4].copy_from_slice(&type_code.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[8] = 0;
        a[9] = (name.len() / 2) as u8;
        a[0x0A..0x0C].copy_from_slice(&0x18u16.to_le_bytes());
        a[0x10..0x14].copy_from_slice(&(value.len() as u32).to_le_bytes());
        a[0x14..0x16].copy_from_slice(&(value_offset as u16).to_le_bytes());
        a[0x18..0x18 + name.len()].copy_from_slice(&name);
        a[value_offset..value_offset + value.len()].copy_from_slice(value);
        self.attrs.push(a);
        self
    }

    pub fn nonresident(
        mut self,
        type_code: u32,
        name: &str,
        lowest_vcn: u64,
        data_size: u64,
        runs: &[u8],
    ) -> Self {
        let name = utf16(name);
        let runs_offset = align8(0x40 + name.len());
        let len = align8(runs_offset + runs.len());
        let mut a = vec![0u8; len];
        a[0..4].copy_from_slice(&type_code.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[8] = 1;
        a[9] = (name.len() / 2) as u8;
        a[0x0A..0x0C].copy_from_slice(&0x40u16.to_le_bytes());
        a[0x10..0x18].copy_from_slice(&lowest_vcn.to_le_bytes());
        a[0x20..0x22].copy_from_slice(&(runs_offset as u16).to_le_bytes());
        let allocated = (data_size + 4095) & !4095;
        a[0x28..0x30].copy_from_slice(&allocated.to_le_bytes());
        a[0x30..0x38].copy_from_slice(&data_size.to_le_bytes());
        a[0x38..0x40].copy_from_slice(&data_size.to_le_bytes());
        a[0x40..0x40 + name.len()].copy_from_slice(&name);
        a[runs_offset..runs_offset + runs.len()].copy_from_slice(runs);
        self.attrs.push(a);
        self
    }

    fn usa_count(&self) -> usize {
        1 + self.size / 512
    }

    /// Registro tal como queda después de aplicar los fixups (en memoria).
    pub fn build_raw(&self) -> Vec<u8> {
        let mut r = vec![0u8; self.size];
        r[0..4].copy_from_slice(b"FILE");
        r[4..6].copy_from_slice(&(HEADER as u16).to_le_bytes());
        r[6..8].copy_from_slice(&(self.usa_count() as u16).to_le_bytes());
        r[0x10..0x12].copy_from_slice(&self.sequence.to_le_bytes());
        let first = align8(HEADER + 2 * self.usa_count());
        r[0x14..0x16].copy_from_slice(&(first as u16).to_le_bytes());
        r[0x16..0x18].copy_from_slice(&self.flags.to_le_bytes());
        r[0x1C..0x20].copy_from_slice(&(self.size as u32).to_le_bytes());
        r[0x20..0x28].copy_from_slice(&self.base.to_le_bytes());
        let mut pos = first;
        for a in &self.attrs {
            r[pos..pos + a.len()].copy_from_slice(a);
            pos += a.len();
        }
        r[pos..pos + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        pos += 8;
        assert!(pos <= self.size, "los atributos no caben en el registro");
        r[0x18..0x1C].copy_from_slice(&(pos as u32).to_le_bytes());
        // En memoria el USA conserva el USN y una copia del final de cada sector.
        r[HEADER..HEADER + 2].copy_from_slice(&USN.to_le_bytes());
        for i in 0..self.usa_count() - 1 {
            let end = (i + 1) * 512 - 2;
            let slot = HEADER + 2 + 2 * i;
            r[slot] = r[end];
            r[slot + 1] = r[end + 1];
        }
        r
    }

    /// Registro tal como está en disco: el final de cada sector lleva el USN.
    pub fn build(&self) -> Vec<u8> {
        let mut r = self.build_raw();
        for i in 0..self.usa_count() - 1 {
            let end = (i + 1) * 512 - 2;
            r[end..end + 2].copy_from_slice(&USN.to_le_bytes());
        }
        r
    }
}

/// Una entrada de `$ATTRIBUTE_LIST`.
pub fn attr_list_entry(type_code: u32, lowest_vcn: u64, record_frs: u64, name: &str) -> Vec<u8> {
    let name = utf16(name);
    let len = align8(0x1A + name.len());
    let mut e = vec![0u8; len];
    e[0..4].copy_from_slice(&type_code.to_le_bytes());
    e[4..6].copy_from_slice(&(len as u16).to_le_bytes());
    e[6] = (name.len() / 2) as u8;
    e[7] = 0x1A;
    e[8..16].copy_from_slice(&lowest_vcn.to_le_bytes());
    e[16..24].copy_from_slice(&(record_frs | (1u64 << 48)).to_le_bytes());
    e[0x1A..0x1A + name.len()].copy_from_slice(&name);
    e
}
