//! Índice dos arquivos ORIGINAIS do jogo, lido dos `*_dir.vpk` da instalação do usuário.
//!
//! É o que permite dizer "este arquivo do mod substitui um arquivo do jogo" (caminho existe no TF2)
//! ou "é novo" (não existe), e calcular o selo de compatibilidade com o casual.
//! Só lê nomes de arquivo; nenhum dado do jogo é copiado ou distribuído.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use crate::gameicons::list_paths;
use crate::scanner::VanillaIndex;

/// Pastas do jogo que um mod costuma tocar. Mapas ficam de fora: são grandes e mods não os substituem.
const ROOTS: &[&str] = &["sound/", "materials/", "models/", "particles/", "scripts/", "resource/", "cfg/"];

fn dir_vpks(tf_dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(tf_dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("tf2_") && n.ends_with("_dir.vpk"))
        })
        .collect();
    v.sort();
    v
}

/// Muda quando o jogo é atualizado (data ou tamanho de algum `_dir.vpk`).
fn signature(files: &[PathBuf]) -> u128 {
    files
        .iter()
        .filter_map(|f| std::fs::metadata(f).ok())
        .map(|m| m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos()) ^ m.len() as u128)
        .fold(0u128, |a, b| a.wrapping_add(b))
}

/// Lê todos os `tf2_*_dir.vpk` da pasta `tf/`. VPK ilegível é ignorado (o índice fica menor, nunca errado).
pub fn load_index(tf_dir: &Path) -> VanillaIndex {
    let mut all: Vec<String> = Vec::new();
    for f in dir_vpks(tf_dir) {
        if let Ok(paths) = list_paths(&f, |k| ROOTS.iter().any(|r| k.starts_with(r))) {
            all.extend(paths);
        }
    }
    VanillaIndex::from_paths(all)
}

static CACHE: Mutex<Option<(PathBuf, u128, Arc<VanillaIndex>)>> = Mutex::new(None);

/// Como `load_index`, mas guarda o resultado na memória (a leitura leva alguns segundos).
/// Refaz sozinho se o jogo foi atualizado.
pub fn index_for(tf_dir: &Path) -> Arc<VanillaIndex> {
    let files = dir_vpks(tf_dir);
    let sig = signature(&files);
    if let Ok(guard) = CACHE.lock() {
        if let Some((dir, s, idx)) = guard.as_ref() {
            if dir == tf_dir && *s == sig {
                return Arc::clone(idx);
            }
        }
    }
    let idx = Arc::new(load_index(tf_dir));
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((tf_dir.to_path_buf(), sig, Arc::clone(&idx)));
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameicons::tests::{make_vpk, tmp};

    #[test]
    fn indexes_original_paths_from_dir_vpks_and_ignores_other_roots() {
        let d = tmp("vanilla");
        make_vpk(&d, &[("sound/ui", "hitsound", b"x"), ("sound/vo", "demoman_battlecry01", b"y"), ("maps", "cp_dustbowl", b"z")], false);
        let idx = load_index(&d);
        assert!(idx.contains("sound/ui/hitsound.vtf"));
        assert!(idx.contains("Sound\\VO\\demoman_battlecry01.vtf"), "busca ignora caixa e barra");
        assert!(!idx.contains("maps/cp_dustbowl.vtf"), "mapas não entram no índice");
        assert!(!idx.contains("sound/ui/novo.vtf"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Teste REAL com o seu TF2 (só leitura): `$env:SUNY_LIVE_TF2_DIR='<pasta tf>'; cargo test -p suny-core live_vanilla -- --ignored --nocapture`
    #[test]
    #[ignore = "lê a instalação real do TF2 (SUNY_LIVE_TF2_DIR)"]
    fn live_vanilla_index_from_the_real_game() {
        let tf = std::env::var("SUNY_LIVE_TF2_DIR").expect("defina SUNY_LIVE_TF2_DIR com a pasta tf do jogo");
        let t0 = std::time::Instant::now();
        let idx = load_index(Path::new(&tf));
        println!("índice montado em {:?}", t0.elapsed());
        let t1 = std::time::Instant::now();
        let again = index_for(Path::new(&tf));
        let cached = index_for(Path::new(&tf));
        println!("primeira chamada do cache: {:?}; segunda: {:?}", t1.elapsed(), std::time::Instant::now().duration_since(t1));
        assert!(Arc::ptr_eq(&again, &cached));
        for p in [
            "sound/ui/hitsound.wav",
            "sound/ui/killsound.wav",
            "sound/weapons/demo_charge_windup1.wav",
            "sound/weapons/demo_sword_swing1.wav",
            "sound/vo/demoman_battlecry01.mp3",
            "sound/vo/taunts/demoman_taunts01.mp3",
            "models/player/demo.mdl",
            "materials/vgui/class_icons/filter_demo.vtf",
        ] {
            println!("  {:<48} {}", p, if idx.contains(p) { "existe no jogo" } else { "NÃO existe" });
        }
        assert!(idx.contains("sound/ui/hitsound.wav"), "o hitsound original precisa estar no índice");
        assert!(idx.contains("models/player/demo.mdl"));
    }

    #[test]
    fn missing_or_unreadable_game_gives_an_empty_index() {
        assert!(load_index(Path::new("Z:/nao/existe")).is_empty());
        let d = tmp("vanilla_bad");
        std::fs::write(d.join("tf2_misc_dir.vpk"), b"isto nao e um vpk").unwrap();
        assert!(load_index(&d).is_empty(), "VPK corrompido é ignorado sem derrubar nada");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cache_is_reused_until_the_game_files_change() {
        let d = tmp("vanilla_cache");
        make_vpk(&d, &[("sound/ui", "hitsound", b"x")], false);
        let a = index_for(&d);
        let b = index_for(&d);
        assert!(Arc::ptr_eq(&a, &b), "segunda chamada reaproveita a memória");

        // o jogo "atualiza": o VPK muda de tamanho
        std::fs::remove_file(d.join("tf2_textures_dir.vpk")).unwrap();
        make_vpk(&d, &[("sound/ui", "hitsound", b"x"), ("sound/ui", "killsound", b"yy")], false);
        let c = index_for(&d);
        assert!(!Arc::ptr_eq(&a, &c));
        assert!(c.contains("sound/ui/killsound.vtf"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
