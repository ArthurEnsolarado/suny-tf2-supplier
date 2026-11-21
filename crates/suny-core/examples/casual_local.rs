//! Ferramenta de desenvolvimento: percorre o Casual de ponta a ponta numa CÓPIA do TF2.
//!
//! Uso: `cargo run -p suny-core --example casual_local -- <pasta do casual> <pasta tf/ da CÓPIA> <zip do mod> [apply|revert|both]`
//!
//! Nunca aponte para a instalação real do jogo. A pasta do casual deve ter `preloader/casual-preloader` pronto
//! (para teste, uma cópia do release extraído com as bibliotecas instaladas).

use std::path::PathBuf;
use std::time::Duration;

use suny_core::casual::{self, AddonSource, ApplyOptions, Layout};

async fn wait_job() -> casual::JobView {
    let mut last = String::new();
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let j = casual::job_view().expect("sem tarefa");
        let line = format!("[{:>3}%] {}", j.pct, j.msg);
        if line != last {
            println!("{line}");
            last = line;
        }
        if j.state != "running" {
            return j;
        }
    }
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("uso: casual_local <pasta do casual> <tf/ da copia> <zip> [apply|revert|both]");
        std::process::exit(2);
    }
    let l = Layout::new(&a[1]);
    let tf = PathBuf::from(&a[2]);
    let zip = PathBuf::from(&a[3]);
    let mode = a.get(4).map(String::as_str).unwrap_or("both");
    assert!(tf.join("gameinfo.txt").is_file(), "{} não parece uma pasta tf/", tf.display());

    println!("runtime: {:?} | versão: {:?}", casual::runtime(&l), casual::installed_version(&l));
    println!("addons antes: {:?}", casual::list_addons(&l).iter().map(|x| &x.name).collect::<Vec<_>>());

    let src = AddonSource {
        kind: "Sound".into(),
        gb_id: 0,
        file_id: 0,
        file_name: zip.file_name().unwrap().to_string_lossy().into_owned(),
        name: "mod de teste".into(),
        version: None,
        submitter: None,
        profile_url: String::new(),
        md5: None,
        added_at: 0,
    };
    let z = zip.clone();
    let l2 = l.clone();
    casual::start_job("add", "adicionar", move |rep| casual::add_zip(l2, z, "Mod de teste (gbs1)".to_string(), src, rep)).expect("começar");
    let j = wait_job().await;
    println!("ADD => {} | {:?} | erro: {:?}", j.state, j.result, j.error);
    if j.state != "done" {
        std::process::exit(1);
    }
    let addons = casual::list_addons(&l);
    for x in &addons {
        println!("  addon {:?}: {} arquivos, {} bytes, tipo {}, marcado {}", x.name, x.file_count, x.size_bytes, x.kind, x.enabled);
    }
    let names: Vec<String> = addons.iter().filter(|x| x.enabled).map(|x| x.name.clone()).collect();

    if mode == "apply" || mode == "both" {
        let (l2, tf2, n2) = (l.clone(), tf.clone(), names.clone());
        casual::start_job("apply", "aplicar", move |rep| casual::apply(l2, tf2, n2, ApplyOptions::default(), rep)).expect("começar");
        let j = wait_job().await;
        println!("APPLY => {} em {}s | {:?} | erro: {:?}", j.state, j.elapsed_s, j.result, j.error);
        if j.state != "done" {
            std::process::exit(1);
        }
        println!("status: {}", casual::status(&l, Some(&tf), None)["applied_here"]);
    }
    if mode == "revert" || mode == "both" {
        let (l2, tf2) = (l.clone(), tf.clone());
        casual::start_job("revert", "reverter", move |rep| casual::revert(l2, tf2, rep)).expect("começar");
        let j = wait_job().await;
        println!("REVERT => {} em {}s | {:?} | erro: {:?}", j.state, j.elapsed_s, j.result, j.error);
        if j.state != "done" {
            std::process::exit(1);
        }
    }
}
