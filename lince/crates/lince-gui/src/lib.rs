//! Lógica de la ventana de Lince que no depende de Windows: textos de la
//! lista, motor de indexación/búsqueda en segundo plano y clic derecho del
//! Explorador.

mod args;
mod config;
mod engine;
mod explorer;
mod model;
mod startup;

pub use args::{
    parse_args, quote_arg, relaunch_params, GuiArgs, Relaunch, SetupAction, RELAUNCH_FLAG,
    REPLACE_FLAG,
};
pub use config::{add_folder, covering_source, load_folders, save_folders, with_folders};
pub use engine::{Client, ContentStatus, Engine, Event, Indexes, Results};
pub use explorer::{
    clean_shell_path, command_exe, drive_root, is_within, menu_command, MenuEntry, FOLDER_FLAG,
    MENU_ENTRIES,
};
pub use model::{cell_text, format_size, Column, COLUMNS};
pub use startup::{task_exe, task_xml, STARTUP_FLAG, TASK_NAME};
