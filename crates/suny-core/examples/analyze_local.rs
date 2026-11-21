//! Ferramenta de desenvolvimento: analisa um .zip LOCAL contra os arquivos originais de um TF2 (só leitura).
//!
//! `cargo run -p suny-core --example analyze_local -- <zip> <pasta tf> [entrada-de-áudio]`
//!
//! Sem o 3º argumento imprime a análise em JSON. Com ele, imprime o JSON da prévia daquele áudio.

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("uso: analyze_local <zip> <pasta tf> [entrada-de-áudio]");
        std::process::exit(2);
    }
    let zip = Path::new(&a[1]);
    if let Some(entry) = a.get(3) {
        match suny_core::analysis::read_audio(zip, entry) {
            Ok((mime, bytes)) => {
                use base64::{engine::general_purpose::STANDARD, Engine};
                println!("{}", serde_json::json!({ "mime": mime, "data_url": format!("data:{mime};base64,{}", STANDARD.encode(bytes)) }));
            }
            Err(e) => {
                eprintln!("erro: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let index = suny_core::vanilla::index_for(Path::new(&a[2]));
    match suny_core::analysis::analyze_zip(zip, Some(&index)) {
        Ok(an) => println!("{}", serde_json::to_string(&an).unwrap()),
        Err(e) => {
            eprintln!("erro: {e}");
            std::process::exit(1);
        }
    }
}
