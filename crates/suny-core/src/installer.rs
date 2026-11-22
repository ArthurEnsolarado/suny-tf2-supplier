//! Instala e remove mods na pasta `tf/custom`.
//!
//! Regras de segurança (cada uma tem teste):
//! - Só mexe no que o próprio app criou. Cada instalação grava um **manifesto** em
//!   `custom/.suny/<slug>.json`, e remover só apaga o que está nele.
//! - Nunca sobrescreve uma pasta ou arquivo que não seja dele (`FolderTaken`).
//! - Nunca executa nada de dentro do mod. Executáveis e caminhos suspeitos bloqueiam tudo.
//! - Extrai para uma pasta temporária dentro de `custom/.suny` e só então move para o lugar
//!   final: se algo falhar no meio, o jogo fica como estava.
//! - Limites de tamanho por arquivo e total, contra "zip bomb".

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::scanner::{self, Compat, Report};
use crate::{Error, Result};

pub const MANIFEST_DIR: &str = ".suny";

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_entry: u64,
    pub max_total: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_entry: 1 << 30, max_total: 2 << 30 }
    }
}

/// De onde o mod veio. Vai para o manifesto, então autor e licença ficam sempre registrados.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModMeta {
    pub gb_id: u64,
    /// "Mod" ou "Sound".
    pub kind: String,
    pub file_id: u64,
    pub file_name: String,
    pub name: String,
    pub version: Option<String>,
    pub submitter: Option<String>,
    pub profile_url: String,
    pub license: Option<String>,
    pub md5: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    /// Caminho relativo à pasta `custom`.
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub slug: String,
    pub meta: ModMeta,
    pub installed_at: u64,
    /// Entradas diretas de `custom` que este mod criou (a pasta e/ou arquivos .vpk).
    pub items: Vec<String>,
    pub files: Vec<ManifestFile>,
    /// Arquivos do zip que não foram instalados (leia-me, imagens...).
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallOutcome {
    pub manifest: Manifest,
    pub report: Report,
}

const BAD_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*'];

/// Nome da pasta do mod: legível, sem caracteres proibidos no Windows, e com o id do GameBanana
/// no fim (ids de mod e de som podem coincidir, por isso `gbm` / `gbs`).
pub fn slug_for(meta: &ModMeta) -> String {
    // mod local (escolhido pelo usuário): não tem id do GameBanana, a identidade é o hash do conteúdo (`file_id`)
    let id = if meta.kind == "Local" { meta.file_id } else { meta.gb_id };
    slug_parts(&meta.kind, &meta.name, id)
}

/// Igual a [`slug_for`], só com os três dados que ele usa. Em `Local`, `gb_id` é o hash do conteúdo.
pub fn slug_parts(kind: &str, name: &str, gb_id: u64) -> String {
    let tag = match kind {
        "Sound" => format!("gbs{gb_id}"),
        "Local" => format!("local-{gb_id:08x}"),
        _ => format!("gbm{gb_id}"),
    };
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { '_' })
        .collect();
    let joined = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed: String = joined.trim_matches(|c| matches!(c, ' ' | '.' | '_')).chars().take(60).collect();
    let base = trimmed.trim_end_matches(|c| matches!(c, ' ' | '.'));
    let base = if base.is_empty() { "mod" } else { base };
    format!("{base} ({tag})")
}

/// Um nome que vai virar UMA entrada de `custom` (sem separadores, sem `..`).
fn valid_item(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 150
        && !name.starts_with('.')
        && !name.contains(['/', '\\'])
        && !name.contains(BAD_CHARS)
        && !name.ends_with([' ', '.'])
}

/// Caminho relativo dentro da pasta do mod: todos os trechos precisam ser nomes normais.
fn safe_rel(rel: &str) -> bool {
    rel.len() <= 240
        && rel.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != ".." && !seg.contains(['\\']) && !seg.contains(BAD_CHARS) && !seg.ends_with([' ', '.']))
}

fn manifest_path(custom: &Path, slug: &str) -> PathBuf {
    custom.join(MANIFEST_DIR).join(format!("{slug}.json"))
}

pub fn read_manifest(custom: &Path, slug: &str) -> Result<Manifest> {
    if !valid_item(slug) {
        return Err(Error::BadName(slug.to_string()));
    }
    let p = manifest_path(custom, slug);
    let text = fs::read_to_string(&p).map_err(|_| Error::BadName(format!("'{slug}' não foi instalado por este app")))?;
    let m: Manifest = serde_json::from_str(&text)?;
    if m.slug != slug {
        return Err(Error::BadName(format!("manifesto de '{slug}' não confere")));
    }
    Ok(m)
}

/// Todos os mods instalados por este app, em ordem alfabética.
pub fn list_managed(custom: &Path) -> Vec<Manifest> {
    let Ok(rd) = fs::read_dir(custom.join(MANIFEST_DIR)) else { return Vec::new() };
    let mut out: Vec<Manifest> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|t| serde_json::from_str(&t).ok())
        .collect();
    out.sort_by(|a, b| a.slug.to_lowercase().cmp(&b.slug.to_lowercase()));
    out
}

fn describe_blocked(blocked: &[String]) -> String {
    let shown: Vec<&str> = blocked.iter().take(5).map(String::as_str).collect();
    let more = blocked.len().saturating_sub(shown.len());
    format!("arquivos perigosos ou com caminho suspeito: {}{}", shown.join(", "), if more > 0 { format!(" e mais {more}") } else { String::new() })
}

fn write_entry(entry: &mut impl Read, dest: &Path, limits: &Limits, total: &mut u64) -> Result<u64> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    let mut out = File::create(dest).map_err(|e| Error::io(dest, e))?;
    let n = io::copy(&mut entry.take(limits.max_entry + 1), &mut out).map_err(|e| Error::io(dest, e))?;
    out.flush().map_err(|e| Error::io(dest, e))?;
    if n > limits.max_entry {
        return Err(Error::TooLarge(format!("um arquivo do mod passa de {} bytes", limits.max_entry)));
    }
    *total += n;
    if *total > limits.max_total {
        return Err(Error::TooLarge(format!("o mod descompactado passa de {} bytes", limits.max_total)));
    }
    Ok(n)
}

fn remove_items(custom: &Path, items: &[String]) -> Result<()> {
    for item in items {
        if !valid_item(item) {
            return Err(Error::BadName(format!("item inválido no manifesto: {item}")));
        }
        let p = custom.join(item);
        let res = if p.is_dir() { fs::remove_dir_all(&p) } else { fs::remove_file(&p) };
        match res {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io(p, e)),
        }
    }
    Ok(())
}

pub fn install_zip(zip_path: &Path, custom_dir: &Path, meta: ModMeta) -> Result<InstallOutcome> {
    install_zip_with(zip_path, custom_dir, meta, Limits::default())
}

pub fn install_zip_with(zip_path: &Path, custom_dir: &Path, meta: ModMeta, limits: Limits) -> Result<InstallOutcome> {
    let file = File::open(zip_path).map_err(|e| Error::io(zip_path, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Archive(e.to_string()))?;

    // 1) nomes e análise de segurança, antes de escrever qualquer coisa
    let mut names: Vec<String> = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let e = zip.by_index_raw(i).map_err(|e| Error::Archive(e.to_string()))?;
        names.push(e.name().map_err(|e| Error::Archive(format!("nome inválido no zip: {e}")))?.into_owned());
    }
    let report = scanner::analyze(&names, None);
    if report.compat == Compat::Blocked {
        return Err(Error::Blocked(describe_blocked(&report.blocked)));
    }

    // 2) plano: o que vai para a pasta do mod, o que é .vpk solto, o que é ignorado
    let mut folder_entries: Vec<(usize, String)> = Vec::new();
    let mut vpk_entries: Vec<(usize, String)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (i, name) in names.iter().enumerate() {
        if name.ends_with('/') || name.ends_with('\\') {
            continue;
        }
        let rel = scanner::game_relative(name);
        if scanner::is_game_path(&rel) {
            if safe_rel(&rel) && seen.insert(format!("f:{rel}")) {
                folder_entries.push((i, rel));
            } else if !safe_rel(&rel) {
                skipped.push(name.clone());
            }
        } else if name.to_ascii_lowercase().ends_with(".vpk") {
            let base = name.rsplit(['/', '\\']).next().unwrap_or(name).to_string();
            if valid_item(&base) && seen.insert(format!("v:{}", base.to_lowercase())) {
                vpk_entries.push((i, base));
            } else {
                skipped.push(name.clone());
            }
        } else {
            skipped.push(name.clone());
        }
    }
    if folder_entries.is_empty() && vpk_entries.is_empty() {
        return Err(Error::NothingToInstall);
    }

    // 3) conflitos: nada que não seja nosso pode ser sobrescrito
    let slug = slug_for(&meta);
    fs::create_dir_all(custom_dir).map_err(|e| Error::io(custom_dir, e))?;
    let old = read_manifest(custom_dir, &slug).ok();
    let owned: HashSet<&str> = old.iter().flat_map(|m| m.items.iter().map(String::as_str)).collect();
    let folder_final = custom_dir.join(&slug);
    if !folder_entries.is_empty() && folder_final.exists() && !owned.contains(slug.as_str()) {
        return Err(Error::FolderTaken(slug));
    }
    for (_, name) in &vpk_entries {
        if custom_dir.join(name).exists() && !owned.contains(name.as_str()) {
            return Err(Error::FolderTaken(name.clone()));
        }
    }

    // 4) extrai para a pasta temporária
    let tmp = custom_dir.join(MANIFEST_DIR).join(format!("tmp-{slug}"));
    let _ = fs::remove_dir_all(&tmp);
    let tmp_folder = tmp.join("folder");
    let tmp_vpk = tmp.join("vpk");
    let mut files: Vec<ManifestFile> = Vec::new();

    let extract = (|| -> Result<()> {
        let mut total = 0u64;
        for (i, rel) in &folder_entries {
            let mut entry = zip.by_index(*i).map_err(|e| Error::Archive(e.to_string()))?;
            let dest = rel.split('/').fold(tmp_folder.clone(), |p, seg| p.join(seg));
            let size = write_entry(&mut entry, &dest, &limits, &mut total)?;
            files.push(ManifestFile { path: format!("{slug}/{rel}"), size });
        }
        for (i, name) in &vpk_entries {
            let mut entry = zip.by_index(*i).map_err(|e| Error::Archive(e.to_string()))?;
            let size = write_entry(&mut entry, &tmp_vpk.join(name), &limits, &mut total)?;
            files.push(ManifestFile { path: name.clone(), size });
        }
        Ok(())
    })();
    if let Err(e) = extract {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }

    // 5) troca: remove a versão antiga (se for nossa) e põe a nova no lugar
    let commit = (|| -> Result<()> {
        if let Some(prev) = &old {
            remove_items(custom_dir, &prev.items)?;
        }
        if tmp_folder.exists() {
            fs::rename(&tmp_folder, &folder_final).map_err(|e| Error::io(&folder_final, e))?;
        }
        for (_, name) in &vpk_entries {
            let to = custom_dir.join(name);
            fs::rename(tmp_vpk.join(name), &to).map_err(|e| Error::io(to, e))?;
        }
        Ok(())
    })();
    let _ = fs::remove_dir_all(&tmp);
    commit?;

    // 6) manifesto
    let mut items: Vec<String> = Vec::new();
    if !folder_entries.is_empty() {
        items.push(slug.clone());
    }
    items.extend(vpk_entries.iter().map(|(_, n)| n.clone()));
    let manifest = Manifest {
        slug: slug.clone(),
        meta,
        installed_at: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        items,
        files,
        skipped,
    };
    let mp = manifest_path(custom_dir, &slug);
    fs::create_dir_all(mp.parent().unwrap()).map_err(|e| Error::io(mp.parent().unwrap(), e))?;
    fs::write(&mp, serde_json::to_string_pretty(&manifest)?).map_err(|e| Error::io(&mp, e))?;

    Ok(InstallOutcome { manifest, report })
}

/// Remove um mod instalado por este app. Só apaga o que está no manifesto.
pub fn uninstall(custom_dir: &Path, slug: &str) -> Result<Manifest> {
    let m = read_manifest(custom_dir, slug)?;
    remove_items(custom_dir, &m.items)?;
    let mp = manifest_path(custom_dir, slug);
    fs::remove_file(&mp).map_err(|e| Error::io(mp, e))?;
    // Se não sobrou nenhum manifesto, tira a pasta `.suny` também (`remove_dir` só apaga pasta vazia).
    let _ = fs::remove_dir(custom_dir.join(MANIFEST_DIR));
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("suny_inst_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    fn meta(name: &str) -> ModMeta {
        ModMeta {
            gb_id: 42,
            kind: "Sound".into(),
            file_id: 7,
            file_name: "x.zip".into(),
            name: name.into(),
            version: Some("1.0".into()),
            submitter: Some("Autor".into()),
            profile_url: "https://gamebanana.com/sounds/42".into(),
            license: None,
            md5: None,
        }
    }

    #[test]
    fn slug_is_readable_and_safe() {
        assert_eq!(slug_for(&meta("Hit Sound: <Pack>?")), "Hit Sound_ _Pack (gbs42)");
        assert_eq!(slug_for(&meta("   ...  ")), "mod (gbs42)");
        let mut m = meta("x");
        m.kind = "Mod".into();
        assert_eq!(slug_for(&m), "x (gbm42)");
        assert!(slug_for(&meta(&"a".repeat(200))).len() < 80);
    }

    #[test]
    fn installs_loose_files_stripping_wrapper_and_skipping_extras() {
        let d = tmpdir("loose");
        let custom = d.join("custom");
        let z = d.join("m.zip");
        make_zip(&z, &[("MeuPack/Sound/UI/HitSound.wav", b"RIFFdata"), ("MeuPack/leia-me.txt", b"oi"), ("MeuPack/preview.png", b"png")]);

        let out = install_zip(&z, &custom, meta("Meu Pack")).unwrap();
        let slug = "Meu Pack (gbs42)";
        assert_eq!(fs::read(custom.join(slug).join("sound/ui/hitsound.wav")).unwrap(), b"RIFFdata");
        assert_eq!(out.manifest.items, vec![slug.to_string()]);
        assert_eq!(out.manifest.files.len(), 1);
        assert_eq!(out.manifest.files[0].path, format!("{slug}/sound/ui/hitsound.wav"));
        assert_eq!(out.manifest.skipped.len(), 2);
        assert!(!custom.join(slug).join("leia-me.txt").exists());
        assert!(manifest_path(&custom, slug).is_file());
        assert!(!custom.join(MANIFEST_DIR).join(format!("tmp-{slug}")).exists(), "não pode sobrar pasta temporária");
        assert_eq!(list_managed(&custom).len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn executables_block_everything_and_write_nothing() {
        let d = tmpdir("exe");
        let custom = d.join("custom");
        let z = d.join("m.zip");
        make_zip(&z, &[("models/a.mdl", b"x"), ("instalar.exe", b"MZ")]);
        let err = install_zip(&z, &custom, meta("Ruim")).unwrap_err();
        assert!(matches!(err, Error::Blocked(_)), "{err}");
        assert!(!custom.join("Ruim (gbs42)").exists());
        assert!(list_managed(&custom).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn path_traversal_is_blocked_and_nothing_escapes() {
        let d = tmpdir("trav");
        let custom = d.join("custom");
        let z = d.join("m.zip");
        make_zip(&z, &[("../../evil/models/a.mdl", b"x")]);
        assert!(matches!(install_zip(&z, &custom, meta("T")).unwrap_err(), Error::Blocked(_)));
        assert!(!d.join("evil").exists() && !d.parent().unwrap().join("evil").exists());
        make_zip(&z, &[("C:/x/models/a.mdl", b"x")]);
        assert!(matches!(install_zip(&z, &custom, meta("T")).unwrap_err(), Error::Blocked(_)));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn vpk_files_go_directly_into_custom() {
        let d = tmpdir("vpk");
        let custom = d.join("custom");
        let z = d.join("m.zip");
        make_zip(&z, &[("Pasta/meu_hud.vpk", b"VPK!"), ("Pasta/leiame.txt", b"x")]);
        let out = install_zip(&z, &custom, meta("HUD")).unwrap();
        assert_eq!(fs::read(custom.join("meu_hud.vpk")).unwrap(), b"VPK!");
        assert_eq!(out.manifest.items, vec!["meu_hud.vpk".to_string()]);
        assert!(!custom.join("HUD (gbs42)").exists(), "sem arquivos soltos, não cria pasta");
        uninstall(&custom, "HUD (gbs42)").unwrap();
        assert!(!custom.join("meu_hud.vpk").exists());
        assert!(!custom.join(MANIFEST_DIR).exists(), "sem manifestos, a pasta .suny some");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn archive_without_game_content_is_refused() {
        let d = tmpdir("none");
        let z = d.join("m.zip");
        make_zip(&z, &[("leia-me.txt", b"x"), ("foto.png", b"y")]);
        assert!(matches!(install_zip(&z, &d.join("custom"), meta("N")).unwrap_err(), Error::NothingToInstall));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn reinstall_replaces_our_own_folder_completely() {
        let d = tmpdir("re");
        let custom = d.join("custom");
        let z = d.join("m.zip");
        make_zip(&z, &[("sound/a.wav", b"v1"), ("sound/velho.wav", b"old")]);
        install_zip(&z, &custom, meta("R")).unwrap();
        make_zip(&z, &[("sound/a.wav", b"v2")]);
        install_zip(&z, &custom, meta("R")).unwrap();
        let slug = "R (gbs42)";
        assert_eq!(fs::read(custom.join(slug).join("sound/a.wav")).unwrap(), b"v2");
        assert!(!custom.join(slug).join("sound/velho.wav").exists(), "arquivo da versão antiga some");
        assert_eq!(list_managed(&custom).len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn never_overwrites_a_folder_it_did_not_create() {
        let d = tmpdir("taken");
        let custom = d.join("custom");
        fs::create_dir_all(custom.join("Pack (gbs42)")).unwrap();
        fs::write(custom.join("Pack (gbs42)/meu_arquivo.txt"), b"do usuario").unwrap();
        let z = d.join("m.zip");
        make_zip(&z, &[("sound/a.wav", b"x")]);
        let err = install_zip(&z, &custom, meta("Pack")).unwrap_err();
        assert!(matches!(err, Error::FolderTaken(_)), "{err}");
        assert_eq!(fs::read(custom.join("Pack (gbs42)/meu_arquivo.txt")).unwrap(), b"do usuario");
        assert!(list_managed(&custom).is_empty());

        // o mesmo vale para um .vpk solto com o mesmo nome
        fs::write(custom.join("hud.vpk"), b"do usuario").unwrap();
        make_zip(&z, &[("hud.vpk", b"novo")]);
        assert!(matches!(install_zip(&z, &custom, meta("Outro")).unwrap_err(), Error::FolderTaken(_)));
        assert_eq!(fs::read(custom.join("hud.vpk")).unwrap(), b"do usuario");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn uninstall_removes_only_what_the_manifest_lists() {
        let d = tmpdir("un");
        let custom = d.join("custom");
        fs::create_dir_all(custom.join("Pink Charge")).unwrap();
        fs::write(custom.join("Pink Charge/a.wav"), b"meu").unwrap();
        let z = d.join("m.zip");
        make_zip(&z, &[("sound/a.wav", b"x")]);
        install_zip(&z, &custom, meta("U")).unwrap();

        let m = uninstall(&custom, "U (gbs42)").unwrap();
        assert_eq!(m.items, vec!["U (gbs42)".to_string()]);
        assert!(!custom.join("U (gbs42)").exists());
        assert!(list_managed(&custom).is_empty());
        assert!(custom.join("Pink Charge/a.wav").exists(), "mod que não é do app fica intacto");

        // não instalado pelo app, ou nome malicioso: recusa
        assert!(uninstall(&custom, "Pink Charge").is_err());
        assert!(custom.join("Pink Charge/a.wav").exists());
        for bad in ["..", "../x", "a/b", ".suny", ""] {
            assert!(matches!(uninstall(&custom, bad), Err(Error::BadName(_))), "{bad:?}");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_tampered_manifest_cannot_delete_outside_custom() {
        let d = tmpdir("tamper");
        let custom = d.join("custom");
        fs::create_dir_all(custom.join(MANIFEST_DIR)).unwrap();
        fs::write(d.join("importante.txt"), b"nao apague").unwrap();
        let m = Manifest {
            slug: "X (gbs1)".into(),
            meta: meta("X"),
            installed_at: 0,
            items: vec!["..".into(), "../importante.txt".into()],
            files: vec![],
            skipped: vec![],
        };
        fs::write(manifest_path(&custom, "X (gbs1)"), serde_json::to_string(&m).unwrap()).unwrap();
        assert!(uninstall(&custom, "X (gbs1)").is_err());
        assert!(d.join("importante.txt").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn size_limits_stop_zip_bombs_and_leave_no_trace() {
        let d = tmpdir("lim");
        let custom = d.join("custom");
        let z = d.join("m.zip");
        make_zip(&z, &[("sound/a.wav", &[7u8; 50])]);
        let err = install_zip_with(&z, &custom, meta("L"), Limits { max_entry: 10, max_total: 100 }).unwrap_err();
        assert!(matches!(err, Error::TooLarge(_)), "{err}");
        assert!(!custom.join("L (gbs42)").exists());
        assert!(!custom.join(MANIFEST_DIR).join("tmp-L (gbs42)").exists());

        make_zip(&z, &[("sound/a.wav", &[7u8; 30]), ("sound/b.wav", &[7u8; 30])]);
        let err = install_zip_with(&z, &custom, meta("L"), Limits { max_entry: 40, max_total: 50 }).unwrap_err();
        assert!(matches!(err, Error::TooLarge(_)), "{err}");
        assert!(list_managed(&custom).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_failed_reinstall_keeps_the_previous_version() {
        let d = tmpdir("keep");
        let custom = d.join("custom");
        let z = d.join("m.zip");
        make_zip(&z, &[("sound/a.wav", b"v1")]);
        install_zip(&z, &custom, meta("K")).unwrap();
        make_zip(&z, &[("sound/a.wav", &[9u8; 50])]);
        assert!(install_zip_with(&z, &custom, meta("K"), Limits { max_entry: 10, max_total: 100 }).is_err());
        assert_eq!(fs::read(custom.join("K (gbs42)/sound/a.wav")).unwrap(), b"v1");
        assert_eq!(list_managed(&custom).len(), 1);
        let _ = fs::remove_dir_all(&d);
    }
}
