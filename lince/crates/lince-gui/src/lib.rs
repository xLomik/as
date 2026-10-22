//! Lógica de la ventana de Lince que no depende de Windows: textos de la
//! lista y motor de indexación/búsqueda en segundo plano.

mod engine;
mod model;

pub use engine::{Engine, Event, Results};
pub use model::{cell_text, format_size, Column, COLUMNS};
