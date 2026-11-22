//! Analisa o conteúdo de um mod e calcula o selo de compatibilidade.
//!
//! Trabalha em cima de uma **lista de caminhos**, então qualquer formato de
//! arquivo (zip hoje; 7z/rar depois) só precisa produzir essa lista.

use std::collections::HashSet;
use std::io::{Read, Seek};

use serde::Serialize;

use crate::{Error, Result};

/// Pastas de primeiro nível que o TF2 reconhece.
const GAME_ROOTS: &[&str] = &[
    "models", "materials", "sound", "particles", "scripts", "resource", "cfg", "maps", "expressions",
    "media", "scenes", "ui", "vgui", "bin", "classes", "lua", "panorama", "shaders",
];

/// Extensões que nunca devem entrar num mod de conteúdo.
const BLOCKED_EXT: &[&str] = &["exe", "dll", "bat", "cmd", "ps1", "vbs", "js", "jar", "msi", "scr", "com", "lnk", "sh"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Model,
    Material,
    Sound,
    Particle,
    Hud,
    Config,
    Map,
    Other,
}

impl ContentKind {
    pub fn of(path: &str) -> Self {
        let p = path.to_ascii_lowercase();
        let ext = p.rsplit('.').next().unwrap_or("");
        let root = p.split('/').next().unwrap_or("");
        match (root, ext) {
            (_, "bsp") | ("maps", _) => Self::Map,
            ("particles", _) | (_, "pcf") => Self::Particle,
            ("models", _) | (_, "mdl" | "vvd" | "vtx" | "phy" | "ani") => Self::Model,
            ("materials", _) | (_, "vmt" | "vtf") => Self::Material,
            ("sound", _) | (_, "wav" | "mp3" | "ogg") => Self::Sound,
            ("cfg", _) => Self::Config,
            ("resource" | "scripts" | "ui" | "vgui", _) => Self::Hud,
            _ => Self::Other,
        }
    }
}

/// Selo mostrado ao usuário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compat {
    /// Todos os modelos/sons usam caminhos do jogo original.
    CasualOk,
    /// Parte dos arquivos usa caminhos do jogo, parte não.
    Partial,
    /// Só funciona offline ou em servidores de comunidade.
    OfflineOnly,
    /// Faltou o índice de arquivos do TF2 para decidir.
    NeedsIndex,
    /// Tem arquivo perigoso. Não instalar.
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Flag {
    /// Partículas customizadas: proibidas na RGL (ban de 1 ano, segundo o FAQ do preloader).
    CompetitiveParticles,
    /// Mapa (.bsp): conteúdo empacotado no mapa não passa pelo preloader.
    MapContent,
    /// Configs (.cfg): precisam ficar em `custom`, não no preloader.
    HasConfigs,
    /// HUD: pode ficar em `custom` normalmente.
    HasHud,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub compat: Compat,
    pub reasons: Vec<String>,
    pub flags: Vec<Flag>,
    /// Arquivos bloqueados (executáveis etc.), já com o caminho original.
    pub blocked: Vec<String>,
    /// Modelos/sons que NÃO existem no jogo original.
    pub unmatched: Vec<String>,
    pub counts: Vec<(ContentKind, usize)>,
    pub file_count: usize,
}

/// Índice dos caminhos que existem no TF2 original (minúsculos, com `/`).
#[derive(Debug, Default, Clone)]
pub struct VanillaIndex(HashSet<String>);

impl VanillaIndex {
    pub fn from_paths<I: IntoIterator<Item = S>, S: AsRef<str>>(paths: I) -> Self {
        Self(paths.into_iter().map(|p| norm(p.as_ref())).collect())
    }
    pub fn contains(&self, path: &str) -> bool {
        self.0.contains(&norm(path))
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Caminhos originais agrupados pelo nome do arquivo, só para os nomes pedidos (minúsculos). Uma volta no índice.
    pub fn paths_named(&self, names: &HashSet<String>) -> std::collections::HashMap<String, Vec<String>> {
        let mut out: std::collections::HashMap<String, Vec<String>> = Default::default();
        for p in &self.0 {
            let base = p.rsplit('/').next().unwrap_or(p);
            if names.contains(base) {
                out.entry(base.to_string()).or_default().push(p.clone());
            }
        }
        for v in out.values_mut() {
            v.sort();
        }
        out
    }
}

fn norm(p: &str) -> String {
    p.replace('\\', "/").trim_start_matches('/').to_ascii_lowercase()
}

/// Tira pastas "embrulho" (`MeuMod/`, `tf/custom/MeuMod/`) até achar uma pasta do jogo.
/// Procura nos 4 primeiros níveis; se não achar, devolve o caminho como veio.
pub fn game_relative(path: &str) -> String {
    let p = norm(path);
    let segs: Vec<&str> = p.split('/').collect();
    for i in 0..segs.len().saturating_sub(1).min(4) {
        if GAME_ROOTS.contains(&segs[i]) {
            return segs[i..].join("/");
        }
    }
    p
}

/// `true` se o primeiro nível do caminho (já relativo ao jogo) é uma pasta que o TF2 reconhece.
pub fn is_game_path(rel: &str) -> bool {
    rel.split('/').next().is_some_and(|root| GAME_ROOTS.contains(&root)) && rel.contains('/')
}

fn is_blocked(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    path.contains('.') && BLOCKED_EXT.contains(&ext.as_str())
}

/// Caminhos suspeitos de escapar da pasta (`..`, caminho absoluto, unidade).
fn is_unsafe_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.starts_with('/') || p.split('/').any(|s| s == "..") || p.chars().nth(1) == Some(':')
}

pub fn analyze(paths: &[String], vanilla: Option<&VanillaIndex>) -> Report {
    let files: Vec<&String> = paths.iter().filter(|p| !p.ends_with('/') && !p.ends_with('\\')).collect();

    let mut blocked: Vec<String> = Vec::new();
    let mut reasons = Vec::new();
    let mut counts: std::collections::BTreeMap<u8, (ContentKind, usize)> = Default::default();
    let mut model_sound: Vec<String> = Vec::new();
    let (mut has_map, mut has_cfg, mut has_hud, mut has_pcf) = (false, false, false, false);

    for original in &files {
        if is_unsafe_path(original) {
            blocked.push((*original).clone());
            continue;
        }
        if is_blocked(original) {
            blocked.push((*original).clone());
            continue;
        }
        let rel = game_relative(original);
        let kind = ContentKind::of(&rel);
        let key = kind as u8;
        counts.entry(key).or_insert((kind, 0)).1 += 1;
        match kind {
            ContentKind::Map => has_map = true,
            ContentKind::Config => has_cfg = true,
            ContentKind::Hud => has_hud = true,
            ContentKind::Particle => has_pcf = true,
            // Só conta se está numa pasta do jogo: um .mp3 solto na raiz do pacote (exemplo, leia-me)
            // não é instalado e não pode derrubar a compatibilidade.
            ContentKind::Model | ContentKind::Sound if is_game_path(&rel) => model_sound.push(rel),
            _ => {}
        }
    }

    let mut flags = Vec::new();
    if has_pcf {
        flags.push(Flag::CompetitiveParticles);
    }
    if has_map {
        flags.push(Flag::MapContent);
    }
    if has_cfg {
        flags.push(Flag::HasConfigs);
    }
    if has_hud {
        flags.push(Flag::HasHud);
    }

    let mut unmatched = Vec::new();
    let compat = if !blocked.is_empty() {
        reasons.push(format!("{} arquivo(s) perigoso(s) ou com caminho inseguro", blocked.len()));
        Compat::Blocked
    } else if has_map {
        reasons.push("contém mapa (.bsp): conteúdo empacotado no mapa não passa pelo preloader".into());
        Compat::OfflineOnly
    } else if model_sound.is_empty() {
        reasons.push("sem modelos ou sons com nome próprio: nada que dependa do caminho original".into());
        Compat::CasualOk
    } else if let Some(index) = vanilla.filter(|i| !i.is_empty()) {
        let (matched, miss): (Vec<_>, Vec<_>) = model_sound.iter().partition(|p| index.contains(p));
        unmatched = miss.into_iter().cloned().collect();
        if unmatched.is_empty() {
            reasons.push(format!("{} modelo(s)/som(ns) usam caminhos do TF2 original", matched.len()));
            Compat::CasualOk
        } else if matched.is_empty() {
            reasons.push(format!("{} modelo(s)/som(ns) com caminho que não existe no TF2 original", unmatched.len()));
            Compat::OfflineOnly
        } else {
            reasons.push(format!(
                "{} de {} modelo(s)/som(ns) usam caminhos do TF2; o resto só funciona offline",
                matched.len(),
                model_sound.len()
            ));
            Compat::Partial
        }
    } else {
        reasons.push("falta o índice de arquivos do TF2 para comparar os caminhos".into());
        Compat::NeedsIndex
    };

    if has_pcf {
        reasons.push("partículas customizadas: proibidas em ligas como a RGL".into());
    }

    Report {
        compat,
        reasons,
        flags,
        blocked,
        unmatched,
        counts: counts.into_values().collect(),
        file_count: files.len(),
    }
}

/// Lista os caminhos de um `.zip` sem extrair nada.
pub fn zip_paths<R: Read + Seek>(reader: R) -> Result<Vec<String>> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| Error::Archive(e.to_string()))?;
    let mut out = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).map_err(|e| Error::Archive(e.to_string()))?;
        let name = entry.name().map_err(|e| Error::Archive(format!("nome de arquivo inválido no zip: {e}")))?;
        out.push(name.into_owned());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn paths(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn vanilla() -> VanillaIndex {
        VanillaIndex::from_paths([
            "models/weapons/c_models/c_scattergun/c_scattergun.mdl",
            "sound/weapons/scatter_gun_shoot.wav",
        ])
    }

    #[test]
    fn wrapper_folders_are_stripped() {
        assert_eq!(game_relative("MeuMod/models/a.mdl"), "models/a.mdl");
        assert_eq!(game_relative("tf/custom/MeuMod/Sound/a.wav"), "sound/a.wav");
        assert_eq!(game_relative("readme.txt"), "readme.txt");
    }

    #[test]
    fn same_paths_as_vanilla_is_casual_ok() {
        let p = paths(&[
            "pack/models/weapons/c_models/c_scattergun/c_scattergun.mdl",
            "pack/materials/models/mine/skin.vtf",
            "pack/readme.txt",
        ]);
        let r = analyze(&p, Some(&vanilla()));
        assert_eq!(r.compat, Compat::CasualOk);
        assert!(r.unmatched.is_empty());
    }

    #[test]
    fn custom_named_model_is_offline_only() {
        let p = paths(&["models/alaxe/custom/thing.mdl"]);
        let r = analyze(&p, Some(&vanilla()));
        assert_eq!(r.compat, Compat::OfflineOnly);
        assert_eq!(r.unmatched, vec!["models/alaxe/custom/thing.mdl"]);
    }

    #[test]
    fn mixed_is_partial() {
        let p = paths(&["sound/weapons/scatter_gun_shoot.wav", "sound/mine/other.wav"]);
        assert_eq!(analyze(&p, Some(&vanilla())).compat, Compat::Partial);
    }

    #[test]
    fn no_index_means_needs_index_only_when_it_matters() {
        let with_model = paths(&["models/a.mdl"]);
        assert_eq!(analyze(&with_model, None).compat, Compat::NeedsIndex);
        let textures_only = paths(&["materials/models/a.vtf", "materials/models/a.vmt"]);
        assert_eq!(analyze(&textures_only, None).compat, Compat::CasualOk);
    }

    #[test]
    fn loose_audio_outside_game_folders_does_not_affect_compatibility() {
        let p = paths(&["Pack/sound/weapons/scatter_gun_shoot.wav", "Pack/Sound Examples.mp3", "Pack/demo.ogg"]);
        let r = analyze(&p, Some(&vanilla()));
        assert_eq!(r.compat, Compat::CasualOk, "{:?}", r.reasons);
        assert!(r.unmatched.is_empty());
    }

    #[test]
    fn executables_and_path_traversal_are_blocked() {
        let r = analyze(&paths(&["models/a.mdl", "setup.exe"]), Some(&vanilla()));
        assert_eq!(r.compat, Compat::Blocked);
        assert_eq!(r.blocked, vec!["setup.exe"]);
        let r = analyze(&paths(&["../../Windows/evil.txt"]), None);
        assert_eq!(r.compat, Compat::Blocked);
        let r = analyze(&paths(&["C:/x/y.txt"]), None);
        assert_eq!(r.compat, Compat::Blocked);
    }

    #[test]
    fn maps_and_particles_raise_flags() {
        let r = analyze(&paths(&["maps/cp_x.bsp"]), None);
        assert_eq!(r.compat, Compat::OfflineOnly);
        assert!(r.flags.contains(&Flag::MapContent));

        let r = analyze(&paths(&["particles/flame.pcf", "cfg/hit.cfg", "resource/ui/a.res"]), None);
        assert_eq!(r.compat, Compat::CasualOk);
        assert!(r.flags.contains(&Flag::CompetitiveParticles));
        assert!(r.flags.contains(&Flag::HasConfigs));
        assert!(r.flags.contains(&Flag::HasHud));
    }

    #[test]
    fn reads_paths_from_a_zip_without_extracting() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            w.start_file("sound/weapons/scatter_gun_shoot.wav", opts).unwrap();
            w.write_all(b"RIFF").unwrap();
            w.finish().unwrap();
        }
        buf.set_position(0);
        let list = zip_paths(buf).unwrap();
        assert_eq!(list, vec!["sound/weapons/scatter_gun_shoot.wav"]);
        assert_eq!(analyze(&list, Some(&vanilla())).compat, Compat::CasualOk);
    }

    #[test]
    fn garbage_is_an_archive_error() {
        let err = zip_paths(Cursor::new(b"not a zip".to_vec())).unwrap_err();
        assert!(matches!(err, Error::Archive(_)));
    }
}
