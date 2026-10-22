//! Conversión entre FILETIME y fechas en hora local.

use chrono::{Local, LocalResult, TimeZone};

use crate::index::EPOCH_DIFF_SECS;

const TICKS_PER_SEC: u64 = 10_000_000;

/// FILETIME de una fecha y hora local. `None` si la fecha no existe.
pub fn filetime_from_local(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
) -> Option<u64> {
    let dt = match Local.with_ymd_and_hms(year, month, day, hour, min, sec) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
        LocalResult::None => return None,
    };
    unix_to_filetime(dt.timestamp())
}

/// Comienzo del día local; si la medianoche no existe (cambio de horario),
/// la primera hora válida.
pub(crate) fn local_day_start(year: i32, month: u32, day: u32) -> Option<u64> {
    (0..3).find_map(|h| filetime_from_local(year, month, day, h, 0, 0))
}

fn unix_to_filetime(secs: i64) -> Option<u64> {
    u64::try_from(secs.checked_add(EPOCH_DIFF_SECS)?)
        .ok()?
        .checked_mul(TICKS_PER_SEC)
}

/// `AAAA-MM-DD hh:mm:ss` en hora local; vacío si la fecha es 0 o inválida.
pub fn format_filetime_local(ft: u64) -> String {
    if ft == 0 {
        return String::new();
    }
    let secs = (ft / TICKS_PER_SEC) as i64 - EPOCH_DIFF_SECS;
    match Local.timestamp_opt(secs, 0) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => {
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        }
        LocalResult::None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_local_time() {
        let ft = filetime_from_local(2026, 10, 22, 14, 5, 9).unwrap();
        assert_eq!(format_filetime_local(ft), "2026-10-22 14:05:09");
    }

    #[test]
    fn invalid_dates_are_none() {
        assert_eq!(filetime_from_local(2026, 2, 30, 0, 0, 0), None);
        assert_eq!(format_filetime_local(0), "");
    }
}
