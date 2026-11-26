//! Escolher qual som de um pacote do casual toca.
//!
//! O TF2 sorteia entre variações numeradas (`demo_charge_hit_world1..3.wav`). Para fixar uma, o app grava o som
//! escolhido nas três posições: o sorteio continua e sempre sai o mesmo. "Original" tira o grupo do mod, e o jogo volta
//! a tocar o dele. A cópia como o pacote veio fica guardada em `originals/<mod>/`, fora do preloader, então dá para
//! mudar de ideia. Os grupos são lidos dos nomes dos arquivos, sem lista fixa.
//!
//! Só mexe na cópia do mod dentro do preloader; o jogo só muda quando o usuário aplica de novo.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::casual::{self, Layout};

/// Segue o preloader: só estes formatos são sons.
const SOUND_EXT: &[&str] = &["wav", "mp3", "ogg"];
const MAX_DEPTH: usize = 16;

pub const RANDOM: &str = "random";
pub const ORIGINAL: &str = "original";

#[derive(Debug, Clone, Serialize)]
pub struct SoundOption {
    /// Caminho dentro do mod (`sound/weapons/x1.wav`): é o que se manda de volta para escolher ou ouvir.
    pub file: String,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SoundFamily {
    pub key: String,
    /// Pasta dentro de `sound/` mais o nome sem o número (`weapons/demo_charge_hit_world`).
    pub name: String,
    pub options: Vec<SoundOption>,
    /// `random` (como o pacote veio), `original` (o jogo toca o dele) ou o `file` escolhido.
    pub choice: String,
}

fn addon_dir(l: &Layout, addon: &str) -> Result<PathBuf, String> {
    let dir = l.addons_dir().join(addon);
    if !casual::valid_addon_name(addon) || !dir.is_dir() {
        return Err(format!("o mod '{addon}' não está na lista do casual"));
    }
    Ok(dir)
}

fn originals_dir(l: &Layout, addon: &str) -> PathBuf {
    l.root.join("originals").join(addon)
}

fn choices_file(l: &Layout, addon: &str) -> PathBuf {
    l.root.join("choices").join(format!("{addon}.json"))
}

fn load_choices(l: &Layout, addon: &str) -> BTreeMap<String, String> {
    fs::read_to_string(choices_file(l, addon)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_choices(l: &Layout, addon: &str, choices: &BTreeMap<String, String>) -> Result<(), String> {
    let path = choices_file(l, addon);
    if choices.is_empty() {
        let _ = fs::remove_file(path);
        return Ok(());
    }
    casual::write_atomic(&path, &serde_json::to_vec_pretty(choices).map_err(|e| e.to_string())?)
}

/// Esquece as escolhas e as cópias guardadas de um mod (ele saiu da lista ou foi importado de novo).
pub fn forget(l: &Layout, addon: &str) {
    if casual::valid_addon_name(addon) {
        let _ = fs::remove_dir_all(originals_dir(l, addon));
        let _ = fs::remove_file(choices_file(l, addon));
    }
}

/// Os sons abaixo de `base/sound`, como (caminho relativo a `base`, caminho no disco).
fn sounds_under(base: &Path) -> BTreeMap<String, PathBuf> {
    fn walk(base: &Path, dir: &Path, depth: usize, out: &mut BTreeMap<String, PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let path = e.path();
            if ft.is_dir() && depth < MAX_DEPTH {
                walk(base, &path, depth + 1, out);
            } else if ft.is_file() {
                let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
                if SOUND_EXT.contains(&ext.as_str()) {
                    if let Ok(rel) = path.strip_prefix(base) {
                        out.insert(rel.to_string_lossy().replace('\\', "/"), path.clone());
                    }
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(base, &base.join("sound"), 0, &mut out);
    out
}

/// (chave do grupo, nome do grupo, número) de um arquivo numerado, ou `None`.
fn family_of(rel: &str) -> Option<(String, String, u64)> {
    let (dir, file) = rel.rsplit_once('/')?;
    let (stem, ext) = file.rsplit_once('.')?;
    let head = stem.trim_end_matches(|c: char| c.is_ascii_digit());
    let number: u64 = stem[head.len()..].parse().ok()?;
    let head = head.trim_end_matches('_');
    if head.is_empty() {
        return None;
    }
    let name = format!("{dir}/{head}");
    let key = format!("{name}.{ext}").to_ascii_lowercase();
    let name = name.split_once('/').map_or(name.clone(), |(_, rest)| rest.to_string()).to_ascii_lowercase();
    Some((key, name, number))
}

fn same_bytes(a: &Path, b: &Path) -> bool {
    matches!((fs::read(a), fs::read(b)), (Ok(x), Ok(y)) if x == y)
}

/// A escolha guardada ainda vale? Se o mod foi importado de novo por cima, os arquivos voltaram e ela não vale mais.
fn still_applied(dir: &Path, originals: &Path, members: &[String], choice: &str) -> bool {
    if choice == ORIGINAL {
        return members.iter().all(|m| !dir.join(m).exists());
    }
    members.iter().all(|m| same_bytes(&dir.join(m), &originals.join(choice)))
}

/// Os grupos como o pacote veio (a cópia guardada vale mais que a do mod), com a escolha de cada um.
fn scan(l: &Layout, addon: &str) -> Result<Vec<(SoundFamily, BTreeMap<String, PathBuf>)>, String> {
    let dir = addon_dir(l, addon)?;
    let originals = originals_dir(l, addon);
    let mut files = sounds_under(&dir);
    files.extend(sounds_under(&originals));

    let mut groups: BTreeMap<String, (String, Vec<(u64, String, PathBuf)>)> = BTreeMap::new();
    for (rel, path) in files {
        if let Some((key, name, number)) = family_of(&rel) {
            groups.entry(key).or_insert_with(|| (name, Vec::new())).1.push((number, rel, path));
        }
    }

    let mut saved = load_choices(l, addon);
    let mut changed = false;
    let mut out = Vec::new();
    for (key, (name, mut members)) in groups {
        if members.len() < 2 {
            continue;
        }
        members.sort();
        let rels: Vec<String> = members.iter().map(|m| m.1.clone()).collect();
        let mut choice = saved.get(&key).cloned().unwrap_or_else(|| RANDOM.into());
        let mut paths: BTreeMap<String, PathBuf> = members.iter().map(|m| (m.1.clone(), m.2.clone())).collect();
        if choice != RANDOM && !still_applied(&dir, &originals, &rels, &choice) {
            // importado por cima: o que está no mod agora é o pacote novo, a cópia antiga não serve mais
            for rel in &rels {
                let _ = fs::remove_file(originals.join(rel));
                if dir.join(rel).is_file() {
                    paths.insert(rel.clone(), dir.join(rel));
                }
            }
            saved.remove(&key);
            changed = true;
            choice = RANDOM.into();
        }
        // variações idênticas não têm o que escolher
        let first = &paths[&rels[0]];
        if choice == RANDOM && rels.iter().skip(1).all(|r| same_bytes(first, &paths[r])) {
            continue;
        }
        let options = rels
            .iter()
            .map(|rel| SoundOption {
                file: rel.clone(),
                name: rel.rsplit('/').next().unwrap_or(rel).to_string(),
                size: fs::metadata(&paths[rel]).map(|m| m.len()).unwrap_or(0),
            })
            .collect();
        out.push((SoundFamily { key, name, options, choice }, paths));
    }
    if changed {
        save_choices(l, addon, &saved)?;
    }
    Ok(out)
}

pub fn families(l: &Layout, addon: &str) -> Result<Vec<SoundFamily>, String> {
    Ok(scan(l, addon)?.into_iter().map(|(f, _)| f).collect())
}

/// Onde ler um som do pacote (como ele veio) para ouvir antes de escolher. Só arquivos que são opção de algum grupo.
pub fn option_path(l: &Layout, addon: &str, file: &str) -> Result<PathBuf, String> {
    scan(l, addon)?.into_iter().find_map(|(_, paths)| paths.get(file).cloned()).ok_or_else(|| "esse som não faz parte deste mod".to_string())
}

/// Troca a escolha de um grupo. `choice`: `random`, `original` ou o `file` de uma das opções.
pub fn choose(l: &Layout, addon: &str, key: &str, choice: &str) -> Result<(), String> {
    let dir = addon_dir(l, addon)?;
    let originals = originals_dir(l, addon);
    let (family, paths) = scan(l, addon)?.into_iter().find(|(f, _)| f.key == key).ok_or_else(|| "esse grupo de sons não existe neste mod".to_string())?;
    if choice != RANDOM && choice != ORIGINAL && !family.options.iter().any(|o| o.file == choice) {
        return Err("esse som não faz parte deste grupo".into());
    }
    if family.choice == choice {
        return Ok(());
    }
    let io = |what: &Path, e: std::io::Error| format!("não consegui gravar {}: {e}", what.display());

    // 1) a cópia como o pacote veio, antes de mexer em qualquer coisa
    for (rel, current) in &paths {
        let kept = originals.join(rel);
        if !kept.is_file() {
            casual::write_atomic(&kept, &fs::read(current).map_err(|e| io(current, e))?)?;
        }
    }
    // 2) o mod fica como a escolha pede
    let fixed = (choice != RANDOM && choice != ORIGINAL).then(|| fs::read(originals.join(choice)).map_err(|e| io(&originals.join(choice), e))).transpose()?;
    for rel in paths.keys() {
        let target = dir.join(rel);
        match (choice, &fixed) {
            (ORIGINAL, _) => {
                if target.exists() {
                    fs::remove_file(&target).map_err(|e| io(&target, e))?;
                }
            }
            (_, Some(bytes)) => casual::write_atomic(&target, bytes)?,
            _ => casual::write_atomic(&target, &fs::read(originals.join(rel)).map_err(|e| io(&originals.join(rel), e))?)?,
        }
    }
    // 3) voltando ao sorteio, a cópia não é mais necessária
    let mut saved = load_choices(l, addon);
    if choice == RANDOM {
        for rel in paths.keys() {
            let _ = fs::remove_file(originals.join(rel));
        }
        saved.remove(key);
    } else {
        saved.insert(key.to_string(), choice.to_string());
    }
    save_choices(l, addon, &saved)?;
    casual::mark_changed(l, addon)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sound/weapons/hit_world.wav";

    fn setup(tag: &str) -> (Layout, PathBuf) {
        let root = std::env::temp_dir().join(format!("suny_pick_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let l = Layout::new(&root);
        let dir = l.addons_dir().join("Pink");
        for (name, data) in [("hit_world1.wav", "um"), ("hit_world2.wav", "dois"), ("hit_world3.wav", "tres"), ("hit_flesh_range1.wav", "a"), ("hit_flesh_range2.wav", "b"), ("solo.wav", "s"), ("same1.wav", "x"), ("same2.wav", "x")] {
            fs::create_dir_all(dir.join("sound/weapons")).unwrap();
            fs::write(dir.join("sound/weapons").join(name), data).unwrap();
        }
        (l, dir)
    }

    fn content(dir: &Path, n: u32) -> Option<String> {
        fs::read_to_string(dir.join(format!("sound/weapons/hit_world{n}.wav"))).ok()
    }

    fn world(l: &Layout) -> SoundFamily {
        families(l, "Pink").unwrap().into_iter().find(|f| f.key == KEY).unwrap()
    }

    #[test]
    fn numbered_sounds_become_groups_and_lone_or_identical_ones_do_not() {
        let (l, _) = setup("groups");
        let names: Vec<String> = families(&l, "Pink").unwrap().into_iter().map(|f| f.name).collect();
        assert_eq!(names, vec!["weapons/hit_flesh_range", "weapons/hit_world"]);
        let w = world(&l);
        assert_eq!(w.choice, RANDOM);
        assert_eq!(w.options.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), vec!["hit_world1.wav", "hit_world2.wav", "hit_world3.wav"]);
        let _ = fs::remove_dir_all(&l.root);
    }

    #[test]
    fn a_fixed_sound_goes_into_every_slot_and_the_pack_can_be_restored() {
        let (l, dir) = setup("fixed");
        let second = "sound/weapons/hit_world2.wav";
        choose(&l, "Pink", KEY, second).unwrap();
        assert_eq!((1..=3).map(|n| content(&dir, n).unwrap()).collect::<Vec<_>>(), vec!["dois"; 3]);
        let w = world(&l);
        assert_eq!(w.choice, second);
        assert_eq!(w.options[0].size, 2, "as opções continuam sendo os sons como o pacote veio");
        assert_eq!(fs::read_to_string(option_path(&l, "Pink", "sound/weapons/hit_world3.wav").unwrap()).unwrap(), "tres");
        // trocar de uma fixa para outra usa o pacote original, não a posição já sobrescrita
        choose(&l, "Pink", KEY, "sound/weapons/hit_world3.wav").unwrap();
        assert_eq!(content(&dir, 1).unwrap(), "tres");
        choose(&l, "Pink", KEY, RANDOM).unwrap();
        assert_eq!((1..=3).map(|n| content(&dir, n).unwrap()).collect::<Vec<_>>(), vec!["um", "dois", "tres"]);
        assert!(!choices_file(&l, "Pink").exists());
        assert_eq!(casual::changed_since_apply(&l), vec!["Pink".to_string()]);
        let _ = fs::remove_dir_all(&l.root);
    }

    #[test]
    fn original_takes_only_that_group_out_of_the_mod_and_can_come_back() {
        let (l, dir) = setup("orig");
        choose(&l, "Pink", KEY, ORIGINAL).unwrap();
        assert!((1..=3).all(|n| content(&dir, n).is_none()));
        assert!(dir.join("sound/weapons/hit_flesh_range1.wav").is_file() && dir.join("sound/weapons/solo.wav").is_file());
        assert_eq!(world(&l).choice, ORIGINAL);
        assert_eq!(world(&l).options.len(), 3, "o grupo continua na tela para poder voltar");
        choose(&l, "Pink", KEY, RANDOM).unwrap();
        assert_eq!(content(&dir, 3).unwrap(), "tres");
        let _ = fs::remove_dir_all(&l.root);
    }

    #[test]
    fn importing_the_pack_again_over_a_choice_resets_it_without_bringing_back_the_old_copy() {
        let (l, dir) = setup("reimport");
        choose(&l, "Pink", KEY, "sound/weapons/hit_world1.wav").unwrap();
        for (n, data) in [(1, "novo1"), (2, "novo2"), (3, "novo3")] {
            fs::write(dir.join(format!("sound/weapons/hit_world{n}.wav")), data).unwrap();
        }
        assert_eq!(world(&l).choice, RANDOM);
        assert_eq!(fs::read_to_string(option_path(&l, "Pink", "sound/weapons/hit_world1.wav").unwrap()).unwrap(), "novo1");
        choose(&l, "Pink", KEY, "sound/weapons/hit_world2.wav").unwrap();
        assert_eq!(content(&dir, 1).unwrap(), "novo2");
        let _ = fs::remove_dir_all(&l.root);
    }

    #[test]
    fn only_listed_files_and_real_mods_are_accepted() {
        let (l, dir) = setup("safe");
        for bad in ["sound/weapons/solo.wav", "../../x", "sound/weapons/hit_flesh_range1.wav"] {
            assert!(choose(&l, "Pink", KEY, bad).is_err(), "{bad}");
        }
        assert!(choose(&l, "Pink", "sound/nada.wav", RANDOM).is_err());
        assert!(families(&l, "../Pink").is_err() && families(&l, "Outro").is_err());
        assert!(option_path(&l, "Pink", "../state.json").is_err());
        assert_eq!(content(&dir, 1).unwrap(), "um", "nada mudou");
        let _ = fs::remove_dir_all(&l.root);
    }

    #[test]
    fn forgetting_a_mod_removes_its_copies() {
        let (l, _) = setup("forget");
        choose(&l, "Pink", KEY, ORIGINAL).unwrap();
        assert!(originals_dir(&l, "Pink").is_dir());
        forget(&l, "Pink");
        assert!(!originals_dir(&l, "Pink").exists() && !choices_file(&l, "Pink").exists());
        let _ = fs::remove_dir_all(&l.root);
    }
}
