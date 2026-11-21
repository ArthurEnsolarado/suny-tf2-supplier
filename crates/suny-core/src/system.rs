//! Informações da máquina e do processo do jogo.

use std::path::Path;

use serde::Serialize;
use sysinfo::{Disks, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Processos que só o TF2 usa.
const TF2_ONLY: &[&str] = &["tf_win64.exe", "tf.exe", "tf_linux64"];
/// Executáveis genéricos do motor Source: também são de outros jogos, então precisam de uma segunda prova.
const SOURCE_GENERIC: &[&str] = &["hl2.exe", "hl2_linux"];

/// Esse processo é o TF2? `tf_win64.exe` sempre. `hl2.exe` só se estiver dentro da pasta do jogo ou rodar com `-game tf`.
fn is_tf2_process(name: &str, exe: Option<&Path>, cmd: &[String], root: Option<&Path>) -> bool {
    if TF2_ONLY.iter().any(|g| name.eq_ignore_ascii_case(g)) {
        return true;
    }
    if !SOURCE_GENERIC.iter().any(|g| name.eq_ignore_ascii_case(g)) {
        return false;
    }
    let in_root = match (exe, root) {
        (Some(e), Some(r)) => e.starts_with(r),
        _ => false,
    };
    let game_tf = cmd.windows(2).any(|w| w[0].eq_ignore_ascii_case("-game") && w[1].trim_matches('"').eq_ignore_ascii_case("tf"));
    in_root || game_tf
}

fn tf2_is_running(game_root: Option<&Path>) -> bool {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet).with_cmd(UpdateKind::OnlyIfNotSet),
    );
    sys.processes().values().any(|p| {
        let cmd: Vec<String> = p.cmd().iter().map(|c| c.to_string_lossy().into_owned()).collect();
        is_tf2_process(&p.name().to_string_lossy(), p.exe(), &cmd, game_root)
    })
}

/// Só o estado do jogo (barato: a interface consulta a cada poucos segundos).
pub fn game_running(game_root: Option<&Path>) -> bool {
    tf2_is_running(game_root)
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub os: String,
    pub total_ram_mb: u64,
    /// Espaço livre no disco onde o jogo está.
    pub free_disk_mb: Option<u64>,
    pub game_running: bool,
    /// Avisos prontos para mostrar.
    pub warnings: Vec<String>,
}

const MIN_FREE_DISK_MB: u64 = 2048;
const MIN_RAM_MB: u64 = 4096;

pub fn snapshot(game_root: Option<&Path>) -> Snapshot {
    let mut sys = System::new();
    sys.refresh_memory();

    let game_running = tf2_is_running(game_root);

    let total_ram_mb = sys.total_memory() / 1024 / 1024;
    let free_disk_mb = game_root.and_then(free_space_mb);

    let mut warnings = Vec::new();
    if game_running {
        warnings.push("O TF2 está aberto. Feche o jogo antes de instalar ou remover mods.".to_string());
    }
    if let Some(free) = free_disk_mb.filter(|f| *f < MIN_FREE_DISK_MB) {
        warnings.push(format!("Pouco espaço livre no disco do jogo ({free} MB)."));
    }
    if total_ram_mb > 0 && total_ram_mb < MIN_RAM_MB {
        warnings.push(format!("Pouca memória ({total_ram_mb} MB). Mods de modelo e partícula podem pesar."));
    }

    Snapshot {
        os: System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string()),
        total_ram_mb,
        free_disk_mb,
        game_running,
        warnings,
    }
}

/// Espaço livre do disco que contém `path` (o ponto de montagem mais longo que é prefixo do caminho).
fn free_space_mb(path: &Path) -> Option<u64> {
    let path = path.canonicalize().ok()?;
    // No Windows, `canonicalize` devolve `\\?\C:\...`, que nunca começa com `C:\`.
    let path = match path.to_string_lossy().strip_prefix(r"\\?\") {
        Some(rest) => std::path::PathBuf::from(rest),
        None => path,
    };
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space() / 1024 / 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_real_tf2_counts_as_the_game() {
        let root = Path::new(r"D:\SteamLibrary\steamapps\common\Team Fortress 2");
        let none: &[String] = &[];
        assert!(is_tf2_process("tf_win64.exe", None, none, None));
        assert!(is_tf2_process("TF_WIN64.EXE", None, none, Some(root)));
        // hl2.exe é de qualquer jogo Source: sozinho não prova nada
        assert!(!is_tf2_process("hl2.exe", None, none, Some(root)));
        assert!(!is_tf2_process("hl2.exe", Some(Path::new(r"C:\Games\Half-Life 2\hl2.exe")), none, Some(root)));
        assert!(is_tf2_process("hl2.exe", Some(&root.join("hl2.exe")), none, Some(root)));
        assert!(is_tf2_process("hl2.exe", None, &["hl2.exe".into(), "-game".into(), "tf".into()], None));
        assert!(!is_tf2_process("hl2.exe", None, &["-game".into(), "hl2".into()], None));
        assert!(!is_tf2_process("chrome.exe", None, none, Some(root)));
    }

    #[test]
    fn snapshot_reports_something_sensible() {
        let s = snapshot(Some(&std::env::temp_dir()));
        assert!(s.total_ram_mb > 0);
        assert!(!s.os.is_empty());
        assert!(s.free_disk_mb.is_some());
    }
}
