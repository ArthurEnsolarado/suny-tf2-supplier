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
    /// Valida uma pasta escolhida à mão. Aceita a raiz do jogo ou a própria `tf/`.
    pub fn from_path(path: &Path) -> Result<Self> {
        let (root, tf_dir) = if path.join("tf").join("gameinfo.txt").is_file() {
            (path.to_path_buf(), path.join("tf"))
        } else if path.join("gameinfo.txt").is_file()
            && path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("tf"))
        {
            (
                path.parent().map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf()),
                path.to_path_buf(),
            )
        } else {
            return Err(Error::NotTf2Dir(path.to_path_buf()));
        };
        Ok(Self { root, tf_dir, build_id: None, source: Source::Manual })
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
}
