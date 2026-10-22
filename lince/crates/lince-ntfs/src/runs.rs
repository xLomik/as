use crate::NtfsError;

/// Un tramo contiguo de un atributo no residente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataRun {
    /// Clúster lógico donde empieza; `None` = tramo disperso (*sparse*, ceros).
    pub lcn: Option<u64>,
    pub clusters: u64,
}

/// Decodifica la lista de *mapping pairs* de un atributo no residente.
///
/// Cada tramo empieza con un byte: nibble bajo = bytes de la longitud, nibble
/// alto = bytes del desplazamiento (con signo, relativo al tramo anterior).
/// Un desplazamiento de 0 bytes indica un tramo disperso.
pub fn decode_runs(bytes: &[u8]) -> Result<Vec<DataRun>, NtfsError> {
    let mut runs = Vec::new();
    let mut pos = 0usize;
    let mut lcn: i64 = 0;
    while let Some(&header) = bytes.get(pos) {
        if header == 0 {
            break;
        }
        let len_size = usize::from(header & 0x0F);
        let off_size = usize::from(header >> 4);
        if len_size == 0 || len_size > 8 || off_size > 8 {
            return Err(NtfsError::BadRuns);
        }
        pos += 1;
        let len_bytes = bytes.get(pos..pos + len_size).ok_or(NtfsError::BadRuns)?;
        let clusters = read_unsigned(len_bytes);
        if clusters == 0 {
            return Err(NtfsError::BadRuns);
        }
        pos += len_size;
        if off_size == 0 {
            runs.push(DataRun {
                lcn: None,
                clusters,
            });
            continue;
        }
        let off_bytes = bytes.get(pos..pos + off_size).ok_or(NtfsError::BadRuns)?;
        pos += off_size;
        lcn = lcn
            .checked_add(read_signed(off_bytes))
            .ok_or(NtfsError::BadRuns)?;
        if lcn < 0 {
            return Err(NtfsError::BadRuns);
        }
        runs.push(DataRun {
            lcn: Some(lcn as u64),
            clusters,
        });
    }
    Ok(runs)
}

fn read_unsigned(b: &[u8]) -> u64 {
    b.iter()
        .rev()
        .fold(0u64, |acc, &x| (acc << 8) | u64::from(x))
}

fn read_signed(b: &[u8]) -> i64 {
    let raw = read_unsigned(b);
    let bits = 8 * b.len() as u32;
    if bits == 64 {
        raw as i64
    } else {
        // Extensión de signo desde el último byte.
        let shift = 64 - bits;
        ((raw << shift) as i64) >> shift
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_single_run() {
        // Cabecera 0x31: 1 byte de longitud, 3 de desplazamiento.
        let runs = decode_runs(&[0x31, 0x20, 0x00, 0x10, 0x00, 0x00]).unwrap();
        assert_eq!(
            runs,
            vec![DataRun {
                lcn: Some(0x1000),
                clusters: 0x20
            }]
        );
    }

    #[test]
    fn offsets_are_signed_and_relative() {
        let bytes = [
            0x31, 0x20, 0x00, 0x10, 0x00, // LCN 0x1000
            0x21, 0x10, 0x00, 0xFF, // -0x100 → 0x0F00
            0x00,
        ];
        let runs = decode_runs(&bytes).unwrap();
        assert_eq!(runs[1].lcn, Some(0x0F00));
        assert_eq!(runs[1].clusters, 0x10);
    }

    #[test]
    fn sparse_run_does_not_move_base() {
        let bytes = [
            0x11, 0x04, 0x40, // LCN 0x40
            0x01, 0x08, // disperso, 8 clústeres
            0x11, 0x02, 0x10, // +0x10 desde 0x40 → 0x50
            0x00,
        ];
        let runs = decode_runs(&bytes).unwrap();
        assert_eq!(
            runs,
            vec![
                DataRun {
                    lcn: Some(0x40),
                    clusters: 4
                },
                DataRun {
                    lcn: None,
                    clusters: 8
                },
                DataRun {
                    lcn: Some(0x50),
                    clusters: 2
                },
            ]
        );
    }

    #[test]
    fn stops_at_terminator() {
        let runs = decode_runs(&[0x11, 0x01, 0x05, 0x00, 0xDE, 0xAD]).unwrap();
        assert_eq!(runs.len(), 1);
    }

    #[test]
    fn truncated_run_is_error() {
        assert!(matches!(
            decode_runs(&[0x31, 0x20, 0x00]),
            Err(NtfsError::BadRuns)
        ));
    }

    #[test]
    fn negative_lcn_is_error() {
        assert!(matches!(
            decode_runs(&[0x11, 0x01, 0xF0, 0x00]),
            Err(NtfsError::BadRuns)
        ));
    }

    #[test]
    fn zero_length_run_is_error() {
        assert!(matches!(
            decode_runs(&[0x11, 0x00, 0x05, 0x00]),
            Err(NtfsError::BadRuns)
        ));
    }
}
