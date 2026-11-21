//! O que está instalado hoje na pasta `tf/custom` e se o Casual Preloader está aplicado.
//! Só leitura: nada aqui altera o jogo.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::steam::Installation;
use crate::{Error, Result};

/// Papel de cada item dentro de `custom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// VPK gerado pelo Casual Preloader (`_casual_preloader_*`, `_QuickPrecache`).
    Preloader,
    /// Pasta que o preloader deixa para guardar o menu original (`zz_run_preloader_again...`).
    PreloaderBackup,
    /// Pasta `workshop`, criada pelo próprio jogo.
    Workshop,
    /// Qualquer outra coisa: mod instalado à mão.
    Mod,
}

/// Dados de um mod instalado por este app (vêm do manifesto).
#[derive(Debug, Clone, Serialize)]
pub struct ManagedInfo {
    pub slug: String,
    pub gb_id: u64,
    pub kind: String,
    pub name: String,
    pub version: Option<String>,
    pub submitter: Option<String>,
    pub profile_url: String,
    pub installed_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomEntry {
    pub name: String,
    pub role: Role,
    pub is_folder: bool,
    pub size_bytes: u64,
    pub file_count: usize,
    /// `Some` se foi instalado por este app (e pode ser removido por ele).
    pub managed: Option<ManagedInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreloaderState {
    /// `gameinfo.txt` com a linha `type multiplayer_only` comentada (marca do preloader).
    pub gameinfo_modified: bool,
    /// Existe `custom/_casual_preloader_dir.vpk`.
    pub vpk_present: bool,
}

impl PreloaderState {
    /// Preloader aplicado de verdade: os dois sinais juntos.
    pub fn applied(&self) -> bool {
        self.gameinfo_modified && self.vpk_present
    }
    /// Só um dos sinais: sobra de uma instalação antiga ou instalação incompleta.
    pub fn leftover(&self) -> bool {
        self.gameinfo_modified != self.vpk_present
    }
}

/// Mesma regra do preloader (`check_game_type`): uma linha com `type`, `multiplayer_only` e `//`.
pub fn gameinfo_modified(gameinfo: &Path) -> bool {
    fs::read_to_string(gameinfo)
        .map(|t| t.lines().any(|l| l.contains("type") && l.contains("multiplayer_only") && l.contains("//")))
        .unwrap_or(false)
}

pub fn preloader_state(install: &Installation) -> PreloaderState {
    PreloaderState {
        gameinfo_modified: gameinfo_modified(&install.gameinfo_path()),
        vpk_present: install.custom_dir().join("_casual_preloader_dir.vpk").is_file(),
    }
}

fn role_of(name: &str, is_folder: bool) -> Role {
    let l = name.to_ascii_lowercase();
    if l.starts_with("_casual_preloader") || l.starts_with("_quickprecache") {
        Role::Preloader
    } else if is_folder && l.starts_with("zz_run_preloader_again") {
        Role::PreloaderBackup
    } else if is_folder && l == "workshop" {
        Role::Workshop
    } else {
        Role::Mod
    }
}

fn walk(dir: &Path, bytes: &mut u64, files: &mut usize) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            walk(&e.path(), bytes, files);
        } else {
            *bytes += meta.len();
            *files += 1;
        }
    }
}

/// Lista o que há em `custom`. Arquivos `.cache` soltos e `.vpk.sound.cache` são ignorados (o jogo recria).
pub fn scan_custom(custom_dir: &Path) -> Result<Vec<CustomEntry>> {
    if !custom_dir.is_dir() {
        return Ok(Vec::new());
    }
    let managed: std::collections::HashMap<String, ManagedInfo> = crate::installer::list_managed(custom_dir)
        .into_iter()
        .flat_map(|m| {
            let info = ManagedInfo {
                slug: m.slug.clone(),
                gb_id: m.meta.gb_id,
                kind: m.meta.kind.clone(),
                name: m.meta.name.clone(),
                version: m.meta.version.clone(),
                submitter: m.meta.submitter.clone(),
                profile_url: m.meta.profile_url.clone(),
                installed_at: m.installed_at,
            };
            m.items.into_iter().map(move |item| (item, info.clone()))
        })
        .collect();

    let mut out = Vec::new();
    for e in fs::read_dir(custom_dir).map_err(|e| Error::io(custom_dir, e))?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.to_ascii_lowercase().ends_with(".cache") || name == crate::installer::MANIFEST_DIR {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        let is_folder = meta.is_dir();
        let (mut size_bytes, mut file_count) = (0u64, 0usize);
        if is_folder {
            walk(&e.path(), &mut size_bytes, &mut file_count);
        } else {
            size_bytes = meta.len();
            file_count = 1;
        }
        let managed = managed.get(&name).cloned();
        out.push(CustomEntry { role: role_of(&name, is_folder), name, is_folder, size_bytes, file_count, managed });
    }
    out.sort_by(|a, b| (a.role as u8, a.name.to_lowercase()).cmp(&(b.role as u8, b.name.to_lowercase())));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("suny_lib_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn detects_preloader_marker_in_gameinfo() {
        let d = tmp("gi");
        let f = d.join("gameinfo.txt");
        fs::write(&f, "\"GameInfo\"\n{\n\tgame TF\n\t//type multiplayer_only\n}").unwrap();
        assert!(gameinfo_modified(&f));
        fs::write(&f, "\"GameInfo\"\n{\n\ttype multiplayer_only\n}").unwrap();
        assert!(!gameinfo_modified(&f));
        assert!(!gameinfo_modified(&d.join("nao_existe.txt")));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn state_applied_and_leftover() {
        assert!(PreloaderState { gameinfo_modified: true, vpk_present: true }.applied());
        let left = PreloaderState { gameinfo_modified: true, vpk_present: false };
        assert!(!left.applied() && left.leftover());
        let clean = PreloaderState { gameinfo_modified: false, vpk_present: false };
        assert!(!clean.applied() && !clean.leftover());
    }

    #[test]
    fn scans_roles_sizes_and_ignores_cache_files() {
        let d = tmp("scan");
        fs::create_dir_all(d.join("Pink Charge/sound/weapons")).unwrap();
        fs::write(d.join("Pink Charge/sound/weapons/a.wav"), vec![0u8; 100]).unwrap();
        fs::write(d.join("Pink Charge/readme.txt"), b"x").unwrap();
        fs::create_dir_all(d.join("workshop")).unwrap();
        fs::create_dir_all(d.join("zz_run_preloader_again_if_you_add_a_custom_hud")).unwrap();
        fs::write(d.join("_casual_preloader_dir.vpk"), vec![0u8; 10]).unwrap();
        fs::write(d.join("_casual_preloader_dir.vpk.sound.cache"), b"c").unwrap();
        fs::write(d.join("hud.vpk"), vec![0u8; 5]).unwrap();

        let list = scan_custom(&d).unwrap();
        let by = |n: &str| list.iter().find(|e| e.name == n).unwrap();
        assert_eq!(by("Pink Charge").role, Role::Mod);
        assert_eq!(by("Pink Charge").size_bytes, 101);
        assert_eq!(by("Pink Charge").file_count, 2);
        assert_eq!(by("workshop").role, Role::Workshop);
        assert_eq!(by("zz_run_preloader_again_if_you_add_a_custom_hud").role, Role::PreloaderBackup);
        assert_eq!(by("_casual_preloader_dir.vpk").role, Role::Preloader);
        assert!(list.iter().all(|e| !e.name.ends_with(".cache")));
        assert_eq!(list[0].role, Role::Preloader, "preloader primeiro na ordenação");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_custom_dir_is_empty_not_error() {
        assert!(scan_custom(Path::new("Z:/nao/existe/custom")).unwrap().is_empty());
    }
}
