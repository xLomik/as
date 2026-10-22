use crate::bytes::{u16_at, u64_at};
use crate::NtfsError;

/// Datos del sector de arranque necesarios para localizar la MFT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootSector {
    pub bytes_per_sector: u32,
    pub bytes_per_cluster: u32,
    /// Clúster lógico donde empieza la MFT.
    pub mft_lcn: u64,
    /// Tamaño de cada registro FILE (normalmente 1024; 4096 en discos 4Kn).
    pub record_size: u32,
}

/// Interpreta el sector de arranque (los primeros 512 bytes del volumen).
pub fn parse_boot_sector(buf: &[u8]) -> Result<BootSector, NtfsError> {
    let b = buf
        .get(..512)
        .ok_or(NtfsError::BadBootSector("faltan bytes"))?;
    if &b[3..11] != b"NTFS    " {
        return Err(NtfsError::NotNtfs);
    }
    let bytes_per_sector = u32::from(u16_at(b, 0x0B).unwrap_or(0));
    if !(512..=4096).contains(&bytes_per_sector) || !bytes_per_sector.is_power_of_two() {
        return Err(NtfsError::BadBootSector("tamaño de sector"));
    }
    // Clústeres de más de 64 KiB se guardan como exponente negativo.
    let spc = b[0x0D] as i8;
    let sectors_per_cluster: u32 = if spc > 0 {
        spc as u32
    } else {
        let exp = -i32::from(spc);
        if spc == 0 || exp > 20 {
            return Err(NtfsError::BadBootSector("sectores por clúster"));
        }
        1 << exp
    };
    let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
    if !sectors_per_cluster.is_power_of_two() || bytes_per_cluster > 2 * 1024 * 1024 {
        return Err(NtfsError::BadBootSector("tamaño de clúster"));
    }
    // Positivo: clústeres por registro. Negativo: 2^-n bytes.
    let cpr = b[0x40] as i8;
    let record_size: u32 = if cpr > 0 {
        cpr as u32 * bytes_per_cluster
    } else {
        let exp = -i32::from(cpr);
        if cpr == 0 || exp > 16 {
            return Err(NtfsError::BadBootSector("tamaño de registro"));
        }
        1 << exp
    };
    if !(512..=65536).contains(&record_size) || !record_size.is_multiple_of(512) {
        return Err(NtfsError::BadBootSector("tamaño de registro"));
    }
    let mft_lcn = u64_at(b, 0x30).unwrap_or(0);
    if mft_lcn == 0 {
        return Err(NtfsError::BadBootSector("posición de la MFT"));
    }
    Ok(BootSector {
        bytes_per_sector,
        bytes_per_cluster,
        mft_lcn,
        record_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sector de arranque mínimo con los campos que leemos.
    fn boot(bps: u16, spc: u8, mft_lcn: u64, clusters_per_record: i8) -> Vec<u8> {
        let mut b = vec![0u8; 512];
        b[3..11].copy_from_slice(b"NTFS    ");
        b[0x0B..0x0D].copy_from_slice(&bps.to_le_bytes());
        b[0x0D] = spc;
        b[0x30..0x38].copy_from_slice(&mft_lcn.to_le_bytes());
        b[0x40] = clusters_per_record as u8;
        b[0x1FE] = 0x55;
        b[0x1FF] = 0xAA;
        b
    }

    #[test]
    fn parses_typical_4k_cluster_volume() {
        let bs = parse_boot_sector(&boot(512, 8, 786_432, -10)).unwrap();
        assert_eq!(
            bs,
            BootSector {
                bytes_per_sector: 512,
                bytes_per_cluster: 4096,
                mft_lcn: 786_432,
                record_size: 1024,
            }
        );
    }

    #[test]
    fn record_size_in_clusters_when_positive() {
        // Clúster de 512 B y 2 clústeres por registro = 1024 B.
        let bs = parse_boot_sector(&boot(512, 1, 4, 2)).unwrap();
        assert_eq!(bs.bytes_per_cluster, 512);
        assert_eq!(bs.record_size, 1024);
    }

    #[test]
    fn large_clusters_use_negative_exponent() {
        // 0xF8 = -8 → 2^8 sectores por clúster = 128 KiB con sectores de 512 B.
        let bs = parse_boot_sector(&boot(512, 0xF8, 4, -12)).unwrap();
        assert_eq!(bs.bytes_per_cluster, 131_072);
        assert_eq!(bs.record_size, 4096);
    }

    #[test]
    fn rejects_non_ntfs() {
        let mut b = boot(512, 8, 4, -10);
        b[3..11].copy_from_slice(b"MSDOS5.0");
        assert!(matches!(parse_boot_sector(&b), Err(NtfsError::NotNtfs)));
    }

    #[test]
    fn rejects_impossible_sector_size() {
        assert!(matches!(
            parse_boot_sector(&boot(500, 8, 4, -10)),
            Err(NtfsError::BadBootSector(_))
        ));
    }

    #[test]
    fn rejects_short_buffer() {
        assert!(parse_boot_sector(&[0u8; 100]).is_err());
    }
}
