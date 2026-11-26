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

/// A linha de comando do jogo manda rodar o config do preloader (`+exec w/config.cfg`)?
fn cmd_runs_preload(cmd: &[String]) -> bool {
    let line = cmd.join(" ").to_ascii_lowercase().replace('\\', "/").replace('"', "");
    line.split_whitespace().collect::<Vec<_>>().windows(2).any(|w| w[0] == "+exec" && matches!(w[1], "w/config.cfg" | "w/config"))
}

/// O TF2 aberto agora: (PID, linha de comando).
fn tf2_processes(game_root: Option<&Path>) -> Vec<(u32, Vec<String>)> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet).with_cmd(UpdateKind::OnlyIfNotSet),
    );
    sys.processes()
        .iter()
        .filter_map(|(pid, p)| {
            let cmd: Vec<String> = p.cmd().iter().map(|c| c.to_string_lossy().into_owned()).collect();
            is_tf2_process(&p.name().to_string_lossy(), p.exe(), &cmd, game_root).then(|| (pid.as_u32(), cmd))
        })
        .collect()
}

fn tf2_pids(game_root: Option<&Path>) -> Vec<u32> {
    tf2_processes(game_root).into_iter().map(|(pid, _)| pid).collect()
}

/// O jogo está aberto? E, se está, recebeu o `+exec w/config.cfg`? `None` = fechado, ou o sistema não deixou ver a
/// linha de comando (na dúvida o app não acusa nada). Só a linha de comando do próprio TF2 é olhada.
pub fn game_and_preload(game_root: Option<&Path>) -> (bool, Option<bool>) {
    let found = tf2_processes(game_root);
    let seen: Vec<&Vec<String>> = found.iter().map(|(_, c)| c).filter(|c| !c.is_empty()).collect();
    (!found.is_empty(), (!seen.is_empty()).then(|| seen.iter().any(|c| cmd_runs_preload(c))))
}

fn tf2_is_running(game_root: Option<&Path>) -> bool {
    !tf2_pids(game_root).is_empty()
}

/// Pede ao TF2 que feche, do jeito educado (como clicar no X da janela): nunca força o encerramento. Devolve quantos
/// processos receberam o pedido. O jogo pode demorar para fechar; quem chama confere depois com [`game_running`].
pub fn request_game_close(game_root: Option<&Path>) -> usize {
    let pids = tf2_pids(game_root);
    for pid in &pids {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // `taskkill` sem `/F` manda o pedido de fechar para a janela. 0x08000000 = sem janela de console piscando.
            let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string()]).creation_flags(0x0800_0000).output();
        }
        #[cfg(not(windows))]
        {
            let _ = std::process::Command::new("kill").arg(pid.to_string()).output(); // SIGTERM: também é o pedido educado
        }
    }
    pids.len()
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
    #[test]
    fn the_command_line_says_whether_the_preload_config_was_asked_for() {
        let cmd = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        for yes in [&["tf_win64.exe", "-steam", "-game", "tf", "+exec", "w/config.cfg"][..], &["hl2.exe", "+EXEC", "W\\Config.cfg"][..], &["tf_win64.exe -novid +exec w/config.cfg -condebug"][..], &["tf.exe", "+exec", "\"w/config\""][..]] {
            assert!(super::cmd_runs_preload(&cmd(yes)), "{yes:?}");
        }
        for no in [&["tf_win64.exe", "-steam", "-game", "tf"][..], &["tf_win64.exe", "+exec", "autoexec.cfg"][..], &["tf_win64.exe", "w/config.cfg"][..], &[][..]] {
            assert!(!super::cmd_runs_preload(&cmd(no)), "{no:?}");
        }
    }

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
