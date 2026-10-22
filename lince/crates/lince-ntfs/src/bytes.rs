//! Lectura de enteros little-endian con comprobación de límites.

#[inline]
pub fn u8_at(b: &[u8], off: usize) -> Option<u8> {
    b.get(off).copied()
}

#[inline]
pub fn u16_at(b: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(off..off + 2)?.try_into().ok()?))
}

#[inline]
pub fn u32_at(b: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(off..off + 4)?.try_into().ok()?))
}

#[inline]
pub fn u64_at(b: &[u8], off: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(off..off + 8)?.try_into().ok()?))
}

/// Separa una referencia de archivo NTFS en (FRS de 48 bits, secuencia).
#[inline]
pub fn split_ref(r: u64) -> (u64, u16) {
    (r & 0x0000_FFFF_FFFF_FFFF, (r >> 48) as u16)
}
