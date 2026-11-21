//! `cargo run -p suny-core --example probe`
//! Mostra o que o núcleo enxerga nesta máquina. Não altera nada.

fn main() {
    match suny_core::steam::find_installations() {
        Ok(list) if list.is_empty() => println!("Steam encontrado, mas o TF2 não está instalado."),
        Ok(list) => {
            for i in &list {
                println!("TF2: {}  (build {:?}, via {:?})", i.root.display(), i.build_id, i.source);
            }
        }
        Err(e) => println!("Steam: {e}"),
    }
    let root = suny_core::steam::find_installations().ok().and_then(|l| l.into_iter().next()).map(|i| i.root);
    let snap = suny_core::system::snapshot(root.as_deref());
    println!("{}", serde_json::to_string_pretty(&snap).unwrap());
}
