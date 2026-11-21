//! Ferramenta de desenvolvimento: instala um .zip LOCAL numa pasta `custom`, como o app faria.
//!
//! `cargo run -p suny-core --example install_local -- <zip> <pasta custom> <nome> <gb_id>`
//!
//! Serve para testar o instalador e a tela "Instalado" sem baixar nada. Use só em cópias de teste.

use suny_core::installer::{install_zip, ModMeta};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 5 {
        eprintln!("uso: install_local <zip> <pasta custom> <nome> <gb_id>");
        std::process::exit(2);
    }
    let meta = ModMeta {
        gb_id: a[4].parse().expect("gb_id numérico"),
        kind: "Sound".into(),
        file_id: 0,
        file_name: "local.zip".into(),
        name: a[3].clone(),
        version: Some("0.0-teste".into()),
        submitter: Some("Autor de teste".into()),
        profile_url: "https://gamebanana.com/".into(),
        license: None,
        md5: None,
    };
    match install_zip(std::path::Path::new(&a[1]), std::path::Path::new(&a[2]), meta) {
        Ok(o) => println!("instalado: {} ({} arquivos, {} ignorados)", o.manifest.slug, o.manifest.files.len(), o.manifest.skipped.len()),
        Err(e) => {
            eprintln!("erro: {e}");
            std::process::exit(1);
        }
    }
}
