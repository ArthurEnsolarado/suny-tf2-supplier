//! Que mods brigam entre si: do Casual, de `custom` (instalados pelo app ou colocados à mão) e o que vai ser instalado.
//!
//! Dois níveis:
//! - **Barra** (`Level::Block`): os dois trazem o mesmo arquivo com conteúdo diferente, então só um vale. As partes de um
//!   modelo (`.mdl`, `.vvd`, `.vtx`, `.phy`, `.ani`) contam como UMA coisa: misturar partes de mods diferentes é o que
//!   mais quebra o jogo.
//! - **Avisa** (`Level::Warn`): arquivos diferentes que mexem no mesmo objeto (mesma arma, cosmético, personagem, HUD, ou
//!   variações do mesmo som). Pode combinar bem ou mal; quem decide é o usuário.
//!
//! Cópias idênticas (mesmo tamanho e CRC-32) não contam. O objeto é estimado pelo caminho: quando o caminho não diz o
//! que é, não há aviso (não chuta). Só leitura: nada aqui altera mods nem o jogo.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::casual;
use crate::installer;
use crate::library::{self, Role};
use crate::scanner;
use crate::{Error, Result};

/// Cada mod é lido até aqui (o mesmo limite do relatório de `custom`).
const MAX_FILES_PER_MOD: usize = 100_000;
const MAX_DEPTH: usize = 32;
/// Quantos exemplos de arquivo e de objeto vão para a interface.
const MAX_EXAMPLES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// Na lista do casual (o `id` é o nome da pasta no preloader).
    Casual,
    /// Em `tf/custom`.
    Custom,
    /// Ainda não instalado.
    New,
}

/// Impressão de um arquivo: tamanho e CRC-32. É o que o `.vpk` e o `.zip` já guardam, então não precisam ser lidos.
type Print = (u64, u32);

#[derive(Debug, Clone)]
enum Content {
    Disk(PathBuf),
    Known(Print),
}

impl Content {
    fn size(&self) -> Option<u64> {
        match self {
            Content::Disk(p) => fs::metadata(p).ok().map(|m| m.len()),
            Content::Known((s, _)) => Some(*s),
        }
    }

    fn crc(&self) -> Option<u32> {
        match self {
            Content::Known((_, c)) => Some(*c),
            Content::Disk(p) => {
                let mut f = fs::File::open(p).ok()?;
                let mut h = crc32fast::Hasher::new();
                let mut buf = vec![0u8; 1 << 16];
                loop {
                    let n = f.read(&mut buf).ok()?;
                    if n == 0 {
                        break;
                    }
                    h.update(&buf[..n]);
                }
                Some(h.finalize())
            }
        }
    }

    /// Na dúvida (arquivo que não deu para ler) são diferentes.
    fn same(&self, other: &Content) -> bool {
        match (self.size(), other.size()) {
            (Some(a), Some(b)) if a == b => matches!((self.crc(), other.crc()), (Some(x), Some(y)) if x == y),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModFiles {
    /// Casual: nome do addon. Custom: slug do app ou nome da entrada.
    pub id: String,
    pub name: String,
    pub place: Place,
    /// Item de origem (`Mod:123`): o mesmo item no casual E em custom é o mesmo mod, não uma disputa.
    pub origin: Option<String>,
    files: BTreeMap<String, Content>,
}

impl ModFiles {
    fn new(id: impl Into<String>, name: impl Into<String>, place: Place) -> Self {
        Self { id: id.into(), name: name.into(), place, origin: None, files: BTreeMap::new() }
    }

    /// Guarda o arquivo se for um arquivo de jogo que de fato disputa lugar.
    fn add(&mut self, raw: &str, content: Content) {
        let rel = scanner::game_relative(raw);
        if !scanner::is_game_path(&rel) || self.files.len() >= MAX_FILES_PER_MOD {
            return;
        }
        let base = rel.rsplit('/').next().unwrap_or("");
        // lixo e caches que o jogo e o sistema recriam
        if base.ends_with(".cache") || base == "thumbs.db" || base == "desktop.ini" {
            return;
        }
        // o preloader ignora os scripts de som que vêm dentro dos mods e escreve os dele
        if self.place == Place::Casual && rel.starts_with("scripts/") && base.contains("sound") && base.ends_with(".txt") {
            return;
        }
        self.files.insert(rel, content);
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Block,
    Warn,
}

/// O que os dois mexem. `kind`: `weapon`, `cosmetic`, `player`, `hud` ou `sound`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Object {
    pub kind: &'static str,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Side {
    pub id: String,
    pub name: String,
    pub place: Place,
}

#[derive(Debug, Clone, Serialize)]
pub struct Clash {
    pub a: Side,
    pub b: Side,
    pub level: Level,
    /// Arquivos em disputa direta (0 quando é só aviso).
    pub files: usize,
    pub examples: Vec<String>,
    pub objects: Vec<Object>,
}

// ---------------------------------------------------------------------------------------------------------------
// o que cada arquivo é

const MODEL_SUFFIXES: &[&str] = &[".dx80.vtx", ".dx90.vtx", ".sw.vtx", ".xbox.vtx", ".vtx", ".mdl", ".vvd", ".phy", ".ani"];

/// A "coisa" que o jogo troca de uma vez: um modelo inteiro, ou o arquivo.
fn unit(rel: &str) -> String {
    if rel.starts_with("models/") {
        if let Some(stem) = MODEL_SUFFIXES.iter().find_map(|s| rel.strip_suffix(s)) {
            return format!("model:{stem}");
        }
    }
    rel.to_string()
}

/// Finais de nome que são só outra textura do mesmo objeto (`c_headtaker_gold` é o machado de ouro).
const SKIN_SUFFIXES: &[&str] = &[
    "_gold", "_australium", "_blue", "_blu", "_red", "_normal", "_normals", "_bump", "_exponent", "_exp", "_phong", "_spec", "_specular", "_illum",
    "_selfillum", "_glow", "_rimexp", "_ambient", "_detail", "_skin", "_color", "_invuln",
];
/// Texturas auxiliares que muitos mods copiam; o nome não diz de qual objeto são.
const AUX_MARKS: &[&str] = &["lightwarp", "phongwarp", "envmap", "cubemap"];

fn strip_skin(mut name: &str) -> &str {
    while let Some(cut) = SKIN_SUFFIXES.iter().find_map(|s| name.strip_suffix(s)).filter(|c| c.len() > 2) {
        name = cut;
    }
    name
}

fn class_key(token: &str) -> Option<&'static str> {
    Some(match token {
        "scout" => "scout",
        "soldier" => "soldier",
        "pyro" => "pyro",
        "demo" | "demoman" => "demoman",
        "heavy" => "heavy",
        "engineer" | "engy" => "engineer",
        "medic" => "medic",
        "sniper" => "sniper",
        "spy" => "spy",
        _ => return None,
    })
}

fn object(kind: &'static str, name: &str) -> Option<Object> {
    (!name.is_empty()).then(|| Object { kind, name: name.to_string() })
}

/// O objeto que o arquivo mexe, quando o caminho deixa claro.
fn object_of(rel: &str) -> Option<Object> {
    let seg: Vec<&str> = rel.split('/').collect();
    let root = *seg.first()?;
    let file = *seg.last()?;
    let stem = file.split('.').next().unwrap_or(file);
    if matches!(root, "resource" | "ui" | "vgui") {
        return object("hud", "hud");
    }
    if root == "sound" {
        // `demo_charge_hit_world1..3` são sorteadas juntas: um mod em cada número mistura os dois
        let family = stem.trim_end_matches(|c: char| c.is_ascii_digit());
        if family.len() == stem.len() {
            return None;
        }
        let family = family.trim_end_matches('_');
        let dir = seg[1..seg.len() - 1].join("/");
        return (!family.is_empty()).then(|| Object { kind: "sound", name: if dir.is_empty() { family.to_string() } else { format!("{dir}/{family}") } });
    }
    if root != "models" && root != "materials" {
        return None;
    }
    if AUX_MARKS.iter().any(|m| stem.contains(m)) {
        return None;
    }
    // cosméticos: .../player/items/<classe>/<nome>[/...]
    if let Some(i) = seg.windows(2).position(|w| w == ["player", "items"]) {
        let rest = &seg[i + 2..];
        let skip = usize::from(rest.len() > 1 && (class_key(rest[0]).is_some() || matches!(rest[0], "all_class" | "all")));
        let name = rest.get(skip)?;
        let name = if rest.len() > skip + 1 { name } else { stem };
        return object("cosmetic", strip_skin(name));
    }
    // armas
    if let Some(i) = seg.iter().position(|s| *s == "weapons") {
        let rest = &seg[i + 1..];
        // c_models/<arma>/arquivo: a pasta diz a arma melhor que o nome da textura
        let folder = (rest.len() >= 3 && rest[0] == "c_models" && rest[1].starts_with("c_")).then(|| rest[1]);
        let name = folder.or_else(|| (stem.starts_with("c_") || stem.starts_with("v_") || stem.starts_with("w_")).then_some(stem))?;
        let name = strip_skin(name);
        let bare = ["c_", "v_", "w_"].iter().find_map(|p| name.strip_prefix(p)).unwrap_or(name);
        return object("weapon", bare);
    }
    // corpo das classes: models/player/demo.mdl, materials/models/player/demo/...
    if root == "models" && seg.get(1) == Some(&"player") && seg.len() == 3 {
        return class_key(stem.split('_').next().unwrap_or(stem)).and_then(|c| object("player", c));
    }
    if rel.starts_with("materials/models/player/") && seg.len() > 4 {
        return class_key(seg[3]).and_then(|c| object("player", c));
    }
    None
}

// ---------------------------------------------------------------------------------------------------------------
// comparação

#[derive(Default)]
struct Pair {
    files: usize,
    examples: BTreeSet<String>,
    objects: BTreeSet<Object>,
}

/// O mesmo item no casual e em custom é uma cópia do mesmo mod, não uma disputa.
fn same_mod_twice(a: &ModFiles, b: &ModFiles) -> bool {
    a.place != b.place && a.origin.is_some() && a.origin == b.origin
}

/// Todas as brigas entre os mods, as que barram primeiro. `only` limita aos pares em que esse índice participa.
fn compare(mods: &[ModFiles], only: Option<usize>) -> Vec<Clash> {
    // unidade → mod → seus arquivos; objeto → mod → unidades dele nesse objeto
    let mut units: HashMap<String, BTreeMap<usize, Vec<&str>>> = HashMap::new();
    let mut objects: BTreeMap<Object, BTreeMap<usize, BTreeSet<String>>> = BTreeMap::new();
    for (i, m) in mods.iter().enumerate() {
        for rel in m.files.keys() {
            let u = unit(rel);
            if let Some(o) = object_of(rel) {
                objects.entry(o).or_default().entry(i).or_default().insert(u.clone());
            }
            units.entry(u).or_default().entry(i).or_default().push(rel.as_str());
        }
    }

    let wanted = |a: usize, b: usize| only.is_none_or(|o| a == o || b == o) && !same_mod_twice(&mods[a], &mods[b]);
    let mut pairs: BTreeMap<(usize, usize), Pair> = BTreeMap::new();

    for owners in units.values().filter(|o| o.len() > 1) {
        let list: Vec<(&usize, &Vec<&str>)> = owners.iter().collect();
        for (x, (&a, fa)) in list.iter().enumerate() {
            for &(&b, fb) in &list[x + 1..] {
                if !wanted(a, b) {
                    continue;
                }
                let common: Vec<&str> = fa.iter().copied().filter(|p| fb.contains(p)).collect();
                let differ: Vec<&str> = common.iter().copied().filter(|p| !mods[a].files[*p].same(&mods[b].files[*p])).collect();
                // sem nenhum arquivo em comum só acontece com modelo: um traz uma parte e o outro, outra
                let clash: &[&str] = if common.is_empty() { fa } else { &differ };
                if clash.is_empty() {
                    continue;
                }
                let pair = pairs.entry((a, b)).or_default();
                pair.files += clash.len();
                for p in clash {
                    pair.examples.insert((*p).to_string());
                }
            }
        }
    }

    for (obj, owners) in &objects {
        if owners.len() < 2 {
            continue;
        }
        let list: Vec<(&usize, &BTreeSet<String>)> = owners.iter().collect();
        for (x, (&a, ua)) in list.iter().enumerate() {
            for &(&b, ub) in &list[x + 1..] {
                // os mesmos arquivos nos dois lados já foram julgados acima (iguais ou barrados)
                if wanted(a, b) && ua.iter().ne(ub.iter()) {
                    pairs.entry((a, b)).or_default().objects.insert(obj.clone());
                }
            }
        }
    }

    let side = |i: usize| Side { id: mods[i].id.clone(), name: mods[i].name.clone(), place: mods[i].place };
    let mut out: Vec<Clash> = pairs
        .into_iter()
        .map(|((a, b), p)| Clash {
            a: side(a),
            b: side(b),
            level: if p.files > 0 { Level::Block } else { Level::Warn },
            files: p.files,
            examples: p.examples.into_iter().take(MAX_EXAMPLES).collect(),
            objects: p.objects.into_iter().take(MAX_EXAMPLES).collect(),
        })
        .collect();
    out.sort_by(|x, y| x.level.cmp(&y.level).then_with(|| x.a.name.cmp(&y.a.name)).then_with(|| x.b.name.cmp(&y.b.name)));
    out
}

/// Todas as brigas entre todos os mods.
pub fn find(mods: &[ModFiles]) -> Vec<Clash> {
    compare(mods, None)
}

/// Só as brigas de `new` com os outros; `new` é sempre o lado `a`.
pub fn with(new: ModFiles, others: Vec<ModFiles>) -> Vec<Clash> {
    let mut all = vec![new];
    all.extend(others);
    compare(&all, Some(0))
}

/// A mensagem que explica por que o app não deixou juntar (a interface traduz).
pub fn refusal(c: &Clash) -> String {
    let place = |p: Place| match p {
        Place::Casual => "no Casual",
        Place::Custom => "em custom",
        Place::New => "novo",
    };
    format!(
        "Conflito: \"{}\" ({}) e \"{}\" ({}) trazem os mesmos arquivos ({}, ex.: {}). Só um dos dois pode ficar.",
        c.a.name,
        place(c.a.place),
        c.b.name,
        place(c.b.place),
        c.files,
        c.examples.first().map_or("?", String::as_str)
    )
}

pub fn first_block(clashes: &[Clash]) -> Option<&Clash> {
    clashes.iter().find(|c| c.level == Level::Block)
}

// ---------------------------------------------------------------------------------------------------------------
// as barreiras

/// A primeira briga que impede estes mods do casual de ficarem juntos (entre si ou com o que está em `custom`).
/// Com `added`, só interessa a briga que envolve um deles: desmarcar um mod nunca é barrado por uma briga antiga.
/// Brigas só entre itens de `custom` não são problema do casual.
pub fn casual_block(l: &casual::Layout, custom_dir: Option<&Path>, selected: &[String], added: Option<&[String]>) -> Result<Option<Clash>> {
    if added.is_some_and(|a| a.is_empty()) {
        return Ok(None);
    }
    let mut mods: Vec<ModFiles> = casual_mods(l).into_iter().filter(|m| selected.contains(&m.id)).collect();
    if let Some(dir) = custom_dir {
        mods.extend(custom_mods(dir)?);
    }
    let involved = |s: &Side| s.place == Place::Casual && added.is_none_or(|a| a.contains(&s.id));
    Ok(find(&mods).into_iter().find(|c| c.level == Level::Block && (involved(&c.a) || involved(&c.b))))
}

/// Com quem este arquivo brigaria, ANTES de instalar: extrai numa pasta temporária (o mesmo caminho da instalação,
/// então `.vpk` e pastas são julgados pelo que de fato iria para o jogo) e compara com `custom` e com o que está
/// marcado no casual. O arquivo é o lado `a`. A própria versão antiga (mesmo nome de pasta) não conta: reinstalar é
/// trocar. Sem a pasta do jogo, compara só com o casual.
pub fn preview_install(zip_path: &Path, custom_dir: Option<&Path>, meta: &installer::ModMeta, l: &casual::Layout) -> Result<Vec<Clash>> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let serial = PRECHECK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let staging = std::env::temp_dir().join("suny-tf2-supplier").join(format!("precheck-{}-{nanos}-{serial}", std::process::id()));
    let judged = (|| -> Result<Vec<Clash>> {
        installer::install_zip(zip_path, &staging, meta.clone())?;
        let origin = (meta.gb_id != 0).then(|| format!("{}:{}", meta.kind, meta.gb_id));
        let new = new_mod(&staging, &meta.name, origin)?;
        let slug = installer::slug_for(meta);
        let mut others: Vec<ModFiles> = Vec::new();
        if let Some(dir) = custom_dir {
            others.extend(custom_mods(dir)?.into_iter().filter(|m| m.id != slug));
        }
        let enabled = casual::enabled_names(l);
        others.extend(casual_mods(l).into_iter().filter(|m| enabled.contains(&m.id)));
        Ok(with(new, others))
    })();
    let _ = fs::remove_dir_all(&staging);
    judged
}

static PRECHECK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A barreira antes de um mod ir para `custom`.
pub fn check_install(zip_path: &Path, custom_dir: &Path, meta: &installer::ModMeta, l: &casual::Layout) -> Result<()> {
    match first_block(&preview_install(zip_path, Some(custom_dir), meta, l)?) {
        Some(c) => Err(Error::Conflict(refusal(c))),
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// de onde vêm os mods

fn walk(base: &Path, dir: &Path, depth: usize, into: &mut ModFiles) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue; // nunca segue atalhos para fora da pasta do mod
        }
        let path = e.path();
        if ft.is_dir() {
            walk(base, &path, depth + 1, into);
        } else if let Ok(rel) = path.strip_prefix(base) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            into.add(&rel, Content::Disk(path.clone()));
        }
    }
}

/// Os mods que estão em `custom`: os do app (um por manifesto, mesmo com vários `.vpk`) e os colocados à mão.
/// Ficam de fora o VPK do próprio preloader (é o casual aplicado), a pasta de backup dele e a `workshop`.
/// Um `.vpk` ilegível é ignorado: não dá para dizer o que ele mexe.
pub fn custom_mods(custom_dir: &Path) -> Result<Vec<ModFiles>> {
    let mut by_id: BTreeMap<String, ModFiles> = BTreeMap::new();
    for entry in library::scan_custom(custom_dir)?.into_iter().filter(|e| e.role == Role::Mod) {
        let (id, name, origin) = match &entry.managed {
            Some(m) => (m.slug.clone(), m.name.clone(), (m.gb_id != 0).then(|| format!("{}:{}", m.kind, m.gb_id))),
            None => (entry.name.clone(), entry.name.clone(), None),
        };
        let item = by_id.entry(id.clone()).or_insert_with(|| {
            let mut m = ModFiles::new(id, name, Place::Custom);
            m.origin = origin;
            m
        });
        let path = custom_dir.join(&entry.name);
        if entry.is_folder {
            walk(&path, &path, 0, item);
        } else if entry.name.to_ascii_lowercase().ends_with(".vpk") {
            if let Ok(list) = crate::gameicons::list_prints(&path) {
                for (inner, size, crc) in list {
                    item.add(&inner, Content::Known((size, crc)));
                }
            }
        }
    }
    Ok(by_id.into_values().filter(|m| !m.is_empty()).collect())
}

/// Os mods da lista do casual (marcados ou não).
pub fn casual_mods(l: &casual::Layout) -> Vec<ModFiles> {
    casual::list_addons(l)
        .into_iter()
        .map(|a| {
            let base = a.source.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| a.name.clone());
            // duas versões do mesmo pacote precisam aparecer com nomes diferentes na lista de conflitos
            let shown = a.variant.as_ref().map_or_else(|| base.clone(), |v| format!("{base} [{v}]"));
            let mut m = ModFiles::new(a.name.clone(), shown, Place::Casual);
            m.origin = a.source.as_ref().filter(|s| s.gb_id != 0).map(|s| format!("{}:{}", s.kind, s.gb_id));
            // variantes do mesmo pacote têm o mesmo nome de origem: o nome da pasta diz qual é qual
            let dir = l.addons_dir().join(&a.name);
            walk(&dir, &dir, 0, &mut m);
            m
        })
        .collect()
}

/// Um mod que acabou de ser extraído numa pasta `custom` temporária, para ser julgado antes de ir para o jogo.
pub fn new_mod(staged_custom: &Path, name: &str, origin: Option<String>) -> Result<ModFiles> {
    let mut all = ModFiles::new(name, name, Place::New);
    all.origin = origin;
    for m in custom_mods(staged_custom)? {
        all.files.extend(m.files);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(name: &str, place: Place, files: &[(&str, u32)]) -> ModFiles {
        let mut out = ModFiles::new(name, name, place);
        for (p, crc) in files {
            out.add(p, Content::Known((10, *crc)));
        }
        out
    }

    fn obj(kind: &'static str, name: &str) -> Object {
        Object { kind, name: name.into() }
    }

    #[test]
    fn the_same_file_with_different_content_blocks() {
        let r = find(&[m("Roxo", Place::Casual, &[("materials/models/weapons/c_items/c_headtaker.vmt", 1)]), m("Fogo", Place::Custom, &[("Pack/materials/models/weapons/c_items/c_headtaker.vmt", 2)])]);
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].level, r[0].files), (Level::Block, 1));
        assert_eq!(r[0].examples, vec!["materials/models/weapons/c_items/c_headtaker.vmt"]);
        assert!(refusal(&r[0]).contains("Roxo") && refusal(&r[0]).contains("em custom"));
    }

    #[test]
    fn identical_copies_do_not_count() {
        let files = [("materials/models/weapons/c_items/c_headtaker.vmt", 7), ("sound/weapons/demo_sword_swing1.wav", 8)];
        assert!(find(&[m("A", Place::Casual, &files), m("B", Place::Custom, &files)]).is_empty());
    }

    #[test]
    fn two_headtakers_with_different_files_only_warn_about_the_same_weapon() {
        // o exemplo do pedido: um australium (textura de ouro) e um com efeito roxo
        let r = find(&[
            m("Australium", Place::Casual, &[("materials/models/weapons/c_models/c_headtaker/c_headtaker_gold.vtf", 1)]),
            m("Roxo", Place::Casual, &[("materials/models/weapons/c_items/c_headtaker.vmt", 2)]),
        ]);
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].level, r[0].files), (Level::Warn, 0));
        assert_eq!(r[0].objects, vec![obj("weapon", "headtaker")]);
    }

    #[test]
    fn parts_of_one_model_from_two_mods_block_even_without_a_common_file() {
        let r = find(&[m("A", Place::Custom, &[("models/weapons/c_models/c_bat/c_bat.mdl", 1)]), m("B", Place::Custom, &[("models/weapons/c_models/c_bat/c_bat.dx90.vtx", 2)])]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].level, Level::Block);
        // animações de classe são um modelo também (.mdl + .ani)
        let r = find(&[m("A", Place::Casual, &[("models/weapons/c_models/c_scout_animations.mdl", 1)]), m("B", Place::Casual, &[("models/weapons/c_models/c_scout_animations.ani", 1)])]);
        assert_eq!(r[0].level, Level::Block);
    }

    #[test]
    fn variations_of_one_sound_split_between_two_mods_warn() {
        let r = find(&[m("A", Place::Casual, &[("sound/weapons/demo_charge_hit_world1.wav", 1)]), m("B", Place::Casual, &[("sound/weapons/demo_charge_hit_world2.wav", 2)])]);
        assert_eq!(r[0].level, Level::Warn);
        assert_eq!(r[0].objects, vec![obj("sound", "weapons/demo_charge_hit_world")]);
    }

    #[test]
    fn each_class_with_its_own_file_is_free() {
        let r = find(&[
            m("Scout", Place::Casual, &[("sound/vo/scout_battlecry01.mp3", 1), ("models/weapons/c_models/c_scout_animations.mdl", 2), ("materials/models/player/scout/scout_red.vtf", 3)]),
            m("Spy", Place::Casual, &[("sound/vo/spy_battlecry01.mp3", 4), ("models/weapons/c_models/c_spy_animations.mdl", 5), ("materials/models/player/spy/spy_red.vtf", 6)]),
            m("Hit", Place::Custom, &[("sound/ui/hitsound_a.wav", 7)]),
            m("Kill", Place::Custom, &[("sound/ui/killsound.wav", 8)]),
        ]);
        assert!(r.is_empty(), "{r:?}");
    }

    #[test]
    fn two_huds_warn_even_without_common_files_and_a_shared_lightwarp_name_says_nothing() {
        let r = find(&[m("HUD 1", Place::Custom, &[("resource/ui/hudplayerhealth.res", 1)]), m("HUD 2", Place::Custom, &[("resource/clientscheme.res", 2)])]);
        assert_eq!(r[0].objects, vec![obj("hud", "hud")]);
        let r = find(&[
            m("A", Place::Casual, &[("materials/models/weapons/c_items/c_ambassador_lightwarp.vtf", 1)]),
            m("B", Place::Casual, &[("materials/models/weapons/c_items/c_ambassador_phongwarp.vtf", 2)]),
        ]);
        assert!(r.is_empty(), "{r:?}");
    }

    #[test]
    fn objects_are_read_from_the_path_and_never_guessed() {
        assert_eq!(object_of("models/weapons/c_models/c_headtaker/c_headtaker.mdl"), Some(obj("weapon", "headtaker")));
        assert_eq!(object_of("materials/models/weapons/v_machete/v_machete_red.vtf"), Some(obj("weapon", "machete")));
        assert_eq!(object_of("models/workshop/player/items/sniper/cool_hat/cool_hat.mdl"), Some(obj("cosmetic", "cool_hat")));
        assert_eq!(object_of("materials/models/player/items/demo/top_hat_blue.vtf"), Some(obj("cosmetic", "top_hat")));
        assert_eq!(object_of("models/player/demo.mdl"), Some(obj("player", "demoman")));
        assert_eq!(object_of("materials/models/player/demo/eyeball_l.vmt"), Some(obj("player", "demoman")));
        for unknown in ["materials/models/weapons/shared/shine.vtf", "materials/effects/fire.vtf", "sound/ui/killsound.wav", "particles/a.pcf", "scripts/items/items_game.txt", "models/props/x.mdl"] {
            assert_eq!(object_of(unknown), None, "{unknown}");
        }
    }

    #[test]
    fn the_same_gamebanana_item_in_casual_and_custom_is_one_mod_but_two_variants_still_fight() {
        let file = "models/weapons/c_models/c_bat/c_bat.mdl";
        let mut a = m("A", Place::Casual, &[(file, 1)]);
        let mut b = m("A em custom", Place::Custom, &[(file, 2)]);
        a.origin = Some("Mod:5".into());
        b.origin = Some("Mod:5".into());
        assert!(find(&[a.clone(), b]).is_empty());
        let mut v = m("A variante", Place::Casual, &[(file, 3)]);
        v.origin = Some("Mod:5".into());
        assert_eq!(find(&[a, v])[0].level, Level::Block);
    }

    #[test]
    fn only_the_pairs_of_the_new_mod_are_reported_when_checking_an_install() {
        let file = "sound/ui/killsound.wav";
        let r = with(m("Novo", Place::New, &[(file, 1)]), vec![m("B", Place::Custom, &[(file, 2)]), m("C", Place::Casual, &[(file, 3)])]);
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|c| c.a.name == "Novo"));
    }

    #[test]
    fn game_junk_and_preloader_ignored_scripts_do_not_fight() {
        let files = [("sound/sound.cache", 1), ("scripts/game_sounds_weapons.txt", 2), ("readme.txt", 3)];
        let other = [("sound/sound.cache", 9), ("scripts/game_sounds_weapons.txt", 9), ("readme.txt", 9)];
        assert!(find(&[m("A", Place::Casual, &files), m("B", Place::Casual, &other)]).is_empty());
        assert_eq!(find(&[m("A", Place::Custom, &files), m("B", Place::Custom, &other)])[0].files, 1, "em custom o script de som vale e disputa");
    }

    fn zip_with(path: &Path, files: &[(&str, &[u8])]) {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (n, data) in files {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    fn meta(name: &str, gb_id: u64) -> installer::ModMeta {
        installer::ModMeta { gb_id, kind: "Mod".into(), file_id: gb_id, file_name: "x.zip".into(), name: name.into(), version: None, submitter: None, profile_url: String::new(), license: None, md5: None }
    }

    #[test]
    fn an_install_that_fights_with_custom_or_casual_never_reaches_the_game() {
        let d = std::env::temp_dir().join(format!("suny_guard_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let (custom, layout) = (d.join("custom"), casual::Layout::new(d.join("casual")));
        let zip = |n: &str, files: &[(&str, &[u8])]| {
            let p = d.join(n);
            zip_with(&p, files);
            p
        };
        let axe = "materials/models/weapons/c_items/c_headtaker.vmt";
        let first = zip("a.zip", &[(axe, b"purple")]);
        check_install(&first, &custom, &meta("Roxo", 1), &layout).unwrap();
        installer::install_zip(&first, &custom, meta("Roxo", 1)).unwrap();
        // a mesma pasta (atualização do mesmo mod) passa
        check_install(&zip("a2.zip", &[(axe, b"purple v2")]), &custom, &meta("Roxo", 1), &layout).unwrap();

        let other = zip("b.zip", &[(axe, b"gold")]);
        let err = check_install(&other, &custom, &meta("Dourado", 2), &layout).unwrap_err().to_string();
        assert!(err.contains("Dourado") && err.contains("Roxo") && err.contains("em custom"), "{err}");
        assert_eq!(installer::list_managed(&custom).len(), 1, "nada foi escrito em custom");

        // contra um mod marcado no casual
        let addon = layout.addons_dir().join("Casual Axe (gbm3)").join("sound/ui");
        fs::create_dir_all(&addon).unwrap();
        fs::write(addon.join("killsound.wav"), b"casual").unwrap();
        let kill = zip("c.zip", &[("sound/ui/killsound.wav", b"mine")]);
        check_install(&kill, &custom, &meta("Kill", 4), &layout).expect("desmarcado no casual não barra");
        casual::set_enabled(&layout, vec!["Casual Axe (gbm3)".into()]).unwrap();
        let err = check_install(&kill, &custom, &meta("Kill", 4), &layout).unwrap_err().to_string();
        assert!(err.contains("no Casual"), "{err}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn casual_only_refuses_what_is_being_added_and_ignores_fights_that_are_only_in_custom() {
        let d = std::env::temp_dir().join(format!("suny_cblock_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let layout = casual::Layout::new(d.join("casual"));
        let put = |base: &Path, rel: &str, data: &[u8]| {
            let p = base.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, data).unwrap();
        };
        for (name, data) in [("A", b"1"), ("B", b"2")] {
            put(&layout.addons_dir().join(name), "sound/ui/killsound.wav", data);
        }
        put(&layout.addons_dir().join("C"), "sound/ui/hitsound.wav", b"3");
        let custom = d.join("custom");
        put(&custom, "x/sound/vo/a_line.mp3", b"x");
        put(&custom, "y/sound/vo/a_line.mp3", b"y");

        let all: Vec<String> = ["A", "B", "C"].map(String::from).to_vec();
        let hit = casual_block(&layout, Some(&custom), &all, None).unwrap().expect("A e B brigam");
        assert_eq!((hit.a.name.as_str(), hit.b.name.as_str()), ("A", "B"));
        assert!(casual_block(&layout, Some(&custom), &all, Some(&["C".into()])).unwrap().is_none(), "C não entra na briga antiga");
        assert!(casual_block(&layout, Some(&custom), &all, Some(&[])).unwrap().is_none(), "só desmarcar");
        assert!(casual_block(&layout, Some(&custom), &["A".into(), "C".into()], None).unwrap().is_none(), "a briga x/y é só de custom");

        put(&custom, "z/sound/ui/hitsound.wav", b"other");
        let hit = casual_block(&layout, Some(&custom), &["C".into()], None).unwrap().expect("casual contra custom");
        assert!(refusal(&hit).contains("em custom"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn custom_folders_vpks_and_disk_content_are_compared_for_real() {
        let d = std::env::temp_dir().join(format!("suny_clash_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let put = |rel: &str, data: &[u8]| {
            let p = d.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, data).unwrap();
        };
        put("hand A/sound/ui/killsound.wav", b"AAAA");
        put("hand B/sound/ui/killsound.wav", b"BBBB");
        put("copy of A/sound/ui/killsound.wav", b"AAAA");
        put("_casual_preloader_dir.vpk", b"not read");
        put("workshop/sound/ui/killsound.wav", b"zzzz");
        let r = find(&custom_mods(&d).unwrap());
        let mut pairs: Vec<(String, String)> = r.iter().map(|c| (c.a.name.clone(), c.b.name.clone())).collect();
        pairs.sort();
        assert_eq!(pairs, vec![("copy of A".to_string(), "hand B".to_string()), ("hand A".to_string(), "hand B".to_string())]);
        let _ = fs::remove_dir_all(&d);
    }
}
