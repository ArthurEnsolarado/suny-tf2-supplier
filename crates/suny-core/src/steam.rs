//! Achar o Team Fortress 2 instalado.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{Error, Result, TF2_APP_ID};

/// De onde veio a instalação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Achada pelo Steam (biblioteca + appmanifest).
    Steam,
    /// Pasta escolhida pelo usuário.
    Manual,
    /// Achada olhando as pastas comuns (quando o Steam não conta onde o jogo está).
    Scan,
}

#[derive(Debug, Clone, Serialize)]
pub struct Installation {
    /// Pasta raiz do jogo (`.../common/Team Fortress 2`).
    pub root: PathBuf,
    /// Pasta `tf/` dentro da raiz.
    pub tf_dir: PathBuf,
    /// Build do jogo, quando o Steam informa. Muda a cada atualização.
    pub build_id: Option<u64>,
    pub source: Source,
}

impl Installation {
    /// Valida uma pasta escolhida à mão. Aceita a raiz do jogo, a própria `tf/`, um arquivo do jogo
    /// (`hl2.exe`, `gameinfo.txt`...), pastas dentro de `tf/` e até a pasta `common`/biblioteca do Steam.
    pub fn from_path(path: &Path) -> Result<Self> {
        let cleaned = PathBuf::from(path.to_string_lossy().trim().trim_matches('"').trim_end_matches(['\\', '/']).to_string());
        for cand in candidate_roots(&cleaned) {
            if let Some(mut inst) = Self::exact(&cand) {
                inst.build_id = read_build_id(&inst.root);
                return Ok(inst);
            }
        }
        Err(Error::NotTf2Dir(path.to_path_buf()))
    }

    fn exact(path: &Path) -> Option<Self> {
        if path.join("tf").join("gameinfo.txt").is_file() {
            Some(Self { root: path.to_path_buf(), tf_dir: path.join("tf"), build_id: None, source: Source::Manual })
        } else if path.join("gameinfo.txt").is_file() && path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("tf")) {
            Some(Self {
                root: path.parent().map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf()),
                tf_dir: path.to_path_buf(),
                build_id: None,
                source: Source::Manual,
            })
        } else {
            None
        }
    }

    pub fn custom_dir(&self) -> PathBuf {
        self.tf_dir.join("custom")
    }

    pub fn gameinfo_path(&self) -> PathBuf {
        self.tf_dir.join("gameinfo.txt")
    }

    pub fn console_log_path(&self) -> PathBuf {
        self.tf_dir.join("console.log")
    }
}

/// Lugares onde o usuário pode ter apontado: a própria pasta, o pai (se for um arquivo), pastas acima (se for
/// `tf`, `custom`, `bin`...) e as pastas do Steam que contêm `Team Fortress 2`.
fn candidate_roots(path: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = vec![path.to_path_buf()];
    for anc in path.ancestors().skip(1).take(4) {
        v.push(anc.to_path_buf());
    }
    for rel in ["Team Fortress 2", "common/Team Fortress 2", "steamapps/common/Team Fortress 2"] {
        v.push(path.join(rel));
    }
    v
}

/// Lê `"buildid"` do `appmanifest_440.acf` que fica em `<biblioteca>/steamapps/`.
pub fn read_build_id(root: &Path) -> Option<u64> {
    let steamapps = root.parent()?.parent()?;
    let text = std::fs::read_to_string(steamapps.join(format!("appmanifest_{TF2_APP_ID}.acf"))).ok()?;
    text.lines().find_map(|l| {
        let mut q = l.split('"').filter(|s| !s.trim().is_empty());
        if q.next()?.eq_ignore_ascii_case("buildid") {
            q.next()?.trim().parse().ok()
        } else {
            None
        }
    })
}

/// Pasta de instalação do próprio Steam, se achada.
pub fn steam_dir() -> Option<PathBuf> {
    steamlocate::SteamDir::locate().ok().map(|d| d.path().to_path_buf())
}

/// Procura o TF2 pelo Steam. Devolve lista vazia se o jogo não está instalado
/// e erro só se o próprio Steam não foi encontrado.
pub fn find_installations() -> Result<Vec<Installation>> {
    let steam = steamlocate::SteamDir::locate().map_err(|e| Error::SteamNotFound(e.to_string()))?;

    let mut found = Vec::new();
    if let Ok(Some((app, library))) = steam.find_app(TF2_APP_ID) {
        let root = library.resolve_app_dir(&app);
        let tf_dir = root.join("tf");
        if tf_dir.join("gameinfo.txt").is_file() {
            found.push(Installation {
                root,
                tf_dir,
                build_id: app.build_id,
                source: Source::Steam,
            });
        }
    }
    Ok(found)
}

/// Resultado completo da procura, com o que foi tentado (para a tela explicar quando não acha nada).
#[derive(Debug, Clone, Serialize)]
pub struct Detection {
    pub installations: Vec<Installation>,
    pub steam_error: Option<String>,
    pub steam_dir: Option<PathBuf>,
    /// Pastas olhadas além do Steam, e se o jogo estava lá.
    pub tried: Vec<TriedPath>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TriedPath {
    pub path: PathBuf,
    pub found: bool,
}

/// Pastas comuns onde o TF2 costuma estar quando o Steam não o informa.
fn common_locations() -> Vec<PathBuf> {
    let mut libs: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles", "ProgramW6432"] {
        if let Some(p) = std::env::var_os(var) {
            libs.push(PathBuf::from(p).join("Steam"));
        }
    }
    if cfg!(windows) {
        for d in b'C'..=b'Z' {
            let drive = format!("{}:\\", d as char);
            if !Path::new(&drive).exists() {
                continue;
            }
            for rel in ["SteamLibrary", "Steam", "Games\\Steam", "Games\\SteamLibrary", "Program Files (x86)\\Steam", "Program Files\\Steam"] {
                libs.push(Path::new(&drive).join(rel));
            }
        }
    }
    let mut seen = Vec::<String>::new();
    libs.into_iter()
        .map(|l| l.join("steamapps").join("common").join("Team Fortress 2"))
        .filter(|p| {
            let k = p.to_string_lossy().to_lowercase();
            if seen.contains(&k) {
                false
            } else {
                seen.push(k);
                true
            }
        })
        .collect()
}

/// Steam + pastas comuns, sem repetir a mesma instalação.
pub fn detect_all() -> Detection {
    let (mut installations, steam_error) = match find_installations() {
        Ok(v) => (v, None),
        Err(e) => (Vec::new(), Some(e.to_string())),
    };
    let mut tried = Vec::new();
    for cand in common_locations() {
        let found = cand.join("tf").join("gameinfo.txt").is_file();
        tried.push(TriedPath { path: cand.clone(), found });
        let key = cand.to_string_lossy().to_lowercase();
        if found && !installations.iter().any(|i| i.root.to_string_lossy().to_lowercase() == key) {
            if let Ok(mut inst) = Installation::from_path(&cand) {
                inst.source = Source::Scan;
                installations.push(inst);
            }
        }
    }
    Detection { installations, steam_error, steam_dir: steam_dir(), tried }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fake_tf2(base: &Path) {
        fs::create_dir_all(base.join("tf")).unwrap();
        fs::write(base.join("tf").join("gameinfo.txt"), "\"GameInfo\" {}").unwrap();
    }

    #[test]
    fn accepts_game_root_and_tf_dir() {
        let tmp = std::env::temp_dir().join(format!("suny_steam_{}", std::process::id()));
        let root = tmp.join("Team Fortress 2");
        fake_tf2(&root);

        let a = Installation::from_path(&root).unwrap();
        assert_eq!(a.tf_dir, root.join("tf"));
        let b = Installation::from_path(&root.join("tf")).unwrap();
        assert_eq!(b.root, root);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rejects_other_folders() {
        let tmp = std::env::temp_dir().join(format!("suny_steam_bad_{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        assert!(matches!(Installation::from_path(&tmp), Err(Error::NotTf2Dir(_))));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn forgiving_paths_all_land_on_the_game() {
        let tmp = std::env::temp_dir().join(format!("suny_steam_forgive_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let common = tmp.join("steamapps").join("common");
        let root = common.join("Team Fortress 2");
        fake_tf2(&root);
        fs::write(root.join("hl2.exe"), b"x").unwrap();
        fs::create_dir_all(root.join("tf").join("custom")).unwrap();
        fs::write(tmp.join("steamapps").join("appmanifest_440.acf"), "\"AppState\"\n{\n\t\"appid\"\t\t\"440\"\n\t\"buildid\"\t\t\"26334388\"\n}").unwrap();

        for p in [
            root.clone(),
            root.join("tf"),
            root.join("tf").join("custom"),
            root.join("hl2.exe"),
            root.join("tf").join("gameinfo.txt"),
            common.clone(),
            tmp.clone(),
            PathBuf::from(format!("\"{}\\\"", root.display())),
        ] {
            let i = Installation::from_path(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert_eq!(i.root, root, "{}", p.display());
            assert_eq!(i.build_id, Some(26334388), "o build vem do appmanifest ({})", p.display());
        }
        assert!(Installation::from_path(&tmp.join("nada")).is_err());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn common_locations_are_unique_and_end_in_the_game_folder() {
        let v = common_locations();
        let mut keys: Vec<String> = v.iter().map(|p| p.to_string_lossy().to_lowercase()).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n, "sem repetidos");
        assert!(v.iter().all(|p| p.ends_with("Team Fortress 2")));
    }
}
