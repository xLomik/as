//! Acceso a volúmenes en Windows: lista de volúmenes NTFS, lectura cruda del
//! volumen (`\\.\C:`, requiere administrador) y tipo de disco.

use std::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::ptr::{self, NonNull};

use lince_ntfs::ReadAt;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, HANDLE};
use windows_sys::Win32::Storage::FileSystem::{
    BusTypeNvme, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW, ReadFile,
    FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE,
};
use windows_sys::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, StorageDeviceSeekPenaltyProperty,
    DEVICE_SEEK_PENALTY_DESCRIPTOR, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_DESCRIPTOR,
    STORAGE_PROPERTY_QUERY,
};
use windows_sys::Win32::System::Threading::CreateEventW;
use windows_sys::Win32::System::IO::{DeviceIoControl, GetOverlappedResult, OVERLAPPED};

use crate::DriveKind;

const DRIVE_FIXED: u32 = 3;
/// Los *handles* de volumen exigen desplazamientos, longitudes y buffers
/// alineados al sector; 4096 sirve para sectores de 512 y de 4096 bytes.
const ALIGN: usize = 4096;
/// `ReadFile` lee como mucho `u32::MAX`; partimos en trozos alineados de 1 GiB.
const MAX_READ: usize = 1 << 30;

/// Volúmenes locales fijos con sistema de archivos NTFS (`C:`, `D:`...).
pub fn ntfs_volumes() -> Vec<String> {
    // SAFETY: sin argumentos; devuelve una máscara de bits.
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| format!("{}:", (b'A' + i) as char))
        .filter(|v| is_fixed_ntfs(v))
        .collect()
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn is_fixed_ntfs(volume: &str) -> bool {
    let root = wide(&format!("{volume}\\"));
    // SAFETY: `root` termina en 0 y vive durante la llamada.
    if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
        return false;
    }
    let mut fs_name = [0u16; 32];
    // SAFETY: buffers válidos con su tamaño; los punteros nulos son opcionales.
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            fs_name.as_mut_ptr(),
            fs_name.len() as u32,
        )
    };
    let len = fs_name.iter().position(|&c| c == 0).unwrap_or(0);
    ok != 0 && String::from_utf16_lossy(&fs_name[..len]) == "NTFS"
}

fn device_path(volume: &str) -> String {
    format!(r"\\.\{}", volume.trim_end_matches(['\\', '/']))
}

/// Volumen abierto para lectura cruda con E/S superpuesta: varios hilos
/// pueden leer a la vez sobre el mismo *handle* sin serializarse.
pub struct Volume {
    file: File,
}

impl Volume {
    /// Abre `C:` (o la letra que sea). Sin administrador falla con
    /// `PermissionDenied`.
    pub fn open(volume: &str) -> io::Result<Volume> {
        let file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OVERLAPPED)
            .open(device_path(volume))?;
        Ok(Volume { file })
    }

    fn handle(&self) -> HANDLE {
        self.file.as_raw_handle() as HANDLE
    }

    fn read_once(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let len = buf.len().min(MAX_READ) as u32;
        // SAFETY: el evento y la estructura OVERLAPPED viven hasta que
        // GetOverlappedResult (con espera) confirma que la lectura terminó;
        // `buf` es válido para `len` bytes durante todo ese tiempo.
        unsafe {
            let event = CreateEventW(ptr::null(), 1, 0, ptr::null());
            if event.is_null() {
                return Err(io::Error::last_os_error());
            }
            let mut ov: OVERLAPPED = std::mem::zeroed();
            ov.Anonymous.Anonymous.Offset = offset as u32;
            ov.Anonymous.Anonymous.OffsetHigh = (offset >> 32) as u32;
            ov.hEvent = event;
            let mut read = 0u32;
            let started = ReadFile(
                self.handle(),
                buf.as_mut_ptr(),
                len,
                ptr::null_mut(),
                &mut ov,
            );
            // Si ReadFile falla de verdad no se llama a GetOverlappedResult y el
            // último error sigue siendo el de ReadFile.
            let in_flight = started != 0
                || io::Error::last_os_error().raw_os_error() == Some(ERROR_IO_PENDING as i32);
            let result = if in_flight && GetOverlappedResult(self.handle(), &ov, &mut read, 1) != 0
            {
                Ok(read as usize)
            } else {
                Err(io::Error::last_os_error())
            };
            CloseHandle(event);
            result
        }
    }
}

impl ReadAt for Volume {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let align = ALIGN as u64;
        let start = offset & !(align - 1);
        let end = (offset + buf.len() as u64).div_ceil(align) * align;
        let mut tmp = AlignedBuf::new((end - start) as usize);
        let data = tmp.as_mut_slice();
        let mut done = 0;
        while done < data.len() {
            match self.read_once(start + done as u64, &mut data[done..])? {
                0 => return Err(io::ErrorKind::UnexpectedEof.into()),
                n => done += n,
            }
        }
        let skip = (offset - start) as usize;
        buf.copy_from_slice(&data[skip..skip + buf.len()]);
        Ok(())
    }
}

/// Buffer alineado a 4096 bytes.
struct AlignedBuf {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl AlignedBuf {
    fn new(len: usize) -> AlignedBuf {
        let layout = Layout::from_size_align(len.max(1), ALIGN).expect("tamaño de buffer válido");
        // SAFETY: layout de tamaño > 0.
        let raw = unsafe { alloc_zeroed(layout) };
        let ptr = NonNull::new(raw).unwrap_or_else(|| handle_alloc_error(layout));
        AlignedBuf { ptr, layout }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: `ptr` apunta a `layout.size()` bytes inicializados a cero y
        // propiedad exclusiva de este buffer.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.layout.size()) }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        // SAFETY: mismo puntero y layout que en `alloc_zeroed`.
        unsafe { dealloc(self.ptr.as_ptr(), self.layout) }
    }
}

/// Tipo de disco físico bajo el volumen. No requiere administrador.
pub fn drive_kind(volume: &str) -> DriveKind {
    let Ok(file) = OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(device_path(volume))
    else {
        return DriveKind::Unknown;
    };
    let handle = file.as_raw_handle() as HANDLE;
    let mut device = [0u8; 1024];
    if query_property(handle, StorageDeviceProperty, &mut device) {
        // SAFETY: el buffer es mayor que la estructura y la llamada lo llenó;
        // read_unaligned evita suponer alineación del arreglo de bytes.
        let desc: STORAGE_DEVICE_DESCRIPTOR =
            unsafe { ptr::read_unaligned(device.as_ptr().cast()) };
        if desc.BusType == BusTypeNvme {
            return DriveKind::Nvme;
        }
    }
    let mut seek = [0u8; std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>()];
    if query_property(handle, StorageDeviceSeekPenaltyProperty, &mut seek) {
        // SAFETY: ídem.
        let desc: DEVICE_SEEK_PENALTY_DESCRIPTOR =
            unsafe { ptr::read_unaligned(seek.as_ptr().cast()) };
        return if desc.IncursSeekPenalty {
            DriveKind::Hdd
        } else {
            DriveKind::Ssd
        };
    }
    DriveKind::Unknown
}

fn query_property(handle: HANDLE, property: i32, out: &mut [u8]) -> bool {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: property,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut returned = 0u32;
    // SAFETY: entrada y salida son buffers válidos con sus tamaños; handle
    // síncrono, por eso OVERLAPPED es nulo.
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const STORAGE_PROPERTY_QUERY).cast(),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            out.as_mut_ptr().cast(),
            out.len() as u32,
            &mut returned,
            ptr::null_mut(),
        )
    };
    ok != 0 && returned as usize >= 8
}
