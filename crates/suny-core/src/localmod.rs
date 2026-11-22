//! Instalar um mod que o usuário já tem no computador (arquivo compactado, pasta ou `.vpk` solto).
//!
//! O app descobre a estrutura sozinho e entrega um `.zip` arrumado, igual ao de um mod do catálogo, para o
//! resto do caminho (análise, instalação em `custom`, lista do casual) funcionar sem saber de onde ele veio:
//! - pastas "embrulho" (`MeuMod/tf/custom/MeuMod/...`) são tiradas pelo `scanner::game_relative`;
//! - arquivos soltos sem pasta do jogo (`hitsound.wav`, `meu_mapa.bsp`, `autoexec.cfg`) ganham a pasta certa:
//!   mapa, config e partícula pela extensão; som, material e modelo pelo nome, comparando com os arquivos
//!   originais do TF2 (só quando UM caminho original combina, nunca por palpite);
//! - o que não deu para colocar fica de fora e é listado, para o usuário saber.
//!
//! Nada aqui escreve no jogo: só lê o que o usuário escolheu e monta o zip na pasta temporária do app.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::archive::{self, Format};
use crate::installer::ModMeta;
use crate::scanner::{self, VanillaIndex};
use crate::{Error, Result};

/// Máximo de arquivos lidos de uma pasta escolhida.
const MAX_FILES: usize = 50_000;
/// Máximo de bytes lidos de uma pasta ou conjunto de arquivos escolhidos (o mesmo limite de um mod instalado).
const MAX_BYTES: u64 = 2 << 30;

/// Extensões que o app tenta colocar na pasta certa quando o arquivo veio solto.
const PLACEABLE: &[&str] = &["wav", "mp3", "ogg", "vtf", "vmt", "mdl", "vvd", "vtx", "phy", "ani", "bsp", "cfg", "pcf"];
/// Lixo que o sistema cria sozinho; nunca vai para o zip.
const JUNK: &[&str] = &["thumbs.db", "desktop.ini", ".ds_store"];

#[derive(Debug, Clone, Serialize)]
pub struct Moved {
    pub from: String,
    pub to: String,
}

/// Um mod local já arrumado em um `.zip` na pasta temporária.
#[derive(Debug, Clone, Serialize)]
pub struct Staged {
    /// 8 dígitos hexadecimais do conteúdo: o mesmo mod gera sempre o mesmo id (e a mesma pasta em `custom`).
    pub id: String,
    #[serde(skip)]
    pub zip: PathBuf,
    /// Nome do que o usuário escolheu (arquivo, pasta ou o primeiro item).
    pub file_name: String,
    pub suggested_name: String,
    /// `zip`, `rar`, `7z`, `vpk`, `folder` ou `files`.
    pub source: String,
    /// Arquivos que ganharam uma pasta do jogo.
    pub moved: Vec<Moved>,
    /// Arquivos que parecem do jogo mas o app não soube onde colocar (ficam de fora).
    pub unplaced: Vec<String>,
}

impl Staged {
    pub fn zip_path(&self) -> &Path {
        &self.zip
    }
}

/// O id vem da interface e vira nome de arquivo: só aceita exatamente 8 dígitos hexadecimais.
pub fn valid_id(id: &str) -> bool {
    id.len() == 8 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn staged_zip(cache: &Path, id: &str) -> Option<PathBuf> {
    valid_id(id).then(|| cache.join(format!("local-{}.zip", id.to_ascii_lowercase())))
}

/// Metadados de um mod local. `kind = "Local"`: o instalador usa `file_id` (o hash) no nome da pasta.
pub fn meta_for(staged: &Staged, name: &str) -> ModMeta {
    let name: String = name.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect();
    ModMeta {
        gb_id: 0,
        kind: "Local".into(),
        file_id: u64::from_str_radix(&staged.id, 16).unwrap_or(0),
        file_name: staged.file_name.clone(),
        name: if name.is_empty() { staged.suggested_name.clone() } else { name },
        version: None,
        submitter: None,
        profile_url: String::new(),
        license: None,
        md5: None,
    }
}

fn lower_ext(p: &str) -> String {
    p.rsplit('.').next().filter(|e| e.len() < p.len()).unwrap_or("").to_ascii_lowercase()
}

fn base_name(p: &str) -> &str {
    p.rsplit(['/', '\\']).next().unwrap_or(p)
}

/// `Meu_Mod-v2` → `Meu Mod-v2`; tira `_dir` de pacotes .vpk e espaços nas pontas.
pub fn suggest_name(raw: &str) -> String {
    let stem = raw.rsplit_once('.').filter(|(_, e)| e.len() <= 4 && e.bytes().all(|b| b.is_ascii_alphanumeric())).map_or(raw, |(s, _)| s);
    let stem = stem.strip_suffix("_dir").unwrap_or(stem);
    let spaced: String = stem.chars().map(|c| if c == '_' || c == '+' { ' ' } else { c }).collect();
    let name = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        "Mod manual".into()
    } else {
        name
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 1) escolher o que entra: tudo vira uma lista (caminho no zip, arquivo no disco)

struct Item {
    name: String,
    path: PathBuf,
    size: u64,
}

fn is_junk(name: &str) -> bool {
    JUNK.contains(&base_name(name).to_ascii_lowercase().as_str())
}

/// Os `.vpk` de dados (`pacote_000.vpk`) que acompanham o índice `pacote_dir.vpk` escolhido.
fn vpk_siblings(file: &Path) -> Vec<PathBuf> {
    let Some(name) = file.file_name().and_then(|n| n.to_str()) else { return Vec::new() };
    let lower = name.to_ascii_lowercase();
    let Some(stem) = lower.strip_suffix("_dir.vpk") else { return Vec::new() };
    let Some(dir) = file.parent() else { return Vec::new() };
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
            n.strip_prefix(&format!("{stem}_")).and_then(|r| r.strip_suffix(".vpk")).is_some_and(|d| d.len() == 3 && d.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    out.sort();
    out
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<Item>, total: &mut u64) -> Result<()> {
    let mut stack = vec![(dir.to_path_buf(), prefix.to_string())];
    while let Some((d, pre)) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&d).map_err(|e| Error::io(&d, e))?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            let meta = fs::symlink_metadata(&path).map_err(|er| Error::io(&path, er))?;
            if meta.file_type().is_symlink() {
                continue; // atalhos e links não são seguidos
            }
            let rel = if pre.is_empty() { name.clone() } else { format!("{pre}/{name}") };
            if meta.is_dir() {
                stack.push((path, rel));
            } else if meta.is_file() && !is_junk(&name) {
                if out.len() >= MAX_FILES {
                    return Err(Error::TooLarge(format!("a pasta tem mais de {MAX_FILES} arquivos")));
                }
                *total += meta.len();
                if *total > MAX_BYTES {
                    return Err(Error::TooLarge(format!("o conteúdo escolhido passa de {MAX_BYTES} bytes")));
                }
                out.push(Item { name: rel, path, size: meta.len() });
            }
        }
    }
    Ok(())
}

fn collect(paths: &[PathBuf]) -> Result<Vec<Item>> {
    let mut out = Vec::new();
    let mut total = 0u64;
    let many = paths.len() > 1;
    let mut taken: HashSet<PathBuf> = HashSet::new();
    for p in paths {
        let meta = fs::symlink_metadata(p).map_err(|e| Error::io(p, e))?;
        if meta.file_type().is_symlink() {
            return Err(Error::Blocked("o item escolhido é um atalho (link simbólico)".into()));
        }
        if meta.is_dir() {
            // uma pasta só: o conteúdo dela é a raiz; várias: cada uma mantém o próprio nome para não misturar
            let prefix = if many { p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() } else { String::new() };
            walk(p, &prefix, &mut out, &mut total)?;
        } else if meta.is_file() {
            let mut files = vec![p.clone()];
            files.extend(vpk_siblings(p));
            for f in files {
                if !taken.insert(f.clone()) {
                    continue;
                }
                let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if is_junk(&name) {
                    continue;
                }
                let size = fs::metadata(&f).map_err(|e| Error::io(&f, e))?.len();
                total += size;
                if total > MAX_BYTES {
                    return Err(Error::TooLarge(format!("o conteúdo escolhido passa de {MAX_BYTES} bytes")));
                }
                out.push(Item { name, path: f, size });
            }
        }
    }
    // dois arquivos com o mesmo nome no mesmo lugar: o primeiro vale (o zip não aceita nomes repetidos)
    let mut seen = HashSet::new();
    out.retain(|i| seen.insert(i.name.to_ascii_lowercase()));
    Ok(out)
}

fn build_zip(items: &[Item], dest: &Path) -> Result<()> {
    let file = File::create(dest).map_err(|e| Error::io(dest, e))?;
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default().large_file(true);
    for it in items {
        w.start_file(&it.name, opts).map_err(|e| Error::Archive(e.to_string()))?;
        let mut f = File::open(&it.path).map_err(|e| Error::io(&it.path, e))?;
        io::copy(&mut f, &mut w).map_err(|e| Error::io(&it.path, e))?;
    }
    let mut inner = w.finish().map_err(|e| Error::Archive(e.to_string()))?;
    inner.flush().map_err(|e| Error::io(dest, e))?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// 2) arrumar: onde cada arquivo solto deve ficar

/// Caminho completo da primeira coincidência única do nome no índice original, ou `None`.
fn pick_vanilla(cands: &[String], entry_path: &str) -> Option<String> {
    match cands {
        [] => None,
        [one] => Some(one.clone()),
        many => {
            // vários arquivos originais com o mesmo nome: usa as pastas que o usuário já pôs (`ui/hitsound.wav`)
            let comps: Vec<String> = entry_path.to_ascii_lowercase().replace('\\', "/").split('/').map(String::from).collect();
            for depth in (2..=comps.len()).rev() {
                let suffix = format!("/{}", comps[comps.len() - depth..].join("/"));
                let hit: Vec<&String> = many.iter().filter(|c| c.ends_with(&suffix)).collect();
                match hit.as_slice() {
                    [one] => return Some((*one).clone()),
                    [] => continue,
                    _ => return None,
                }
            }
            None
        }
    }
}

/// Decide o novo nome de cada entrada que não está numa pasta do jogo. Devolve (índice → nome novo, movidos, sem lugar).
fn plan_moves(names: &[String], vanilla: Option<&VanillaIndex>) -> (HashMap<usize, String>, Vec<Moved>, Vec<String>) {
    let mut wanted: HashSet<String> = HashSet::new();
    let mut loose: Vec<usize> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        if name.ends_with('/') || name.ends_with('\\') {
            continue;
        }
        let rel = scanner::game_relative(name);
        if scanner::is_game_path(&rel) || lower_ext(name) == "vpk" {
            continue;
        }
        let ext = lower_ext(name);
        if PLACEABLE.contains(&ext.as_str()) {
            loose.push(i);
            if !matches!(ext.as_str(), "bsp" | "cfg" | "pcf") {
                wanted.insert(base_name(name).to_ascii_lowercase());
            }
        }
    }
    let index: HashMap<String, Vec<String>> = match vanilla.filter(|v| !v.is_empty()) {
        Some(v) if !wanted.is_empty() => v.paths_named(&wanted),
        _ => HashMap::new(),
    };

    let mut renames = HashMap::new();
    let mut moved = Vec::new();
    let mut unplaced = Vec::new();
    let mut used: HashSet<String> = names.iter().map(|n| scanner::game_relative(n)).collect();
    for i in loose {
        let name = &names[i];
        let base = base_name(name).to_ascii_lowercase();
        let to = match lower_ext(name).as_str() {
            "bsp" => Some(format!("maps/{base}")),
            "cfg" => Some(format!("cfg/{base}")),
            "pcf" => Some(format!("particles/{base}")),
            _ => index.get(&base).and_then(|c| pick_vanilla(c, name)),
        };
        match to {
            // dois arquivos soltos que cairiam no mesmo lugar: o segundo fica de fora em vez de sobrescrever
            Some(to) if used.insert(to.clone()) => {
                moved.push(Moved { from: name.clone(), to: to.clone() });
                renames.insert(i, to);
            }
            _ => unplaced.push(name.clone()),
        }
    }
    (renames, moved, unplaced)
}

fn rewrite_zip(src: &Path, dest: &Path, renames: &HashMap<usize, String>) -> Result<()> {
    let mut zin = zip::ZipArchive::new(File::open(src).map_err(|e| Error::io(src, e))?).map_err(|e| Error::Archive(e.to_string()))?;
    let out = File::create(dest).map_err(|e| Error::io(dest, e))?;
    let mut w = zip::ZipWriter::new(out);
    for i in 0..zin.len() {
        let e = zin.by_index_raw(i).map_err(|e| Error::Archive(e.to_string()))?;
        match renames.get(&i) {
            Some(new) => w.raw_copy_file_rename(e, new.clone()),
            None => w.raw_copy_file(e),
        }
        .map_err(|e| Error::Archive(e.to_string()))?;
    }
    let mut inner = w.finish().map_err(|e| Error::Archive(e.to_string()))?;
    inner.flush().map_err(|e| Error::io(dest, e))?;
    Ok(())
}

fn hash_file(p: &Path) -> Result<String> {
    let mut f = File::open(p).map_err(|e| Error::io(p, e))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| Error::io(p, e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().take(4).map(|b| format!("{b:02x}")).collect())
}

fn unique_tmp(cache: &Path, tag: &str) -> PathBuf {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    cache.join(format!("local-{tag}-{}-{n}.tmp", std::process::id()))
}

/// Lê o que o usuário escolheu e entrega o `.zip` arrumado em `cache/local-<id>.zip`.
pub fn stage(paths: &[PathBuf], cache: &Path, vanilla: Option<&VanillaIndex>) -> Result<Staged> {
    if paths.is_empty() {
        return Err(Error::NothingToInstall);
    }
    fs::create_dir_all(cache).map_err(|e| Error::io(cache, e))?;
    let first = &paths[0];
    let first_name = first.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "mod".into());
    let archive_fmt = |p: &Path| p.is_file().then(|| archive::format_of(&p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())).flatten();

    // 1) uma lista de arquivos e pastas vira um zip; um compactado vira (ou já é) um zip
    let src_tmp = unique_tmp(cache, "src");
    let cleanup = |p: &Path| {
        let _ = fs::remove_file(p);
    };
    let (src_zip, owned, source, display) = if paths.len() == 1 && archive_fmt(first).is_some() {
        match archive_fmt(first).unwrap() {
            Format::Zip => (first.clone(), false, "zip".to_string(), first_name.clone()),
            fmt => {
                let dest = src_tmp.with_extension("zip");
                archive::convert_to_zip(first, fmt, &dest)?;
                (dest, true, if fmt == Format::Rar { "rar" } else { "7z" }.to_string(), first_name.clone())
            }
        }
    } else {
        if paths.iter().any(|p| archive_fmt(p).is_some()) {
            return Err(Error::BadName("escolha um arquivo compactado por vez, sem misturar com outros arquivos".into()));
        }
        let items = collect(paths)?;
        if items.is_empty() {
            return Err(Error::NothingToInstall);
        }
        let _total: u64 = items.iter().map(|i| i.size).sum();
        let dest = src_tmp.with_extension("zip");
        if let Err(e) = build_zip(&items, &dest) {
            cleanup(&dest);
            return Err(e);
        }
        let source = if paths.len() == 1 && paths[0].is_dir() {
            "folder"
        } else if paths.len() == 1 && lower_ext(&first_name) == "vpk" {
            "vpk"
        } else {
            "files"
        };
        let display = if paths.len() == 1 { first_name.clone() } else { format!("{first_name} +{}", paths.len() - 1) };
        (dest, true, source.to_string(), display)
    };

    // 2) arruma o que veio solto
    let names = match File::open(&src_zip).map_err(|e| Error::io(&src_zip, e)).and_then(scanner::zip_paths) {
        Ok(n) => n,
        Err(e) => {
            if owned {
                cleanup(&src_zip);
            }
            return Err(e);
        }
    };
    let (renames, moved, unplaced) = plan_moves(&names, vanilla);
    let final_tmp = unique_tmp(cache, "out");
    let made = if renames.is_empty() {
        if owned {
            fs::rename(&src_zip, &final_tmp).map_err(|e| Error::io(&final_tmp, e))
        } else {
            fs::copy(&src_zip, &final_tmp).map(|_| ()).map_err(|e| Error::io(&final_tmp, e))
        }
    } else {
        let r = rewrite_zip(&src_zip, &final_tmp, &renames);
        if owned {
            cleanup(&src_zip);
        }
        r
    };
    if let Err(e) = made {
        cleanup(&final_tmp);
        return Err(e);
    }

    // 3) o id é o hash do zip final: o mesmo mod sempre dá a mesma pasta
    let id = match hash_file(&final_tmp) {
        Ok(id) => id,
        Err(e) => {
            cleanup(&final_tmp);
            return Err(e);
        }
    };
    let zip = cache.join(format!("local-{id}.zip"));
    let _ = fs::remove_file(&zip);
    fs::rename(&final_tmp, &zip).map_err(|e| Error::io(&zip, e))?;

    let suggested = if source == "folder" || paths.len() > 1 && source == "files" { suggest_name(&first_name) } else { suggest_name(&display) };
    Ok(Staged { id, zip, file_name: display, suggested_name: suggested, source, moved, unplaced })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("suny_local_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(p: &Path, data: &[u8]) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }

    fn names_in(zip: &Path) -> Vec<String> {
        let mut v = scanner::zip_paths(File::open(zip).unwrap()).unwrap();
        v.sort();
        v
    }

    fn vanilla() -> VanillaIndex {
        VanillaIndex::from_paths(["sound/ui/hitsound.wav", "sound/ui/killsound.wav", "sound/weapons/shotgun_shoot.wav", "sound/weapons/tf_shotgun_shoot.wav", "sound/misc/hitsound.wav", "models/weapons/c_models/c_bat/c_bat.mdl"])
    }

    #[test]
    fn suggests_a_readable_name() {
        assert_eq!(suggest_name("Cool_Hitsound+v2.zip"), "Cool Hitsound v2");
        assert_eq!(suggest_name("mypack_dir.vpk"), "mypack");
        assert_eq!(suggest_name("Meu Mod"), "Meu Mod");
        assert_eq!(suggest_name(".zip"), "Mod manual");
        assert_eq!(suggest_name("v1.2 final"), "v1.2 final", "ponto no meio do nome não é extensão");
    }

    #[test]
    fn ids_become_file_names_only_when_they_are_eight_hex_digits() {
        assert!(valid_id("0a1B2c3d"));
        for bad in ["", "123", "../../etc1", "0a1b2c3d4", "zzzzzzzz", "0a1b2c3/"] {
            assert!(!valid_id(bad), "{bad}");
        }
        assert!(staged_zip(Path::new("c"), "../../x").is_none());
        assert_eq!(staged_zip(Path::new("c"), "0A1B2C3D").unwrap(), Path::new("c").join("local-0a1b2c3d.zip"));
    }

    #[test]
    fn loose_sounds_go_where_the_original_game_has_them_and_only_when_it_is_certain() {
        let names: Vec<String> = ["hitsound.wav", "Pack/killsound.wav", "weird_new_sound.wav", "mapa.bsp", "autoexec.cfg", "fx.pcf", "readme.txt", "preview.png"].iter().map(|s| s.to_string()).collect();
        // hitsound.wav existe em dois lugares no jogo: sozinho é ambíguo
        let (renames, moved, unplaced) = plan_moves(&names, Some(&vanilla()));
        let to: HashMap<&str, &str> = moved.iter().map(|m| (m.from.as_str(), m.to.as_str())).collect();
        assert_eq!(to.get("Pack/killsound.wav"), Some(&"sound/ui/killsound.wav"));
        assert_eq!(to.get("mapa.bsp"), Some(&"maps/mapa.bsp"));
        assert_eq!(to.get("autoexec.cfg"), Some(&"cfg/autoexec.cfg"));
        assert_eq!(to.get("fx.pcf"), Some(&"particles/fx.pcf"));
        assert!(!to.contains_key("hitsound.wav"), "ambíguo: não chuta");
        assert!(unplaced.contains(&"hitsound.wav".to_string()) && unplaced.contains(&"weird_new_sound.wav".to_string()), "{unplaced:?}");
        assert!(!unplaced.iter().any(|u| u == "readme.txt" || u == "preview.png"), "documentos e imagens não são arquivos de mod");
        assert_eq!(renames.len(), 4);
    }

    #[test]
    fn a_folder_hint_resolves_a_name_that_exists_twice() {
        let (_, moved, unplaced) = plan_moves(&[r"misc\hitsound.wav".to_string()], Some(&vanilla()));
        assert!(unplaced.is_empty(), "{unplaced:?}");
        assert_eq!(moved[0].to, "sound/misc/hitsound.wav");
        // uma pasta que não existe no jogo para esse nome não ajuda: continua ambíguo
        let (_, moved, unplaced) = plan_moves(&["Pack/hitsound.wav".to_string()], Some(&vanilla()));
        assert!(moved.is_empty() && unplaced.len() == 1);
    }

    #[test]
    fn without_the_game_index_only_extension_based_files_are_placed() {
        let names = vec!["hitsound.wav".to_string(), "x.bsp".to_string()];
        let (_, moved, unplaced) = plan_moves(&names, None);
        assert_eq!(moved.len(), 1);
        assert_eq!(unplaced, vec!["hitsound.wav".to_string()]);
    }

    #[test]
    fn files_already_in_game_folders_and_vpks_are_left_alone() {
        let names: Vec<String> = ["MyMod/sound/ui/hitsound.wav", "tf/custom/x/materials/a.vmt", "pack_dir.vpk", "pack_000.vpk", "sound/ui/killsound.wav"].iter().map(|s| s.to_string()).collect();
        let (renames, moved, unplaced) = plan_moves(&names, Some(&vanilla()));
        assert!(renames.is_empty() && moved.is_empty() && unplaced.is_empty());
    }

    #[test]
    fn two_loose_files_that_land_in_the_same_place_do_not_overwrite_each_other() {
        let names = vec!["a/killsound.wav".to_string(), "b/killsound.wav".to_string()];
        let (renames, moved, unplaced) = plan_moves(&names, Some(&vanilla()));
        assert_eq!((renames.len(), moved.len(), unplaced.len()), (1, 1, 1));
    }

    #[test]
    fn a_folder_with_wrapping_directories_becomes_a_clean_zip_with_a_stable_id() {
        let d = tmp("folder");
        let src = d.join("Cool Mod");
        write(&src.join("tf/custom/cool/sound/ui/hitsound.wav"), b"RIFFxxxx");
        write(&src.join("tf/custom/cool/materials/a.vmt"), b"vmt");
        write(&src.join("Thumbs.db"), b"junk");
        write(&src.join("readme.txt"), b"hi");
        let cache = d.join("cache");
        let s1 = stage(&[src.clone()], &cache, None).unwrap();
        assert_eq!((s1.source.as_str(), s1.suggested_name.as_str()), ("folder", "Cool Mod"));
        assert!(s1.moved.is_empty() && s1.unplaced.is_empty());
        let n = names_in(&s1.zip);
        assert_eq!(n.len(), 3, "{n:?}");
        assert!(n.iter().all(|x| !x.to_ascii_lowercase().contains("thumbs")));
        assert!(s1.zip.ends_with(format!("local-{}.zip", s1.id)));
        let s2 = stage(&[src.clone()], &cache, None).unwrap();
        assert_eq!(s1.id, s2.id, "o mesmo conteúdo dá o mesmo id");
        write(&src.join("tf/custom/cool/materials/b.vmt"), b"other");
        assert_ne!(stage(&[src], &cache, None).unwrap().id, s1.id);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_dir_vpk_brings_its_data_parts_along() {
        let d = tmp("vpk");
        write(&d.join("hud_dir.vpk"), b"index");
        write(&d.join("hud_000.vpk"), b"data0");
        write(&d.join("hud_001.vpk"), b"data1");
        write(&d.join("other_000.vpk"), b"nope");
        let s = stage(&[d.join("hud_dir.vpk")], &d.join("cache"), None).unwrap();
        assert_eq!((s.source.as_str(), s.suggested_name.as_str()), ("vpk", "hud"));
        assert_eq!(names_in(&s.zip), vec!["hud_000.vpk", "hud_001.vpk", "hud_dir.vpk"]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn loose_files_are_organized_into_the_zip_and_the_originals_stay_untouched() {
        let d = tmp("loose");
        write(&d.join("in/killsound.wav"), b"RIFF1");
        write(&d.join("in/menu.bsp"), b"VBSP");
        write(&d.join("in/notes.txt"), b"x");
        let before = fs::read(d.join("in/killsound.wav")).unwrap();
        let s = stage(&[d.join("in/killsound.wav"), d.join("in/menu.bsp"), d.join("in/notes.txt")], &d.join("cache"), Some(&vanilla())).unwrap();
        assert_eq!(s.source, "files");
        assert_eq!(names_in(&s.zip), vec!["maps/menu.bsp", "notes.txt", "sound/ui/killsound.wav"]);
        assert_eq!(s.moved.len(), 2);
        assert_eq!(fs::read(d.join("in/killsound.wav")).unwrap(), before);
        assert!(d.join("in/menu.bsp").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_user_zip_is_copied_not_modified_and_mixing_archives_with_files_is_refused() {
        let d = tmp("zip");
        let z = d.join("pack.zip");
        let mut w = zip::ZipWriter::new(File::create(&z).unwrap());
        w.start_file("Pack/sound/ui/hitsound.wav", zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(b"RIFF").unwrap();
        w.finish().unwrap();
        let before = fs::read(&z).unwrap();
        let s = stage(&[z.clone()], &d.join("cache"), None).unwrap();
        assert_eq!((s.source.as_str(), s.suggested_name.as_str()), ("zip", "pack"));
        assert_eq!(fs::read(&z).unwrap(), before);
        assert_eq!(names_in(&s.zip), vec!["Pack/sound/ui/hitsound.wav"]);
        write(&d.join("a.wav"), b"x");
        assert!(stage(&[z, d.join("a.wav")], &d.join("cache"), None).is_err());
        assert!(stage(&[], &d.join("cache"), None).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_local_mod_installs_like_any_other_and_comes_out_cleanly() {
        let d = tmp("e2e");
        write(&d.join("src/Meu Pack/killsound.wav"), b"RIFFkill");
        write(&d.join("src/Meu Pack/hud/resource/ui/x.res"), b"res");
        write(&d.join("src/Meu Pack/exemplo.exe"), b"MZ");
        let paths = [d.join("src/Meu Pack")];
        let custom = d.join("custom");
        // o .exe bloqueia a instalação inteira, como em qualquer mod
        let s = stage(&paths, &d.join("cache"), Some(&vanilla())).unwrap();
        assert!(crate::installer::install_zip(&s.zip, &custom, meta_for(&s, "Meu Pack")).is_err());
        fs::remove_file(d.join("src/Meu Pack/exemplo.exe")).unwrap();

        let s = stage(&paths, &d.join("cache"), Some(&vanilla())).unwrap();
        assert_eq!(s.moved.len(), 1, "o som solto ganhou a pasta certa");
        let meta = meta_for(&s, "Meu Pack");
        let out = crate::installer::install_zip(&s.zip, &custom, meta).unwrap();
        assert_eq!(out.manifest.slug, format!("Meu Pack (local-{})", s.id));
        assert!(custom.join(&out.manifest.slug).join("sound/ui/killsound.wav").is_file());
        assert!(custom.join(&out.manifest.slug).join("resource/ui/x.res").is_file());
        // instalar o mesmo mod de novo troca a versão nossa, sem pedir nada
        let again = crate::installer::install_zip(&s.zip, &custom, meta_for(&s, "Meu Pack")).unwrap();
        assert_eq!(again.manifest.slug, out.manifest.slug);
        crate::installer::uninstall(&custom, &out.manifest.slug).unwrap();
        assert!(!custom.join(&out.manifest.slug).exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn an_empty_folder_is_nothing_to_install() {
        let d = tmp("empty");
        fs::create_dir_all(d.join("vazia")).unwrap();
        assert!(matches!(stage(&[d.join("vazia")], &d.join("cache"), None), Err(Error::NothingToInstall)));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_meta_marks_the_mod_as_local_and_the_hash_is_the_identity() {
        let s = Staged { id: "0000ff10".into(), zip: PathBuf::new(), file_name: "x.zip".into(), suggested_name: "X Mod".into(), source: "zip".into(), moved: vec![], unplaced: vec![] };
        let m = meta_for(&s, "  Meu   Hitsound ");
        assert_eq!((m.kind.as_str(), m.gb_id, m.file_id, m.name.as_str()), ("Local", 0, 0xff10, "Meu Hitsound"));
        assert_eq!(meta_for(&s, "   ").name, "X Mod");
        assert!(m.profile_url.is_empty() && m.submitter.is_none());
    }
}
