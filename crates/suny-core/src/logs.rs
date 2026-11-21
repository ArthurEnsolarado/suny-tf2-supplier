//! Lê o `console.log` do TF2 (ligado com `-condebug`) e aponta mods suspeitos.
//!
//! O TF2 não escreve relatório de crash amigável, e não temos uma lista oficial
//! das mensagens de erro. Por isso a análise **não depende do texto exato**:
//! ela acha caminhos de arquivo do jogo no log e descobre de qual mod cada um veio.
//! Cada suspeita diz quanta confiança tem e por quê.

use std::collections::HashMap;

use serde::Serialize;

use crate::scanner::game_relative;

/// Quem é o dono de cada arquivo instalado (caminho relativo ao jogo, minúsculo → nome do mod).
#[derive(Debug, Default, Clone)]
pub struct Ownership(HashMap<String, String>);

impl Ownership {
    pub fn add_mod<I: IntoIterator<Item = S>, S: AsRef<str>>(&mut self, mod_name: &str, files: I) {
        for f in files {
            self.0.insert(game_relative(f.as_ref()), mod_name.to_string());
        }
    }
    fn owner(&self, rel: &str) -> Option<&str> {
        self.0.get(rel).map(String::as_str)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize)]
pub struct Suspect {
    pub mod_name: String,
    pub confidence: Confidence,
    pub reason: String,
    /// Linha do log que sustenta a suspeita (com número da linha, a partir de 1).
    pub line_no: usize,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    /// Mais confiáveis primeiro.
    pub suspects: Vec<Suspect>,
    /// Linhas de erro que citam arquivos sem dono conhecido (arquivo do jogo ou de mod não gerenciado).
    pub unowned_errors: Vec<(usize, String)>,
    pub error_lines: usize,
    pub total_lines: usize,
}

const ERROR_WORDS: &[&str] = &[
    "error", "couldn't", "could not", "can't", "cannot", "failed", "missing", "not found", "unable", "invalid",
];
const ASSET_EXT: &[&str] = &[
    "mdl", "vtx", "vvd", "phy", "ani", "vmt", "vtf", "wav", "mp3", "ogg", "pcf", "bsp", "res", "txt", "cfg", "nav",
];
const ASSET_ROOTS: &[&str] = &[
    "models", "materials", "sound", "particles", "scripts", "resource", "cfg", "maps", "scenes", "media", "vgui",
];

/// Pega caminhos de arquivo do jogo dentro de uma linha.
pub fn asset_paths(line: &str) -> Vec<String> {
    line.split(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '(' | ')' | ',' | ';' | '<' | '>' | '[' | ']'))
        .filter_map(|tok| {
            let tok = tok.trim_matches(|c: char| matches!(c, '.' | ':' | '!' | '?'));
            let t = tok.replace('\\', "/");
            let ext = t.rsplit('.').next()?;
            if !t.contains('/') || !t.contains('.') || !ASSET_EXT.contains(&ext.to_ascii_lowercase().as_str()) {
                return None;
            }
            let rel = game_relative(&t);
            let root = rel.split('/').next().unwrap_or("");
            ASSET_ROOTS.contains(&root).then_some(rel)
        })
        .collect()
}

fn is_error_line(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    ERROR_WORDS.iter().any(|w| l.contains(w))
}

/// `crashed`: o processo do jogo terminou de forma anormal (quem chama descobre isso).
pub fn analyze(log: &str, owners: &Ownership, crashed: bool) -> Analysis {
    let lines: Vec<&str> = log.lines().collect();
    let mut best: HashMap<String, Suspect> = HashMap::new();
    let mut unowned = Vec::new();
    let mut error_lines = 0;

    let mut consider = |s: Suspect| {
        let keep = best.get(&s.mod_name).map_or(true, |old| s.confidence > old.confidence);
        if keep {
            best.insert(s.mod_name.clone(), s);
        }
    };

    for (i, line) in lines.iter().enumerate() {
        let error = is_error_line(line);
        if error {
            error_lines += 1;
        }
        if !error {
            continue;
        }
        let mut owned_any = false;
        for path in asset_paths(line) {
            if let Some(m) = owners.owner(&path) {
                owned_any = true;
                consider(Suspect {
                    mod_name: m.to_string(),
                    confidence: Confidence::High,
                    reason: format!("uma linha de erro cita o arquivo `{path}`, que é do mod"),
                    line_no: i + 1,
                    line: line.trim().to_string(),
                });
            }
        }
        if !owned_any && !asset_paths(line).is_empty() {
            unowned.push((i + 1, line.trim().to_string()));
        }
    }

    // Sem erro explícito: se o jogo caiu, os últimos arquivos citados antes do fim valem como pista fraca.
    if crashed {
        let mut recent = 0;
        for (i, line) in lines.iter().enumerate().rev() {
            for path in asset_paths(line) {
                if let Some(m) = owners.owner(&path) {
                    consider(Suspect {
                        mod_name: m.to_string(),
                        confidence: Confidence::Medium,
                        reason: format!("o jogo caiu e `{path}`, do mod, estava entre as últimas coisas citadas no log"),
                        line_no: i + 1,
                        line: line.trim().to_string(),
                    });
                }
                recent += 1;
            }
            if recent >= 20 {
                break;
            }
        }
    }

    let mut suspects: Vec<Suspect> = best.into_values().collect();
    suspects.sort_by(|a, b| b.confidence.cmp(&a.confidence).then(a.line_no.cmp(&b.line_no)));

    Analysis { suspects, unowned_errors: unowned, error_lines, total_lines: lines.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owners() -> Ownership {
        let mut o = Ownership::default();
        o.add_mod("Pack de Sons", ["pack/sound/hitsounds/ding.wav"]);
        o.add_mod("Skin Scout", ["Scout/models/player/scout.mdl", "Scout/materials/models/player/scout/scout.vtf"]);
        o
    }

    #[test]
    fn extracts_paths_from_messy_lines() {
        let p = asset_paths(r#"Error loading 'models\player\scout.mdl'. Could not open sound/hitsounds/ding.wav!"#);
        assert_eq!(p, vec!["models/player/scout.mdl", "sound/hitsounds/ding.wav"]);
        assert!(asset_paths("Connecting to 10.0.0.1:27015").is_empty());
        assert!(asset_paths("hello world.txt").is_empty()); // sem pasta do jogo
    }

    #[test]
    fn error_line_with_owned_file_is_high_confidence() {
        let log = "Loading map\nError: Model 'models/player/scout.mdl' has bad vertex data\nok\n";
        let a = analyze(log, &owners(), false);
        assert_eq!(a.suspects.len(), 1);
        assert_eq!(a.suspects[0].mod_name, "Skin Scout");
        assert_eq!(a.suspects[0].confidence, Confidence::High);
        assert_eq!(a.suspects[0].line_no, 2);
        assert_eq!(a.error_lines, 1);
    }

    #[test]
    fn crash_without_error_gives_medium_hint() {
        let log = "Precaching sound/hitsounds/ding.wav\nround start\n";
        let calm = analyze(log, &owners(), false);
        assert!(calm.suspects.is_empty());
        let crash = analyze(log, &owners(), true);
        assert_eq!(crash.suspects[0].mod_name, "Pack de Sons");
        assert_eq!(crash.suspects[0].confidence, Confidence::Medium);
    }

    #[test]
    fn high_beats_medium_for_the_same_mod_and_sorting_works() {
        let log = "sound/hitsounds/ding.wav ok\nError: Failed to load models/player/scout.mdl\n";
        let a = analyze(log, &owners(), true);
        assert_eq!(a.suspects[0].mod_name, "Skin Scout");
        assert_eq!(a.suspects[0].confidence, Confidence::High);
        assert_eq!(a.suspects[1].mod_name, "Pack de Sons");
    }

    #[test]
    fn errors_about_unmanaged_files_are_reported_separately() {
        let log = "Error: Material materials/models/props/crate.vmt not found\n";
        let a = analyze(log, &owners(), false);
        assert!(a.suspects.is_empty());
        assert_eq!(a.unowned_errors.len(), 1);
    }
}
