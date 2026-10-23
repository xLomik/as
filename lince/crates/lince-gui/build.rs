//! Incrusta icono, manifiesto y datos de versión en Lince.exe.

fn main() {
    println!("cargo:rerun-if-changed=res");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("res/lince.rc", embed_resource::NONE)
            .manifest_required()
            .expect("no se pudieron compilar los recursos de Windows");
    }
}
