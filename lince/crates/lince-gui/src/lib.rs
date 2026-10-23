//! Lógica de la ventana de Lince que no depende de Windows: textos de la
//! lista y motor de indexación/búsqueda en segundo plano.

mod args;
mod config;
mod engine;
mod model;

pub use args::{parse_args, quote_arg, relaunch_params, GuiArgs, RELAUNCH_FLAG};
pub use config::{add_folder, load_folders, save_folders, with_folders};
pub use engine::{Engine, Event, Indexes, Results};
pub use model::{cell_text, format_size, Column, COLUMNS};
