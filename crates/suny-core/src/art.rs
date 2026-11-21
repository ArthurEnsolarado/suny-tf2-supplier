//! Arte do jogo (banner, logo, capa), lida **da instalação do Steam do próprio usuário**.
//!
//! O projeto não distribui nenhuma arte da Valve nem de terceiros: o app só lê, em tempo de
//! execução, o que o Steam já guarda no computador. Capas personalizadas que o usuário tenha
//! posto no Steam (por exemplo, vindas do SteamGridDB) têm prioridade sobre as oficiais.
//!
//! Só lê nomes de arquivo fixos, em pastas conhecidas. Nenhum caminho vem da interface.

use std::fs;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;

/// Imagens maiores que isso são ignoradas (viram data URL na memória da interface).
const MAX_BYTES: u64 = 6 * 1024 * 1024;
const EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];

/// Cada campo é uma data URL pronta para usar em `<img>` ou `background-image`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ArtSet {
    /// Banner largo e nítido.
    pub hero: Option<String>,
    /// Versão desfocada do banner (o Steam guarda uma, 192x62). Serve de fundo da janela.
    pub hero_blur: Option<String>,
    pub logo: Option<String>,
    /// Capa horizontal pequena.
    pub header: Option<String>,
    /// Capa vertical (pôster).
    pub portrait: Option<String>,
    /// `true` se alguma imagem veio de uma capa personalizada do usuário.
    pub custom: bool,
}

fn mime(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}

fn first_existing(dir: &Path, stems: &[String]) -> Option<PathBuf> {
    stems
        .iter()
        .flat_map(|s| EXTS.iter().map(move |e| dir.join(format!("{s}.{e}"))))
        .find(|p| p.is_file())
}

fn data_url(path: &Path, max_bytes: u64) -> Option<String> {
    let len = fs::metadata(path).ok()?.len();
    if len == 0 || len > max_bytes {
        return None;
    }
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let bytes = fs::read(path).ok()?;
    Some(format!("data:{};base64,{}", mime(&ext), STANDARD.encode(bytes)))
}

/// Uma vaga de arte: nomes das capas personalizadas e nomes do cache oficial do Steam.
struct Slot {
    custom: Vec<String>,
    cache: Vec<String>,
}

pub fn find_art(steam_dir: &Path, app_id: u32) -> ArtSet {
    find_art_with_limit(steam_dir, app_id, MAX_BYTES)
}

fn find_art_with_limit(steam_dir: &Path, app_id: u32, max_bytes: u64) -> ArtSet {
    let id = app_id.to_string();
    let cache_dir = steam_dir.join("appcache").join("librarycache").join(&id);

    // pastas de capas personalizadas de todas as contas do Steam deste computador
    let grids: Vec<PathBuf> = fs::read_dir(steam_dir.join("userdata"))
        .map(|rd| rd.flatten().map(|e| e.path().join("config").join("grid")).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();

    let slot = |custom: &[String], cache: &[&str]| Slot {
        custom: custom.to_vec(),
        cache: cache.iter().map(|s| s.to_string()).collect(),
    };

    let mut custom_used = false;
    let mut pick = |s: Slot| -> Option<String> {
        for g in &grids {
            if let Some(url) = first_existing(g, &s.custom).and_then(|p| data_url(&p, max_bytes)) {
                custom_used = true;
                return Some(url);
            }
        }
        first_existing(&cache_dir, &s.cache).and_then(|p| data_url(&p, max_bytes))
    };

    let hero = pick(slot(&[format!("{id}_hero")], &["library_hero"]));
    // Capas personalizadas não têm versão desfocada; sem ela o app borra o banner por CSS.
    let hero_blur = first_existing(&cache_dir, &["library_hero_blur".to_string()]).and_then(|p| data_url(&p, max_bytes));
    let logo = pick(slot(&[format!("{id}_logo")], &["logo"]));
    let header = pick(slot(&[id.clone()], &["header"]));
    let portrait = pick(slot(&[format!("{id}p")], &["library_600x900"]));

    ArtSet { hero, hero_blur, logo, header, portrait, custom: custom_used }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("suny_art_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn put(path: PathBuf, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn reads_the_official_steam_cache() {
        let s = tmp("cache");
        put(s.join("appcache/librarycache/440/library_hero.jpg"), b"hero");
        put(s.join("appcache/librarycache/440/logo.png"), b"logo");
        put(s.join("appcache/librarycache/440/library_hero_blur.jpg"), b"blur");
        let a = find_art(&s, 440);
        assert!(a.hero_blur.as_deref().unwrap().starts_with("data:image/jpeg;base64,"));
        assert!(a.hero.as_deref().unwrap().starts_with("data:image/jpeg;base64,"));
        assert!(a.logo.as_deref().unwrap().starts_with("data:image/png;base64,"));
        assert!(a.header.is_none() && a.portrait.is_none());
        assert!(!a.custom);
        let _ = fs::remove_dir_all(&s);
    }

    #[test]
    fn custom_grid_wins_over_official_art() {
        let s = tmp("custom");
        put(s.join("appcache/librarycache/440/library_hero.jpg"), b"oficial");
        put(s.join("userdata/111/config/grid/440_hero.png"), b"personalizada");
        let a = find_art(&s, 440);
        let url = a.hero.unwrap();
        assert!(url.starts_with("data:image/png;base64,"), "preferiu a personalizada: {url}");
        assert_eq!(STANDARD.decode(url.split(',').nth(1).unwrap()).unwrap(), b"personalizada");
        assert!(a.custom);
        let _ = fs::remove_dir_all(&s);
    }

    #[test]
    fn other_apps_and_oversized_files_are_ignored() {
        let s = tmp("limits");
        put(s.join("appcache/librarycache/730/library_hero.jpg"), b"outro jogo");
        put(s.join("appcache/librarycache/440/logo.png"), &vec![1u8; 200]);
        let a = find_art_with_limit(&s, 440, 100);
        assert!(a.hero.is_none(), "não pode pegar arte de outro jogo");
        assert!(a.logo.is_none(), "arquivo acima do limite é ignorado");
        let _ = fs::remove_dir_all(&s);
    }

    #[test]
    fn missing_steam_dir_is_just_empty() {
        let a = find_art(Path::new("Z:/nao/existe"), 440);
        assert!(a.hero.is_none() && a.logo.is_none() && !a.custom);
    }
}
