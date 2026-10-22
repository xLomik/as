//! Memoria máxima usada por el proceso (para `--stats`).

/// Pico del conjunto de trabajo (Windows) o de memoria residente (Linux).
#[cfg(windows)]
pub fn peak_memory_bytes() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    // SAFETY: estructura C de enteros; todo ceros es un valor válido.
    let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: estructura con su tamaño en `cb`; el pseudo-handle del proceso
    // actual no hay que cerrarlo.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    (ok != 0).then_some(c.PeakWorkingSetSize as u64)
}

#[cfg(target_os = "linux")]
pub fn peak_memory_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn peak_memory_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn reports_some_peak_memory() {
        let big = vec![1u8; 32 << 20];
        let peak = peak_memory_bytes().expect("memoria pico");
        assert!(peak >= 32 << 20, "{peak}");
        drop(big);
    }
}
