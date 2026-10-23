//! Lince.exe: buscador de archivos con ventana de Windows.

// Sin consola detrás de la ventana.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod win;

fn main() {
    #[cfg(windows)]
    win::run();
    #[cfg(not(windows))]
    eprintln!("La ventana de Lince solo funciona en Windows; use el programa `lince-cli`.");
}
