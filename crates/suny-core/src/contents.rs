//! O que cada item de `custom` muda e onde dois itens disputam o mesmo arquivo.
//! Só leitura: lê índices de `.vpk` e nomes de arquivos de pastas, nunca altera nada.
//!
//! Quando dois itens trazem o mesmo caminho (por exemplo `models/weapons/c_claymore/c_claymore.mdl`), o jogo usa
//! um só. A ordem em que ele procura é a ordem alfabética dos nomes em `custom` (sem diferenciar maiúsculas):
//! o primeiro da fila vence. É o mesmo motivo de o Casual Preloader se chamar `_casual_preloader...`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::library::{CustomEntry, Role};
use crate::replaces::{self, Group};
use crate::scanner;

/// Cada item é lido até aqui; passou disso, o resultado vem marcado como parcial.
pub const MAX_FILES_PER_ITEM: usize = if cfg!(test) { 200 } else { 100_000 };
/// Profundidade máxima ao percorrer uma pasta de mod.
const MAX_DEPTH: usize = 32;
/// Quantos grupos de disputa vão para a interface.
const MAX_CONFLICTS: usize = 40;
/// Quantos grupos de conteúdo por item.
const MAX_GROUPS: usize = 6;
/// Quantos outros itens aparecem na lista de "disputa com" de cada item.
const MAX_RIVALS: usize = 8;

#[derive(Debug, Clone, Serialize)]
pub struct ItemContents {
    /// Mesmo nome de `CustomEntry.name`.
    pub name: String,
    /// Arquivos de jogo (models, sound, materials...) que ele traz.
    pub files: usize,
    /// Passou de `MAX_FILES_PER_ITEM`: a contagem é parcial.
    pub truncated: bool,
    pub groups: Vec<Group>,
    pub classes: Vec<String>,
    /// Arquivos em que ele é o primeiro da fila, entre os que outro item também traz.
    pub wins: usize,
    /// Arquivos que outro item, antes dele na fila, também traz (o dele é ignorado).
    pub loses: usize,
    /// Os outros itens com quem ele disputa algum arquivo (no máximo `MAX_RIVALS`).
    pub rivals: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    /// Itens que trazem os mesmos arquivos, na ordem da fila (o primeiro vence).
    pub items: Vec<String>,
    pub winner: String,
    /// Quantos arquivos têm exatamente esse conjunto de itens.
    pub count: usize,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Report {
    pub items: Vec<ItemContents>,
    pub conflicts: Vec<Conflict>,
    /// Total de arquivos disputados por dois ou mais itens.
    pub conflict_files: usize,
    /// Itens cujo conteúdo não deu para ler (`.vpk` quebrado ou pasta ilegível).
    pub unreadable: Vec<String>,
}

fn is_data_part(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.len() > 8 && l.ends_with(".vpk") && l[..l.len() - 4].rsplit_once('_').is_some_and(|(_, n)| n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Caminho de dentro de um mod/VPK → caminho relativo ao jogo (minúsculo, com `/`), ou `None` se não for arquivo de jogo.
fn game_path(raw: &str) -> Option<String> {
    let rel = scanner::game_relative(raw).to_ascii_lowercase();
    scanner::is_game_path(&rel).then_some(rel)
}

fn walk_folder(base: &Path, dir: &Path, depth: usize, out: &mut Vec<String>, truncated: &mut bool) -> bool {
    if depth > MAX_DEPTH {
        return true;
    }
    let Ok(rd) = fs::read_dir(dir) else { return false };
    for e in rd.flatten() {
        if out.len() >= MAX_FILES_PER_ITEM {
            *truncated = true;
            return true;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue; // nunca segue atalhos para fora da pasta do mod
        }
        let path = e.path();
        if ft.is_dir() {
            walk_folder(base, &path, depth + 1, out, truncated);
        } else if ft.is_file() {
            if let Ok(rel) = path.strip_prefix(base) {
                if let Some(p) = game_path(&rel.to_string_lossy().replace('\\', "/")) {
                    out.push(p);
                }
            }
        }
    }
    true
}

/// Caminhos de jogo de um item. `None` = não deu para ler.
fn paths_of(custom_dir: &Path, entry: &CustomEntry) -> Option<(Vec<String>, bool)> {
    let path = custom_dir.join(&entry.name);
    let mut out = Vec::new();
    let mut truncated = false;
    if entry.is_folder {
        let ok = walk_folder(&path, &path, 0, &mut out, &mut truncated);
        return ok.then_some((out, truncated));
    }
    let l = entry.name.to_ascii_lowercase();
    if !l.ends_with(".vpk") || is_data_part(&entry.name) {
        return Some((out, false));
    }
    let list = crate::gameicons::list_paths(&path, |_| true).ok()?;
    for raw in list {
        if out.len() >= MAX_FILES_PER_ITEM {
            truncated = true;
            break;
        }
        if let Some(p) = game_path(&raw) {
            out.push(p);
        }
    }
    Some((out, truncated))
}

/// Lê o conteúdo de cada item de `custom` e acha os arquivos que mais de um item traz.
pub fn report(custom_dir: &Path, entries: &[CustomEntry]) -> Report {
    // Fila de busca do jogo: ordem alfabética sem diferenciar maiúsculas.
    let mut order: Vec<&CustomEntry> = entries.iter().filter(|e| matches!(e.role, Role::Mod | Role::Preloader)).collect();
    order.sort_by_key(|e| e.name.to_lowercase());

    let mut rep = Report::default();
    // item (posição na fila) → seus caminhos
    let mut all: Vec<(usize, Vec<String>, bool)> = Vec::new();
    for (pos, e) in order.iter().enumerate() {
        match paths_of(custom_dir, e) {
            Some((paths, truncated)) => {
                if !paths.is_empty() {
                    all.push((pos, paths, truncated));
                }
            }
            None => rep.unreadable.push(e.name.clone()),
        }
    }

    // caminho → itens (posições) que o trazem
    let mut by_path: HashMap<&str, Vec<usize>> = HashMap::new();
    for (pos, paths, _) in &all {
        for p in paths {
            let v = by_path.entry(p.as_str()).or_default();
            if v.last() != Some(pos) {
                v.push(*pos);
            }
        }
    }

    let mut sets: HashMap<Vec<usize>, (usize, Vec<String>)> = HashMap::new();
    let mut wins = vec![0usize; order.len()];
    let mut loses = vec![0usize; order.len()];
    let mut rivals: Vec<Vec<usize>> = vec![Vec::new(); order.len()];
    for (path, owners) in &by_path {
        if owners.len() < 2 {
            continue;
        }
        rep.conflict_files += 1;
        wins[owners[0]] += 1;
        for &o in &owners[1..] {
            loses[o] += 1;
        }
        for &a in owners {
            for &b in owners {
                if a != b && !rivals[a].contains(&b) && rivals[a].len() < MAX_RIVALS {
                    rivals[a].push(b);
                }
            }
        }
        let slot = sets.entry(owners.clone()).or_insert_with(|| (0, Vec::new()));
        slot.0 += 1;
        if slot.1.len() < 3 {
            slot.1.push((*path).to_string());
        }
    }

    for (pos, paths, truncated) in &all {
        let groups = replaces::group(paths, None);
        let classes = replaces::classes_affected(&groups);
        let mut r = rivals[*pos].clone();
        r.sort_unstable();
        rep.items.push(ItemContents {
            name: order[*pos].name.clone(),
            files: paths.len(),
            truncated: *truncated,
            groups: groups.into_iter().take(MAX_GROUPS).collect(),
            classes,
            wins: wins[*pos],
            loses: loses[*pos],
            rivals: r.into_iter().map(|i| order[i].name.clone()).collect(),
        });
    }

    let mut list: Vec<(Vec<usize>, (usize, Vec<String>))> = sets.into_iter().collect();
    list.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(&b.0)));
    rep.conflicts = list
        .into_iter()
        .take(MAX_CONFLICTS)
        .map(|(owners, (count, mut examples))| {
            examples.sort();
            Conflict { winner: order[owners[0]].name.clone(), items: owners.iter().map(|&i| order[i].name.clone()).collect(), count, examples }
        })
        .collect();
    rep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::scan_custom;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("suny_contents_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn put(base: &Path, rel: &str) {
        let p = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    #[test]
    fn two_folders_with_the_same_file_fight_and_the_first_in_the_alphabet_wins() {
        let d = tmp("fight");
        put(&d, "Zeta Sword/models/weapons/c_claymore/c_claymore.mdl");
        put(&d, "Zeta Sword/readme.txt");
        put(&d, "alpha sword/models/weapons/c_claymore/c_claymore.mdl");
        put(&d, "alpha sword/sound/ui/hit.wav");
        put(&d, "Only Hud/resource/ui/hudplayerhealth.res");

        let entries = scan_custom(&d).unwrap();
        let r = report(&d, &entries);
        assert_eq!(r.conflict_files, 1);
        assert_eq!(r.conflicts.len(), 1);
        assert_eq!(r.conflicts[0].winner, "alpha sword", "sem diferenciar maiúsculas: 'alpha' vem antes de 'Zeta'");
        assert_eq!(r.conflicts[0].items, vec!["alpha sword", "Zeta Sword"]);
        assert_eq!(r.conflicts[0].examples, vec!["models/weapons/c_claymore/c_claymore.mdl"]);

        let by = |n: &str| r.items.iter().find(|i| i.name == n).unwrap();
        assert_eq!((by("alpha sword").wins, by("alpha sword").loses), (1, 0));
        assert_eq!((by("Zeta Sword").wins, by("Zeta Sword").loses), (0, 1));
        assert_eq!(by("Zeta Sword").files, 1, "readme.txt não é arquivo de jogo");
        assert_eq!(by("Zeta Sword").rivals, vec!["alpha sword"]);
        assert!(by("Only Hud").rivals.is_empty() && by("Only Hud").loses == 0);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_vpk_and_a_folder_are_compared_by_what_is_inside() {
        let d = tmp("vpk");
        // VPK mínimo v1 com um arquivo: models/weapons/w.mdl
        fn tiny_vpk(path: &Path, inner: (&str, &str, &str)) {
            let mut tree = Vec::new();
            for s in [inner.0, inner.1, inner.2] {
                tree.extend_from_slice(s.as_bytes());
                tree.push(0);
            }
            // CRC, preload bytes, arquivo (0x7fff = dentro do dir), offset, length, terminador
            tree.extend_from_slice(&0u32.to_le_bytes());
            tree.extend_from_slice(&0u16.to_le_bytes());
            tree.extend_from_slice(&0x7fffu16.to_le_bytes());
            tree.extend_from_slice(&0u32.to_le_bytes());
            tree.extend_from_slice(&0u32.to_le_bytes());
            tree.extend_from_slice(&0xffffu16.to_le_bytes());
            tree.extend_from_slice(&[0, 0, 0]);
            let mut out = Vec::new();
            out.extend_from_slice(&0x55aa1234u32.to_le_bytes());
            out.extend_from_slice(&1u32.to_le_bytes());
            out.extend_from_slice(&(tree.len() as u32).to_le_bytes());
            out.extend_from_slice(&tree);
            fs::write(path, out).unwrap();
        }
        tiny_vpk(&d.join("a_pack.vpk"), ("mdl", "models/weapons", "w"));
        put(&d, "b_folder/models/weapons/w.mdl");

        let entries = scan_custom(&d).unwrap();
        let r = report(&d, &entries);
        assert!(r.unreadable.is_empty(), "{:?}", r.unreadable);
        assert_eq!(r.conflict_files, 1, "{r:?}");
        assert_eq!(r.conflicts[0].winner, "a_pack.vpk");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn data_parts_workshop_and_broken_vpks_are_handled() {
        let d = tmp("odd");
        fs::write(d.join("big_000.vpk"), b"data").unwrap();
        fs::write(d.join("broken.vpk"), b"not a vpk").unwrap();
        put(&d, "workshop/models/x.mdl");
        let entries = scan_custom(&d).unwrap();
        let r = report(&d, &entries);
        assert!(r.items.is_empty());
        assert_eq!(r.unreadable, vec!["broken.vpk"]);
        assert!(r.conflicts.is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_big_folder_is_cut_and_marked_as_partial() {
        let d = tmp("cut");
        let base = d.join("many");
        fs::create_dir_all(base.join("sound")).unwrap();
        for i in 0..(MAX_FILES_PER_ITEM + 5) {
            fs::write(base.join("sound").join(format!("{i}.wav")), b"").unwrap();
        }
        let entries = scan_custom(&d).unwrap();
        let r = report(&d, &entries);
        assert!(r.items[0].truncated);
        assert_eq!(r.items[0].files, MAX_FILES_PER_ITEM);
        let _ = fs::remove_dir_all(&d);
    }
}
