//! Traduz caminhos de arquivo em "o que este mod muda": "Hitsound", "Voz do Demoman", "Armas do Soldier".
//!
//! O GameBanana não diz qual som ou classe um pacote troca; isto é **estimado pelo caminho** do arquivo
//! (e confirmado contra o índice dos arquivos originais do jogo quando ele existe).
//! Quando o rótulo é de uma classe, `class` traz a chave do ícone (`demoman`, `scout`...), que a interface
//! mostra com o ícone que o próprio TF2 do usuário tem.

use std::collections::HashMap;

use serde::Serialize;

use crate::scanner::{ContentKind, VanillaIndex};

/// (chave do ícone, nome para exibir)
pub const CLASSES: &[(&str, &str)] = &[
    ("scout", "Scout"),
    ("soldier", "Soldier"),
    ("pyro", "Pyro"),
    ("demoman", "Demoman"),
    ("heavy", "Heavy"),
    ("engineer", "Engineer"),
    ("medic", "Medic"),
    ("sniper", "Sniper"),
    ("spy", "Spy"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub text: String,
    pub class: Option<&'static str>,
}

fn l(text: impl Into<String>, class: Option<&'static str>) -> Label {
    Label { text: text.into(), class }
}

fn class_of_token(tok: &str) -> Option<&'static str> {
    Some(match tok {
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

fn class_name(key: &str) -> &'static str {
    CLASSES.iter().find(|(k, _)| *k == key).map_or("?", |(_, n)| *n)
}

/// Primeira "palavra" do caminho que é nome de classe (`models/player/demo.mdl` → demoman).
fn class_in(rel: &str) -> Option<&'static str> {
    rel.split(['/', '_', '.', '-']).find_map(class_of_token)
}

fn with_class(prefix: &str, class: Option<&'static str>, fallback: &str) -> Label {
    match class {
        Some(c) => l(format!("{prefix} {}", class_name(c)), Some(c)),
        None => l(fallback, None),
    }
}

/// Prefixos de arquivo em `sound/weapons/` que pertencem claramente a uma classe.
const WEAPON_CLASS: &[(&str, &str)] = &[
    ("demo_", "demoman"),
    ("pipebomb", "demoman"),
    ("sticky", "demoman"),
    ("grenade_launcher", "demoman"),
    ("sniper", "sniper"),
    ("bow", "sniper"),
    ("huntsman", "sniper"),
    ("scatter", "scout"),
    ("bat_", "scout"),
    ("scout", "scout"),
    ("rocket", "soldier"),
    ("soldier", "soldier"),
    ("shovel", "soldier"),
    ("flame", "pyro"),
    ("flare", "pyro"),
    ("fireaxe", "pyro"),
    ("pyro", "pyro"),
    ("minigun", "heavy"),
    ("heavy", "heavy"),
    ("tomislav", "heavy"),
    ("wrench", "engineer"),
    ("sentry", "engineer"),
    ("dispenser", "engineer"),
    ("teleport", "engineer"),
    ("engineer", "engineer"),
    ("medigun", "medic"),
    ("syringe", "medic"),
    ("bonesaw", "medic"),
    ("medic", "medic"),
    ("knife", "spy"),
    ("revolver", "spy"),
    ("cloak", "spy"),
    ("sapper", "spy"),
    ("spy", "spy"),
];

fn weapon_label(stem: &str) -> Label {
    if let Some((_, c)) = WEAPON_CLASS.iter().find(|(p, _)| stem.starts_with(p)) {
        return l(format!("Armas do {}", class_name(c)), Some(c));
    }
    if ["shotgun", "pistol", "smg"].iter().any(|p| stem.starts_with(p)) {
        return l("Armas compartilhadas (pistola, shotgun, SMG)", None);
    }
    l("Sons de armas", None)
}

fn vo_label(seg: &[&str], stem: &str) -> Label {
    // sound/vo/taunts/<classe>/...
    if seg.get(2) == Some(&"taunts") {
        return match seg.get(3).and_then(|s| class_of_token(s)) {
            Some(c) => l(format!("Provocações (voz) do {}", class_name(c)), Some(c)),
            None => l("Provocações (voz)", None),
        };
    }
    // sound/vo/<classe>/arquivo
    if seg.len() >= 4 {
        if let Some(c) = class_of_token(seg[2]) {
            return l(format!("Voz do {}", class_name(c)), Some(c));
        }
    }
    // sound/vo/<classe>_arquivo
    let first = stem.split('_').next().unwrap_or("");
    if let Some(c) = class_of_token(first) {
        return l(format!("Voz do {}", class_name(c)), Some(c));
    }
    match first {
        "announcer" => l("Locutor (announcer)", None),
        "mvm" => l("Vozes do Mann vs. Machine", None),
        _ => l("Vozes (outras)", None),
    }
}

fn sound_label(seg: &[&str], stem: &str) -> Label {
    match seg.get(1).copied().unwrap_or("") {
        "ui" if stem.starts_with("hitsound") => l("Hitsound", None),
        "ui" if stem.starts_with("killsound") => l("Killsound", None),
        "ui" if stem.contains("gamestartup") || stem.contains("menu_music") => l("Música do menu", None),
        "ui" => l("Sons de interface", None),
        "vo" => vo_label(seg, stem),
        "weapons" => weapon_label(stem),
        "player" if seg.get(2) == Some(&"footsteps") => l("Passos", None),
        "player" if stem.contains("crit") => l("Críticos", None),
        "player" if stem.contains("pain") || stem.contains("death") => l("Dano e morte", None),
        "player" => l("Sons do jogador", None),
        "misc" if stem.contains("freeze_cam") || stem.contains("freezecam") => l("Freeze cam", None),
        "misc" if stem.contains("killstreak") => l("Killstreak", None),
        "misc" => l("Sons diversos", None),
        "music" => l("Música", None),
        "items" if seg.get(2) == Some(&"taunts") => l("Provocações (música)", None),
        "items" => l("Sons de itens", None),
        "mvm" => l("Mann vs. Machine", None),
        _ => l("Sons (outros)", None),
    }
}

/// `rel` é o caminho relativo ao jogo, em minúsculas e com `/` (o que `scanner::game_relative` devolve).
pub fn label_for(rel: &str) -> Label {
    let seg: Vec<&str> = rel.split('/').collect();
    let file = seg.last().copied().unwrap_or("");
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    match seg.first().copied().unwrap_or("") {
        "sound" => sound_label(&seg, stem),
        "scripts" if stem.starts_with("game_sounds") || stem.starts_with("mvm_level_sounds") => l("Scripts de som (game_sounds)", None),
        "scripts" => l("Scripts do jogo", None),
        "materials" if rel.starts_with("materials/models/player/") && !rel.contains("/items/") => with_class("Texturas do", class_in(rel), "Texturas de personagens"),
        "materials" if rel.contains("/player/items/") => {
            let c = class_in(rel);
            Label { text: "Texturas de cosméticos".into(), class: c }
        }
        "materials" if rel.starts_with("materials/models/weapons/") || rel.contains("/weapons/") => Label { text: "Texturas de armas".into(), class: class_in(rel) },
        "materials" if rel.starts_with("materials/vgui") => l("Interface (imagens)", None),
        "materials" if rel.starts_with("materials/effects") || rel.starts_with("materials/particle") || rel.starts_with("materials/sprites") => l("Efeitos", None),
        "materials" if rel.starts_with("materials/skybox") => l("Skybox", None),
        "materials" => l("Texturas", None),
        "models" if rel.starts_with("models/player/items") || rel.starts_with("models/workshop/player/items") => Label { text: "Cosméticos".into(), class: class_in(rel) },
        "models" if rel.starts_with("models/weapons") || rel.starts_with("models/workshop/weapons") => Label { text: "Modelos de armas".into(), class: class_in(rel) },
        "models" if rel.starts_with("models/player/") => with_class("Modelo do", class_in(rel), "Modelos de personagens"),
        "models" => l("Modelos", None),
        "particles" => l("Partículas", None),
        "resource" | "ui" | "vgui" => l("HUD e interface", None),
        "cfg" => l("Configurações (cfg)", None),
        "maps" => l("Mapas", None),
        _ => l("Outros", None),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub label: String,
    /// Chave do ícone de classe, se o rótulo é de uma classe.
    pub class: Option<String>,
    pub kind: ContentKind,
    pub total: usize,
    /// Quantos arquivos do grupo existem no TF2 original (substituem algo). `None` se o índice do jogo não está disponível.
    pub replaces: Option<usize>,
    /// Quantos são novos (não existem no jogo).
    pub added: Option<usize>,
    pub examples: Vec<String>,
}

/// Agrupa caminhos (relativos ao jogo) por rótulo. Maiores grupos primeiro.
pub fn group(paths: &[String], vanilla: Option<&VanillaIndex>) -> Vec<Group> {
    let index = vanilla.filter(|v| !v.is_empty());
    let mut map: HashMap<String, Group> = HashMap::new();
    for rel in paths {
        let lab = label_for(rel);
        let g = map.entry(lab.text.clone()).or_insert_with(|| Group {
            label: lab.text.clone(),
            class: lab.class.map(str::to_string),
            kind: ContentKind::of(rel),
            total: 0,
            replaces: index.map(|_| 0),
            added: index.map(|_| 0),
            examples: Vec::new(),
        });
        g.total += 1;
        if let Some(idx) = index {
            if idx.contains(rel) {
                *g.replaces.as_mut().unwrap() += 1;
            } else {
                *g.added.as_mut().unwrap() += 1;
            }
        }
        if g.examples.len() < 3 {
            g.examples.push(rel.rsplit('/').next().unwrap_or(rel).to_string());
        }
    }
    let mut out: Vec<Group> = map.into_values().collect();
    out.sort_by(|a, b| b.total.cmp(&a.total).then(a.label.cmp(&b.label)));
    out
}

/// Classes afetadas, sem repetir, das mais mexidas para as menos.
pub fn classes_affected(groups: &[Group]) -> Vec<String> {
    let mut count: Vec<(String, usize)> = Vec::new();
    for g in groups {
        if let Some(c) = &g.class {
            match count.iter_mut().find(|(k, _)| k == c) {
                Some((_, n)) => *n += g.total,
                None => count.push((c.clone(), g.total)),
            }
        }
    }
    count.sort_by(|a, b| b.1.cmp(&a.1));
    count.into_iter().map(|(k, _)| k).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(rel: &str, text: &str, class: Option<&str>) {
        let got = label_for(rel);
        assert_eq!(got.text, text, "{rel}");
        assert_eq!(got.class, class.map(|c| CLASSES.iter().find(|(k, _)| *k == c).unwrap().0), "{rel} (classe)");
    }

    #[test]
    fn sounds_are_named_by_what_they_replace() {
        check("sound/ui/hitsound.wav", "Hitsound", None);
        check("sound/ui/hitsound_electro1.wav", "Hitsound", None);
        check("sound/ui/killsound.wav", "Killsound", None);
        check("sound/ui/gamestartup1.mp3", "Música do menu", None);
        check("sound/ui/achievement_earned.wav", "Sons de interface", None);
        check("sound/vo/demoman_battlecry01.mp3", "Voz do Demoman", Some("demoman"));
        check("sound/vo/demo/x.mp3", "Voz do Demoman", Some("demoman"));
        check("sound/vo/taunts/spy/spy_taunt01.mp3", "Provocações (voz) do Spy", Some("spy"));
        check("sound/vo/announcer_am_roundstart01.mp3", "Locutor (announcer)", None);
        check("sound/vo/mvm_wave_start01.mp3", "Vozes do Mann vs. Machine", None);
        check("sound/vo/algo_estranho.mp3", "Vozes (outras)", None);
        check("sound/weapons/demo_charge_windup1.wav", "Armas do Demoman", Some("demoman"));
        check("sound/weapons/rocket_shoot.wav", "Armas do Soldier", Some("soldier"));
        check("sound/weapons/scatter_gun_shoot.wav", "Armas do Scout", Some("scout"));
        check("sound/weapons/shotgun_shoot.wav", "Armas compartilhadas (pistola, shotgun, SMG)", None);
        check("sound/weapons/algum_som.wav", "Sons de armas", None);
        check("sound/player/footsteps/concrete1.wav", "Passos", None);
        check("sound/player/crit_hit.wav", "Críticos", None);
        check("sound/player/pl_pain05.wav", "Dano e morte", None);
        check("sound/misc/freeze_cam.wav", "Freeze cam", None);
        check("sound/misc/killstreak.wav", "Killstreak", None);
        check("sound/music/stingers/x.mp3", "Música", None);
        check("sound/items/taunts/conga.mp3", "Provocações (música)", None);
        check("sound/mvm/mvm_bomb.wav", "Mann vs. Machine", None);
    }

    #[test]
    fn other_content_is_named_by_kind_and_class() {
        check("materials/models/player/demo/demoman_red.vtf", "Texturas do Demoman", Some("demoman"));
        check("materials/models/player/items/demo/hat.vtf", "Texturas de cosméticos", Some("demoman"));
        check("materials/models/weapons/c_items/c_scattergun.vtf", "Texturas de armas", None);
        check("materials/vgui/logos/spray.vtf", "Interface (imagens)", None);
        check("materials/effects/fire.vtf", "Efeitos", None);
        check("materials/skybox/sky_day01.vtf", "Skybox", None);
        check("models/player/demo.mdl", "Modelo do Demoman", Some("demoman"));
        check("models/player/items/sniper/hat.mdl", "Cosméticos", Some("sniper"));
        check("models/weapons/c_models/c_demo_sultan_sword/x.mdl", "Modelos de armas", Some("demoman"));
        check("models/props_gameplay/ammo.mdl", "Modelos", None);
        check("particles/flame.pcf", "Partículas", None);
        check("scripts/game_sounds_weapons.txt", "Scripts de som (game_sounds)", None);
        check("scripts/items/items_game.txt", "Scripts do jogo", None);
        check("resource/ui/hudplayerhealth.res", "HUD e interface", None);
        check("cfg/hit.cfg", "Configurações (cfg)", None);
        check("maps/cp_foo.bsp", "Mapas", None);
    }

    #[test]
    fn groups_count_replacements_against_the_game_index() {
        let vanilla = VanillaIndex::from_paths(["sound/ui/hitsound.wav", "sound/vo/demoman_battlecry01.mp3", "sound/vo/demoman_cheers01.mp3"]);
        let paths: Vec<String> = ["sound/ui/hitsound.wav", "sound/vo/demoman_battlecry01.mp3", "sound/vo/demoman_cheers01.mp3", "sound/vo/demoman_novo.mp3"].map(String::from).to_vec();
        let g = group(&paths, Some(&vanilla));
        assert_eq!(g[0].label, "Voz do Demoman");
        assert_eq!((g[0].total, g[0].replaces, g[0].added), (3, Some(2), Some(1)));
        assert_eq!(g[0].class.as_deref(), Some("demoman"));
        assert_eq!(g[1].label, "Hitsound");
        assert_eq!((g[1].replaces, g[1].added), (Some(1), Some(0)));
        assert_eq!(classes_affected(&g), vec!["demoman".to_string()]);

        // sem índice do jogo: não inventa números
        let g = group(&paths, None);
        assert!(g.iter().all(|x| x.replaces.is_none() && x.added.is_none()));
        let g = group(&paths, Some(&VanillaIndex::default()));
        assert!(g.iter().all(|x| x.replaces.is_none()), "índice vazio conta como ausente");
    }

    #[test]
    fn affected_classes_are_unique_and_ordered_by_weight() {
        let paths: Vec<String> = [
            "sound/vo/spy_a.mp3",
            "sound/vo/demoman_a.mp3",
            "sound/vo/demoman_b.mp3",
            "sound/weapons/demo_charge1.wav",
            "sound/ui/hitsound.wav",
        ]
        .map(String::from)
        .to_vec();
        let g = group(&paths, None);
        assert_eq!(classes_affected(&g), vec!["demoman".to_string(), "spy".to_string()]);
        assert!(g.iter().any(|x| x.label == "Hitsound" && x.class.is_none()), "hitsound não é de classe nenhuma");
    }
}
