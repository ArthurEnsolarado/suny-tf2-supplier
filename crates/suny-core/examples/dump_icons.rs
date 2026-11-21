//! Ferramenta de desenvolvimento: converte texturas do VPK do TF2 em PNG para inspeção visual.
//!
//! `cargo run -p suny-core --example dump_icons -- <pasta tf> <pasta de saída> <caminho no VPK>...`
//! Só lê o jogo; escreve só na pasta de saída.

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("uso: dump_icons <pasta tf> <pasta de saída> <caminho no vpk>...");
        std::process::exit(2);
    }
    let (tf, out) = (std::path::Path::new(&a[1]), std::path::Path::new(&a[2]));
    std::fs::create_dir_all(out).unwrap();
    for vpk_path in &a[3..] {
        match suny_core::gameicons::extract_png(tf, vpk_path) {
            Ok(png) => {
                let name = vpk_path.rsplit('/').next().unwrap().replace(".vtf", ".png");
                std::fs::write(out.join(&name), &png).unwrap();
                println!("{name}: {} bytes", png.len());
            }
            Err(e) => println!("{vpk_path}: ERRO {e}"),
        }
    }
}
