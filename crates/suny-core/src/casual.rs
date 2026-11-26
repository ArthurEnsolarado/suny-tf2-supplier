//! Casual Preloader dentro do app.
//!
//! O preloader (cueki, GPL-3.0) é um programa Python. O app não reescreve nada dele:
//! baixa a versão oficial **fixada** (tamanho e SHA-256 conferidos), guarda numa pasta própria
//! e fala com ele pela ponte `bridge/suny_bridge.py`, uma mensagem JSON por linha.
//!
//! Tudo que é lento (baixar, instalar dependências, aplicar no jogo) roda como **tarefa**:
//! a interface começa a tarefa e consulta o andamento com `casual_job`.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{scanner, Error};

/// A ponte, embutida no executável (assim o app funciona sem a pasta `bridge/` do código-fonte).
const BRIDGE_PY: &str = include_str!("../../../bridge/suny_bridge.py");

/// Versão do preloader que o app foi testado. Outras versões podem ter a API interna diferente.
pub struct ReleaseSpec {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

impl ReleaseSpec {
    pub fn pinned() -> Self {
        Self {
            version: "2.3.1".into(),
            url: "https://github.com/cueki/casual-pre-loader/releases/download/v2.3.1/casual-preloader-v2.3.1.zip".into(),
            sha256: "e911e6d5496b324a3d9b47e154fdebddb64c9fba27add4176d1eaae742f397d3".into(),
            size: 19_065_394,
        }
    }
}

/// Limites ao extrair o zip do preloader e ao importar mods.
const MAX_ENTRIES: usize = 30_000;
const MAX_UNPACKED: u64 = 900 << 20;
/// A ponte pode ficar quieta por um bom tempo (QuickPrecache, VPK grande): 10 min sem nenhum evento = travou.
const IDLE_TIMEOUT: Duration = Duration::from_secs(600);
const START_TIMEOUT: Duration = Duration::from_secs(90);
const LOG_LINES: usize = 300;

// ---------------------------------------------------------------- pastas

/// Onde tudo do Casual fica: `<root>/preloader` (release oficial), `<root>/meta`, `<root>/state.json`...
#[derive(Debug, Clone)]
pub struct Layout {
    pub root: PathBuf,
}

impl Default for Layout {
    fn default() -> Self {
        if let Some(p) = std::env::var_os("SUNY_CASUAL_DIR") {
            return Self { root: PathBuf::from(p) };
        }
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
            .unwrap_or_else(std::env::temp_dir);
        Self { root: base.join("suny-tf2-supplier").join("casual") }
    }
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    /// Pasta onde o zip oficial é extraído (ele traz `RUNME.bat` e `casual-preloader/`).
    pub fn release_dir(&self) -> PathBuf {
        self.root.join("preloader")
    }
    pub fn preloader_dir(&self) -> PathBuf {
        self.release_dir().join("casual-preloader")
    }
    pub fn python(&self) -> PathBuf {
        self.preloader_dir().join(if cfg!(windows) { "python.exe" } else { "python3" })
    }
    /// Os mods do casual ficam DENTRO do preloader (regra dele), não em `custom`.
    pub fn addons_dir(&self) -> PathBuf {
        self.preloader_dir().join("userdata").join("data").join("mods").join("addons")
    }
    pub fn meta_dir(&self) -> PathBuf {
        self.root.join("meta")
    }
    pub fn state_file(&self) -> PathBuf {
        self.root.join("state.json")
    }
    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }
    fn bridge_script(&self) -> PathBuf {
        self.root.join("bridge").join("suny_bridge.py")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    /// O preloader ainda não foi baixado.
    Missing,
    /// Baixado, mas faltam as bibliotecas Python (pip).
    NeedsDeps,
    Ready,
}

pub fn runtime(l: &Layout) -> Runtime {
    let dir = l.preloader_dir();
    if !l.python().is_file() || !dir.join("core").join("version.py").is_file() {
        return Runtime::Missing;
    }
    let site = dir.join("Lib").join("site-packages");
    if site.join("cappa").is_dir() && site.join("valve_parsers").is_dir() {
        Runtime::Ready
    } else {
        Runtime::NeedsDeps
    }
}

/// Lê `VERSION = "x.y.z"` de `core/version.py`.
pub fn installed_version(l: &Layout) -> Option<String> {
    let text = std::fs::read_to_string(l.preloader_dir().join("core").join("version.py")).ok()?;
    text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("VERSION")?.trim_start().strip_prefix('=')?;
        let v = rest.trim().trim_matches(|c| c == '"' || c == '\'');
        (!v.is_empty()).then(|| v.to_string())
    })
}

// ---------------------------------------------------------------- estado e mods

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct State {
    /// Mods cujo conteúdo mudou (escolha de sons) desde a última aplicação: o jogo ainda tem a versão anterior.
    #[serde(default)]
    changed: Vec<String>,
    /// Mods marcados para entrar na próxima aplicação.
    #[serde(default)]
    enabled: Vec<String>,
    /// O que foi aplicado em cada instalação do jogo (chave: pasta `tf` em minúsculas).
    #[serde(default)]
    applied: BTreeMap<String, Applied>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Applied {
    pub addons: Vec<String>,
    pub at: u64,
    pub preloader_version: String,
}

static STATE_LOCK: Mutex<()> = Mutex::new(());

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn load_state(l: &Layout) -> State {
    std::fs::read_to_string(l.state_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("não consegui criar {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data).map_err(|e| format!("não consegui gravar {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("não consegui gravar {}: {e}", path.display()))
}

fn update_state(l: &Layout, f: impl FnOnce(&mut State)) -> Result<(), String> {
    let _g = STATE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut s = load_state(l);
    f(&mut s);
    write_atomic(&l.state_file(), &serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?)
}

fn state_key(tf_dir: &Path) -> String {
    tf_dir.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// De onde veio um mod do casual (GameBanana).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddonSource {
    pub kind: String,
    pub gb_id: u64,
    pub file_id: u64,
    pub file_name: String,
    pub name: String,
    pub version: Option<String>,
    pub submitter: Option<String>,
    pub profile_url: String,
    pub md5: Option<String>,
    pub added_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Addon {
    pub name: String,
    /// `type` do `mod.json` do preloader (Hud, Sound...). "Unknown" quando ele não sabe.
    pub kind: String,
    pub file_count: u64,
    pub size_bytes: u64,
    pub enabled: bool,
    pub source: Option<AddonSource>,
    /// Tem a pasta `models/`: modelos (armas, cosméticos, animações). No casual eles passam pelo pacote do preloader, que é
    /// lido ANTES de `custom` e depende do pré-carregamento que roda quando o TF2 abre (`+exec w/config.cfg`).
    pub has_models: bool,
    /// O mesmo item do GameBanana também está instalado em `custom` por este app. A cópia do Casual vence a de `custom`.
    pub also_in_custom: bool,
    /// Quantos grupos de sons variados dá para escolher (`soundpick`).
    pub sound_groups: usize,
}

fn dir_has_models(dir: &Path) -> bool {
    std::fs::read_dir(dir.join("models")).map(|mut r| r.next().is_some()).unwrap_or(false)
}

/// Nome de pasta que o app aceita mexer: um item direto de `addons`, sem separador nem `..`.
pub(crate) fn valid_addon_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && name != ".."
        && name.trim() == name
}

fn walk_size(dir: &Path, files: &mut u64, bytes: &mut u64, depth: u32) {
    if depth > 24 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            walk_size(&e.path(), files, bytes, depth + 1);
        } else if ft.is_file() {
            *files += 1;
            *bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
}

fn read_meta(l: &Layout, name: &str) -> Option<AddonSource> {
    serde_json::from_str(&std::fs::read_to_string(l.meta_dir().join(format!("{name}.json"))).ok()?).ok()
}

pub fn list_addons(l: &Layout) -> Vec<Addon> {
    list_addons_in(l, None)
}

/// Com a pasta `custom` do jogo, marca os mods que também estão instalados lá (mesmo item do GameBanana).
pub fn list_addons_in(l: &Layout, custom: Option<&Path>) -> Vec<Addon> {
    let state = load_state(l);
    let managed: Vec<(String, u64)> = custom.map(|c| crate::installer::list_managed(c).into_iter().map(|m| (m.meta.kind, m.meta.gb_id)).collect()).unwrap_or_default();
    let Ok(rd) = std::fs::read_dir(l.addons_dir()) else { return Vec::new() };
    let mut out: Vec<Addon> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !valid_addon_name(&name) {
                return None;
            }
            let (mut files, mut bytes) = (0, 0);
            walk_size(&e.path(), &mut files, &mut bytes, 0);
            let kind = std::fs::read_to_string(e.path().join("mod.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v.get("type").and_then(|t| t.as_str().map(str::to_string)))
                .unwrap_or_else(|| "Unknown".into());
            let source = read_meta(l, &name);
            let also_in_custom = source.as_ref().is_some_and(|s| s.gb_id != 0 && managed.iter().any(|(k, id)| *k == s.kind && *id == s.gb_id));
            Some(Addon {
                enabled: state.enabled.contains(&name),
                source,
                has_models: dir_has_models(&e.path()),
                also_in_custom,
                sound_groups: crate::soundpick::families(l, &name).map_or(0, |f| f.len()),
                name,
                kind,
                file_count: files,
                size_bytes: bytes,
            })
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Um mod do casual que veio de um certo item do GameBanana, e em que pé ele está nesta instalação do jogo.
#[derive(Debug, Clone, Serialize)]
pub struct InstalledAddon {
    pub name: String,
    pub file_id: u64,
    pub enabled: bool,
    /// O jogo está com ESTA versão dele aplicada.
    pub in_game: bool,
    pub sound_groups: usize,
}

pub fn installed_state(l: &Layout, tf_dir: Option<&Path>, game_applied: bool, kind: &str, gb_id: u64) -> Vec<InstalledAddon> {
    let state = load_state(l);
    let applied: Vec<String> = tf_dir.and_then(|d| state.applied.get(&state_key(d))).map(|a| a.addons.clone()).unwrap_or_default();
    list_addons(l)
        .into_iter()
        .filter(|a| gb_id != 0 && a.source.as_ref().is_some_and(|s| s.kind == kind && s.gb_id == gb_id))
        .map(|a| InstalledAddon {
            in_game: game_applied && applied.contains(&a.name) && !state.changed.contains(&a.name),
            file_id: a.source.as_ref().map_or(0, |s| s.file_id),
            enabled: a.enabled,
            sound_groups: a.sound_groups,
            name: a.name,
        })
        .collect()
}

/// Anota que o conteúdo de um mod mudou: só vale no jogo depois de aplicar de novo.
pub fn mark_changed(l: &Layout, name: &str) -> Result<(), String> {
    update_state(l, |s| {
        if !s.changed.iter().any(|n| n == name) {
            s.changed.push(name.to_string());
        }
    })
}

pub fn changed_since_apply(l: &Layout) -> Vec<String> {
    load_state(l).changed
}

/// Os mods marcados para a próxima aplicação.
pub fn enabled_names(l: &Layout) -> Vec<String> {
    load_state(l).enabled
}

pub fn set_enabled(l: &Layout, names: Vec<String>) -> Result<(), String> {
    let existing: Vec<String> = list_addons(l).into_iter().map(|a| a.name).collect();
    let mut keep: Vec<String> = Vec::new();
    for n in names {
        if !existing.contains(&n) {
            return Err(format!("o mod '{n}' não está na lista do casual"));
        }
        if !keep.contains(&n) {
            keep.push(n);
        }
    }
    update_state(l, |s| s.enabled = keep)
}

/// Tira um mod da LISTA do casual (apaga a pasta dele no preloader). O jogo só muda quando você aplicar de novo.
pub fn remove_addon(l: &Layout, name: &str) -> Result<(), String> {
    if !valid_addon_name(name) {
        return Err(Error::BadName(name.to_string()).to_string());
    }
    let dir = l.addons_dir().join(name);
    if !dir.is_dir() || dir.parent() != Some(l.addons_dir().as_path()) {
        return Err(format!("o mod '{name}' não está na lista do casual"));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("não consegui apagar {}: {e}", dir.display()))?;
    let _ = std::fs::remove_file(l.meta_dir().join(format!("{name}.json")));
    crate::soundpick::forget(l, name);
    update_state(l, |s| {
        s.enabled.retain(|n| n != name);
        s.changed.retain(|n| n != name);
    })
}

// ---------------------------------------------------------------- tarefas

#[derive(Debug, Clone, Serialize)]
pub struct JobView {
    pub id: u64,
    pub kind: String,
    pub title: String,
    /// `running`, `done` ou `failed`.
    pub state: &'static str,
    pub pct: u8,
    pub msg: String,
    pub log: Vec<String>,
    pub error: Option<String>,
    pub result: Option<Value>,
    pub elapsed_s: u64,
    #[serde(skip)]
    started: Option<Instant>,
    #[serde(skip)]
    ended_s: Option<u64>,
}

static JOB: Mutex<Option<JobView>> = Mutex::new(None);
static JOB_SEQ: AtomicU64 = AtomicU64::new(0);

fn with_job<R>(f: impl FnOnce(&mut Option<JobView>) -> R) -> R {
    f(&mut JOB.lock().unwrap_or_else(|p| p.into_inner()))
}

/// Andamento da tarefa atual (ou da última).
pub fn job_view() -> Option<JobView> {
    with_job(|j| {
        j.as_mut().map(|v| {
            v.elapsed_s = v.ended_s.unwrap_or_else(|| v.started.map(|s| s.elapsed().as_secs()).unwrap_or(0));
            v.clone()
        })
    })
}

/// Entregue à tarefa para ela contar o que está fazendo.
#[derive(Clone)]
pub struct Reporter {
    id: u64,
}

impl Reporter {
    /// Só para testes de outros módulos (sem tarefa de verdade por trás).
    #[doc(hidden)]
    pub fn for_tests() -> Self {
        Self { id: 0 }
    }
    /// A barra nunca anda para trás: o preloader às vezes volta de 80% para 78%.
    pub fn progress(&self, pct: u8, msg: &str) {
        with_job(|j| {
            if let Some(v) = j.as_mut().filter(|v| v.id == self.id) {
                v.pct = v.pct.max(pct.min(99));
                v.msg = msg.to_string();
                push_log(&mut v.log, msg);
            }
        });
    }
    pub fn log(&self, line: &str) {
        with_job(|j| {
            if let Some(v) = j.as_mut().filter(|v| v.id == self.id) {
                push_log(&mut v.log, line);
            }
        });
    }
}

fn push_log(log: &mut Vec<String>, line: &str) {
    let line = line.trim();
    if line.is_empty() || log.last().is_some_and(|l| l == line) {
        return;
    }
    log.push(line.chars().take(400).collect());
    if log.len() > LOG_LINES {
        log.remove(0);
    }
}

/// Começa uma tarefa em segundo plano. Só uma por vez.
pub fn start_job<F, Fut>(kind: &str, title: &str, f: F) -> Result<JobView, String>
where
    F: FnOnce(Reporter) -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>> + Send + 'static,
{
    let id = with_job(|j| {
        if let Some(cur) = j.as_ref().filter(|v| v.state == "running") {
            return Err(format!("Já existe uma operação em andamento: {}. Espere terminar.", cur.title));
        }
        let id = JOB_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
        *j = Some(JobView {
            id,
            kind: kind.to_string(),
            title: title.to_string(),
            state: "running",
            pct: 0,
            msg: "Começando…".into(),
            log: Vec::new(),
            error: None,
            result: None,
            elapsed_s: 0,
            started: Some(Instant::now()),
            ended_s: None,
        });
        Ok(id)
    })?;

    let rep = Reporter { id };
    let fut = f(rep);
    tokio::spawn(async move {
        // Um pânico dentro da tarefa vira "falhou", e a tarefa nunca fica "em andamento" para sempre.
        let res = match tokio::spawn(fut).await {
            Ok(r) => r,
            Err(e) => Err(format!("a operação parou de forma inesperada: {e}")),
        };
        with_job(|j| {
            if let Some(v) = j.as_mut().filter(|v| v.id == id) {
                v.ended_s = v.started.map(|s| s.elapsed().as_secs());
                match res {
                    Ok(r) => {
                        v.state = "done";
                        v.pct = 100;
                        v.msg = "Pronto".into();
                        v.result = Some(r);
                    }
                    Err(e) => {
                        v.state = "failed";
                        push_log(&mut v.log, &e);
                        v.error = Some(e);
                    }
                }
            }
        });
    });
    Ok(job_view().expect("tarefa recém criada"))
}

// ---------------------------------------------------------------- ponte (Python)

enum Line {
    Event(Value),
    Eof,
}

pub struct Bridge {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Line>,
    stderr_tail: Arc<Mutex<Vec<String>>>,
}

#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}
#[cfg(not(windows))]
fn hide_window(_: &mut Command) {}

fn python_command(l: &Layout) -> Command {
    let mut cmd = Command::new(l.python());
    cmd.current_dir(l.preloader_dir())
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut cmd);
    cmd
}

impl Bridge {
    pub fn spawn(l: &Layout) -> Result<Self, String> {
        if runtime(l) != Runtime::Ready {
            return Err("O Casual Preloader ainda não está configurado no app.".into());
        }
        write_atomic(&l.bridge_script(), BRIDGE_PY.as_bytes())?;
        let mut cmd = python_command(l);
        cmd.arg(l.bridge_script());
        let mut child = cmd.spawn().map_err(|e| format!("não consegui iniciar o Python do preloader: {e}"))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("sem saída do Python")?;
        let stderr = child.stderr.take().ok_or("sem erro do Python")?;

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).split(b'\n') {
                let Ok(bytes) = line else { break };
                let text = String::from_utf8_lossy(&bytes);
                if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
                    if tx.send(Line::Event(v)).is_err() {
                        return;
                    }
                }
            }
            let _ = tx.send(Line::Eof);
        });
        let stderr_tail = Arc::new(Mutex::new(Vec::new()));
        let tail = Arc::clone(&stderr_tail);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).split(b'\n') {
                let Ok(bytes) = line else { break };
                let text = String::from_utf8_lossy(&bytes).trim().to_string();
                if text.is_empty() {
                    continue;
                }
                let mut t = tail.lock().unwrap_or_else(|p| p.into_inner());
                t.push(text);
                if t.len() > 60 {
                    t.remove(0);
                }
            }
        });

        let bridge = Self { child, stdin, rx, stderr_tail };
        // A primeira mensagem é "ready". Se não vier, o Python caiu ao importar algo.
        match bridge.rx.recv_timeout(START_TIMEOUT) {
            Ok(Line::Event(v)) if v["event"] == "ready" => Ok(bridge),
            Ok(Line::Event(v)) => Err(format!("a ponte respondeu algo inesperado: {v}")),
            Ok(Line::Eof) | Err(_) => Err(format!("a ponte do preloader não iniciou. {}", bridge.tail_text())),
        }
    }

    fn tail_text(&self) -> String {
        let t = self.stderr_tail.lock().unwrap_or_else(|p| p.into_inner());
        let last: Vec<&str> = t.iter().rev().take(6).map(String::as_str).collect::<Vec<_>>().into_iter().rev().collect();
        if last.is_empty() {
            String::new()
        } else {
            format!("Últimas linhas do Python: {}", last.join(" | "))
        }
    }

    /// Manda um comando e espera a resposta final. Eventos de progresso passam por `on_progress`.
    pub fn request(&mut self, cmd: Value, mut on_progress: impl FnMut(u8, &str)) -> Result<Value, String> {
        let stdin = self.stdin.as_mut().ok_or("a ponte já foi fechada")?;
        writeln!(stdin, "{cmd}").and_then(|_| stdin.flush()).map_err(|e| format!("não consegui falar com a ponte: {e}"))?;
        loop {
            match self.rx.recv_timeout(IDLE_TIMEOUT) {
                Ok(Line::Event(v)) => match v["event"].as_str() {
                    Some("progress") => on_progress(v["pct"].as_u64().unwrap_or(0).min(100) as u8, v["msg"].as_str().unwrap_or("")),
                    Some("ready") => {}
                    Some("error") => {
                        let msg = v["msg"].as_str().unwrap_or("erro sem mensagem").to_string();
                        let kind = v["kind"].as_str().unwrap_or("");
                        return Err(if kind.is_empty() { msg } else { format!("{msg} ({kind})") });
                    }
                    _ => return Ok(v),
                },
                Ok(Line::Eof) => return Err(format!("a ponte do preloader fechou no meio da operação. {}", self.tail_text())),
                Err(_) => {
                    let _ = self.child.kill();
                    return Err("o preloader ficou 10 minutos sem responder e foi encerrado.".into());
                }
            }
        }
    }

    /// Fecha a entrada (a ponte termina sozinha) e espera um pouco antes de forçar.
    pub fn close(mut self) {
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

// ---------------------------------------------------------------- configurar (baixar + pip)

fn extract_release(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("não consegui abrir o zip baixado: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("o zip do preloader está corrompido: {e}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!("o zip do preloader tem arquivos demais ({})", archive.len()));
    }
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("entrada inválida no zip: {e}"))?;
        let rel = entry.enclosed_name().ok_or_else(|| format!("caminho perigoso no zip: {}", entry.name().unwrap_or_default()))?;
        if rel.components().any(|c| c.as_os_str().to_string_lossy().contains(':')) {
            return Err(format!("caminho perigoso no zip: {}", rel.display()));
        }
        if entry.is_symlink() {
            return Err(format!("o zip tem um link simbólico ({}), recusado", rel.display()));
        }
        let out = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            continue;
        }
        total += entry.size();
        if total > MAX_UNPACKED {
            return Err("o zip do preloader descompactaria acima do limite de segurança".into());
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let mut f = std::fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        // `take` limita o que sai de verdade, não só o tamanho declarado no cabeçalho.
        let declared = entry.size();
        let copied = std::io::copy(&mut (&mut entry).take(declared + 1), &mut f).map_err(|e| format!("{}: {e}", out.display()))?;
        if copied > declared {
            return Err(format!("o arquivo {} é maior do que o zip declara", rel.display()));
        }
    }
    Ok(())
}

async fn download_release(spec: &ReleaseSpec, dest: &Path, rep: &Reporter) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("suny-tf2-supplier/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(900))
        .build()
        .map_err(|e| e.to_string())?;
    let mut res = client
        .get(&spec.url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("não consegui baixar o Casual Preloader: {}", crate::error::net_err(&e)))?;
    if let Some(len) = res.content_length() {
        if len != spec.size {
            return Err(format!("o arquivo do preloader tem tamanho diferente do esperado ({len} em vez de {} bytes). Recusado.", spec.size));
        }
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = std::fs::File::create(dest).map_err(|e| format!("não consegui criar {}: {e}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut got = 0u64;
    loop {
        let chunk = res.chunk().await.map_err(|e| format!("a conexão caiu durante o download: {}", crate::error::net_err(&e)))?;
        let Some(chunk) = chunk else { break };
        got += chunk.len() as u64;
        if got > spec.size {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err("o download passou do tamanho esperado. Recusado.".into());
        }
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|e| format!("não consegui gravar o download: {e}"))?;
        rep.progress(2 + (got * 28 / spec.size.max(1)) as u8, &format!("Baixando o Casual Preloader… {} de {} MB", got >> 20, spec.size >> 20));
    }
    drop(file);
    let actual = hex(&hasher.finalize());
    if got != spec.size || !actual.eq_ignore_ascii_case(&spec.sha256) {
        let _ = std::fs::remove_file(dest);
        return Err(format!("o download não bate com a versão oficial fixada (SHA-256 {actual}). Nada foi instalado."));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Dependências do `requirements.txt` do preloader, sem a parte gráfica (PyQt6): o app não abre a janela dele.
fn core_requirements(text: &str) -> String {
    let mut out: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.to_ascii_lowercase().starts_with("pyqt6"))
        .collect();
    out.push("");
    out.join("\n")
}

fn run_pip(l: &Layout, rep: &Reporter) -> Result<(), String> {
    let req = std::fs::read_to_string(l.preloader_dir().join("requirements.txt")).map_err(|e| format!("o preloader não tem requirements.txt: {e}"))?;
    let filtered = l.root.join("requirements.suny.txt");
    write_atomic(&filtered, core_requirements(&req).as_bytes())?;

    let mut cmd = python_command(l);
    cmd.args(["pip.pyz", "install", "--disable-pip-version-check", "--no-warn-script-location", "-r"]).arg(&filtered);
    let mut child = cmd.spawn().map_err(|e| format!("não consegui iniciar o pip: {e}"))?;
    drop(child.stdin.take());
    let stdout = child.stdout.take().ok_or("sem saída do pip")?;
    let stderr = child.stderr.take().ok_or("sem erro do pip")?;

    let errs = Arc::new(Mutex::new(Vec::<String>::new()));
    let errs2 = Arc::clone(&errs);
    let rep2 = rep.clone();
    let t = std::thread::spawn(move || {
        for line in BufReader::new(stderr).split(b'\n').map_while(Result::ok) {
            let s = String::from_utf8_lossy(&line).trim().to_string();
            if !s.is_empty() {
                rep2.log(&s);
                let mut e = errs2.lock().unwrap_or_else(|p| p.into_inner());
                e.push(s);
                if e.len() > 20 {
                    e.remove(0);
                }
            }
        }
    });
    let mut pct = 35u8;
    for line in BufReader::new(stdout).split(b'\n').map_while(Result::ok) {
        let s = String::from_utf8_lossy(&line).trim().to_string();
        if s.starts_with("Collecting") {
            pct = (pct + 4).min(88);
        }
        if !s.is_empty() {
            rep.progress(pct, &format!("Instalando bibliotecas do preloader… {s}"));
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let _ = t.join();
    if !status.success() {
        let tail = errs.lock().unwrap_or_else(|p| p.into_inner()).iter().rev().take(4).cloned().collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
        return Err(format!("o pip não conseguiu instalar as bibliotecas (código {}). Precisa de internet. {tail}", status.code().unwrap_or(-1)));
    }
    Ok(())
}

/// Baixa (se faltar), extrai, instala as bibliotecas e confere que a ponte abre.
pub async fn setup(l: Layout, spec: ReleaseSpec, rep: Reporter) -> Result<Value, String> {
    if runtime(&l) == Runtime::Missing {
        rep.progress(1, "Baixando o Casual Preloader…");
        let zip_path = l.root.join("download").join("casual-preloader.zip");
        download_release(&spec, &zip_path, &rep).await?;

        rep.progress(31, "Extraindo…");
        let (l2, zip2) = (l.clone(), zip_path.clone());
        tokio::task::spawn_blocking(move || -> Result<(), String> {
            let tmp = l2.root.join("preloader.novo");
            let _ = std::fs::remove_dir_all(&tmp);
            std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
            if let Err(e) = extract_release(&zip2, &tmp) {
                let _ = std::fs::remove_dir_all(&tmp);
                return Err(e);
            }
            // Se já havia dados (mods) de uma instalação incompleta, eles vão junto.
            let old_data = l2.preloader_dir().join("userdata");
            if old_data.is_dir() {
                let new_data = tmp.join("casual-preloader").join("userdata");
                let _ = std::fs::remove_dir_all(&new_data);
                std::fs::rename(&old_data, &new_data).map_err(|e| format!("não consegui preservar seus mods: {e}"))?;
            }
            let _ = std::fs::remove_dir_all(l2.release_dir());
            std::fs::rename(&tmp, l2.release_dir()).map_err(|e| format!("não consegui finalizar a extração: {e}"))?;
            let _ = std::fs::remove_file(&zip2);
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())??;
    }

    if runtime(&l) == Runtime::NeedsDeps {
        rep.progress(34, "Instalando bibliotecas do preloader…");
        let (l2, rep2) = (l.clone(), rep.clone());
        tokio::task::spawn_blocking(move || run_pip(&l2, &rep2)).await.map_err(|e| e.to_string())??;
    }
    if runtime(&l) != Runtime::Ready {
        return Err("O preloader foi extraído, mas faltam bibliotecas depois do pip. Veja o registro da operação.".into());
    }

    rep.progress(92, "Conferindo a ponte com o preloader…");
    let l2 = l.clone();
    let hello = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let mut b = Bridge::spawn(&l2)?;
        let r = b.request(json!({ "cmd": "hello" }), |_, _| {});
        b.close();
        r
    })
    .await
    .map_err(|e| e.to_string())??;

    let version = hello["preloader_version"].as_str().unwrap_or("?").to_string();
    Ok(json!({ "version": version, "pinned": spec.version, "tested": version == spec.version }))
}

// ---------------------------------------------------------------- adicionar mod

/// Confere o zip ANTES de entregá-lo ao preloader: sem executável, sem caminho perigoso,
/// sem partículas (o app ainda não escolhe partículas) e sem zip bomba.
pub fn precheck_zip(zip_path: &Path) -> Result<(), Error> {
    let file = std::fs::File::open(zip_path).map_err(|e| Error::io(zip_path, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| Error::Archive(e.to_string()))?;
    if archive.len() > MAX_ENTRIES {
        return Err(Error::TooLarge(format!("{} arquivos", archive.len())));
    }
    let mut paths = Vec::with_capacity(archive.len());
    let mut total = 0u64;
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).map_err(|e| Error::Archive(e.to_string()))?;
        let name = entry.name().map_err(|e| Error::Archive(format!("nome inválido no zip: {e}")))?.into_owned();
        let unsafe_path = name.starts_with('/')
            || name.starts_with('\\')
            || name.split(['/', '\\']).any(|c| c == ".." || c.contains(':'));
        if unsafe_path {
            return Err(Error::Blocked(format!("caminho perigoso no zip: {name}")));
        }
        total += entry.size();
        paths.push(name);
    }
    if total > (1 << 30) {
        return Err(Error::TooLarge(format!("{} MB descompactado", total >> 20)));
    }
    let report = scanner::analyze(&paths, None);
    if !report.blocked.is_empty() {
        return Err(Error::Blocked(format!("tem arquivo executável ou perigoso ({})", report.blocked.iter().take(3).cloned().collect::<Vec<_>>().join(", "))));
    }
    if paths.iter().any(|p| {
        let low = p.to_ascii_lowercase();
        low.ends_with(".pcf") && (low.starts_with("particles/") || low.contains("/particles/"))
    }) {
        return Err(Error::Blocked(
            "este mod tem partículas (.pcf). O app ainda não escolhe partículas para o casual: use por enquanto a janela do Casual Preloader".into(),
        ));
    }
    Ok(())
}

fn addon_fingerprints(l: &Layout) -> BTreeMap<String, (u64, u64)> {
    let mut m = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(l.addons_dir()) {
        for e in rd.flatten().filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)) {
            let (mut f, mut b) = (0, 0);
            walk_size(&e.path(), &mut f, &mut b, 0);
            m.insert(e.file_name().to_string_lossy().into_owned(), (f, b));
        }
    }
    m
}

/// Entrega o zip ao importador do próprio preloader e guarda de onde veio.
pub async fn add_zip(l: Layout, zip_path: PathBuf, addon_name: String, source: AddonSource, rep: Reporter) -> Result<Value, String> {
    if !valid_addon_name(&addon_name) {
        return Err(Error::BadName(addon_name).to_string());
    }
    rep.progress(5, "Conferindo o arquivo…");
    let zp = zip_path.clone();
    tokio::task::spawn_blocking(move || precheck_zip(&zp)).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;

    rep.progress(15, "Importando no preloader…");
    let rep2 = rep.clone();
    tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let before = addon_fingerprints(&l);
        let mut b = Bridge::spawn(&l)?;
        let res = b.request(json!({ "cmd": "import_zip", "zip": zip_path, "name": addon_name }), |p, m| rep2.progress(15 + p / 2, m));
        b.close();
        let res = res?;
        if res["ok"] != true {
            return Err(res["message"].as_str().unwrap_or("o preloader recusou o arquivo").to_string());
        }
        let after = addon_fingerprints(&l);
        let changed: Vec<String> = after.iter().filter(|(k, v)| before.get(*k) != Some(*v)).map(|(k, _)| k.clone()).collect();
        for name in &changed {
            if !valid_addon_name(name) {
                continue;
            }
            crate::soundpick::forget(&l, name);
            let meta = serde_json::to_vec_pretty(&source).map_err(|e| e.to_string())?;
            write_atomic(&l.meta_dir().join(format!("{name}.json")), &meta)?;
        }
        // Um mod novo já entra marcado para a próxima aplicação. Um pacote com VÁRIAS variantes entra desmarcado:
        // elas trocam os mesmos arquivos, então marcar todas seria uma disputa. Quem escolhe é o usuário.
        let variants = changed.len() > 1;
        // Quem traz os mesmos arquivos que um mod já marcado também entra desmarcado: o usuário escolhe qual fica.
        let mut blocked_by: Vec<String> = Vec::new();
        if !variants {
            let mut selected = enabled_names(&l);
            selected.extend(changed.iter().cloned());
            let hit = crate::conflicts::casual_block(&l, None, &selected, Some(&changed)).map_err(|e| e.to_string())?;
            if let Some(c) = hit {
                blocked_by.extend([c.a, c.b].into_iter().filter(|s| !changed.contains(&s.id)).map(|s| s.name));
            }
        }
        if !variants && blocked_by.is_empty() {
            update_state(&l, |s| {
                for n in &changed {
                    if !s.enabled.contains(n) {
                        s.enabled.push(n.clone());
                    }
                }
            })?;
        }
        let has_models = changed.iter().any(|n| dir_has_models(&l.addons_dir().join(n)));
        Ok(json!({ "added": changed, "message": res["message"], "already_there": changed.is_empty(), "has_models": has_models, "variants": variants, "blocked_by": blocked_by }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- aplicar / reverter

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ApplyOptions {
    /// QuickPrecache. Desligado por padrão e ainda não testado no app. No preloader 2.3.1 ele só pega modelos de objetos do
    /// mapa, filtrados pelo caminho (`QUICKPRECACHE_MODEL_LIST`: prop, flag, bots, medkit...); armas e animações não entram.
    #[serde(default)]
    pub quickprecache: bool,
}

fn backup_gameinfo(l: &Layout, tf_dir: &Path) -> Result<PathBuf, String> {
    let dir = l.backups_dir().join(now().to_string());
    std::fs::create_dir_all(&dir).map_err(|e| format!("não consegui criar a pasta de backup: {e}"))?;
    for name in ["gameinfo.txt"] {
        let src = tf_dir.join(name);
        if src.is_file() {
            std::fs::copy(&src, dir.join(name)).map_err(|e| format!("não consegui copiar {name} para o backup: {e}"))?;
        }
    }
    // Guarda só os 5 backups mais recentes.
    if let Ok(rd) = std::fs::read_dir(l.backups_dir()) {
        let mut all: Vec<PathBuf> = rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.path()).collect();
        all.sort();
        while all.len() > 5 {
            let _ = std::fs::remove_dir_all(all.remove(0));
        }
    }
    Ok(dir)
}

/// Aplica no jogo os mods escolhidos. O preloader restaura os arquivos originais antes, então reaplicar é seguro.
pub async fn apply(l: Layout, tf_dir: PathBuf, addons: Vec<String>, opts: ApplyOptions, rep: Reporter) -> Result<Value, String> {
    if addons.is_empty() {
        return Err("Escolha pelo menos um mod. Para deixar o jogo sem mods, use Reverter.".into());
    }
    tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let known: Vec<String> = list_addons(&l).into_iter().map(|a| a.name).collect();
        for n in &addons {
            if !known.contains(n) {
                return Err(format!("o mod '{n}' não está na lista do casual"));
            }
        }
        rep.progress(1, "Guardando uma cópia do gameinfo.txt…");
        let backup = backup_gameinfo(&l, &tf_dir)?;
        rep.progress(2, "Iniciando o preloader…");
        let mut b = Bridge::spawn(&l)?;
        let res = b.request(
            json!({
                "cmd": "install",
                "tf_path": tf_dir,
                "addons": addons,
                "options": { "skip_quickprecache": !opts.quickprecache },
            }),
            |p, m| rep.progress(p, m),
        );
        b.close();
        let done = res?;
        let version = installed_version(&l).unwrap_or_default();
        let list = addons.clone();
        update_state(&l, |s| {
            s.changed.clear();
            s.enabled = list.clone();
            s.applied.insert(state_key(&tf_dir), Applied { addons: list, at: now(), preloader_version: version.clone() });
        })?;
        Ok(json!({ "modified": done["modified"], "addons": addons, "backup": backup }))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Tira o preloader do jogo (os arquivos voltam ao original).
pub async fn revert(l: Layout, tf_dir: PathBuf, rep: Reporter) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || -> Result<Value, String> {
        rep.progress(1, "Guardando uma cópia do gameinfo.txt…");
        let backup = backup_gameinfo(&l, &tf_dir)?;
        rep.progress(2, "Iniciando o preloader…");
        let mut b = Bridge::spawn(&l)?;
        let res = b.request(json!({ "cmd": "uninstall", "tf_path": tf_dir }), |p, m| rep.progress(p, m));
        b.close();
        let done = res?;
        update_state(&l, |s| {
            s.applied.remove(&state_key(&tf_dir));
        })?;
        Ok(json!({ "modified": done["modified"], "backup": backup }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- status

/// Opções de inicialização do TF2 no Steam (só leitura), **da conta em uso**. O casual precisa de `+exec w/config.cfg`.
/// `Some("")` = a conta não tem opção nenhuma; `None` = não deu para saber (o botão Jogar acrescenta o `+exec` mesmo assim).
pub fn steam_launch_options() -> Option<String> {
    launch_options_in(&crate::steam::steam_dir()?)
}

fn launch_options_in(steam: &Path) -> Option<String> {
    // Só a conta em uso vale. Antes o app pulava quem não tinha opção para o TF2 e acabava lendo a de OUTRA conta do
    // computador: dizia "já está lá" e o Jogar não passava o `+exec`, então o pré-carregamento nunca rodava.
    let text = std::fs::read_to_string(active_localconfig(steam)?).ok()?;
    Some(vdf_launch_options(&text, "440").unwrap_or_default())
}

/// O `localconfig.vdf` da conta em uso. O Steam marca a conta em `config/loginusers.vdf`; sem esse arquivo, vale o
/// que foi gravado por último (o Steam regrava o da conta aberta).
fn active_localconfig(steam: &Path) -> Option<PathBuf> {
    let of = |user: &Path| user.join("config").join("localconfig.vdf");
    let users = steam.join("userdata");
    if let Some(id) = std::fs::read_to_string(steam.join("config").join("loginusers.vdf")).ok().and_then(|t| recent_account(&t)) {
        // sabemos qual é a conta: se o arquivo dela não existe, não vale usar o de outra
        return Some(of(&users.join(id.to_string()))).filter(|f| f.is_file());
    }
    std::fs::read_dir(users)
        .ok()?
        .flatten()
        .map(|u| of(&u.path()))
        .filter_map(|f| Some((std::fs::metadata(&f).ok()?.modified().ok()?, f)))
        .max_by_key(|(at, _)| *at)
        .map(|(_, f)| f)
}

/// Diferença entre o SteamID de 64 bits do `loginusers.vdf` e o número da pasta em `userdata`.
const STEAM_ID_BASE: u64 = 76_561_197_960_265_728;

/// O número da pasta em `userdata` da conta marcada como a mais recente (ou, sem marca, a de login mais novo).
fn recent_account(loginusers: &str) -> Option<u64> {
    let tokens = vdf_tokens(loginusers);
    let mut stack: Vec<String> = Vec::new();
    let mut best: Option<(bool, u64, u64)> = None; // (mais recente?, quando, conta)
    let mut now: (bool, u64) = (false, 0);
    let mut i = 0;
    while i < tokens.len() {
        match (&tokens[i], tokens.get(i + 1)) {
            ((true, key), Some((false, b))) if b == "{" => {
                stack.push(key.clone());
                now = (false, 0);
                i += 2;
            }
            ((false, b), _) if b == "}" => {
                if stack.len() == 2 {
                    if let Some(account) = stack[1].parse::<u64>().ok().and_then(|id| id.checked_sub(STEAM_ID_BASE)) {
                        let this = (now.0, now.1, account);
                        if best.is_none_or(|b| (this.0, this.1) > (b.0, b.1)) {
                            best = Some(this);
                        }
                    }
                }
                stack.pop();
                i += 1;
            }
            ((true, key), Some((true, value))) => {
                if key.eq_ignore_ascii_case("MostRecent") {
                    now.0 = value == "1";
                } else if key.eq_ignore_ascii_case("Timestamp") {
                    now.1 = value.parse().unwrap_or(0);
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    best.map(|(_, _, account)| account)
}

/// Chaves e valores de um VDF do Steam, em ordem: `(true, texto)`; as chaves `{` `}` viram `(false, "{" / "}")`.
fn vdf_tokens(text: &str) -> Vec<(bool, String)> {
    let mut tokens = Vec::<(bool, String)>::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(o) => s.push(o),
                            None => break,
                        },
                        '"' => break,
                        _ => s.push(c),
                    }
                }
                tokens.push((true, s));
            }
            '{' => tokens.push((false, "{".into())),
            '}' => tokens.push((false, "}".into())),
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    tokens
}

/// Procura `...\"apps\" { \"<app>\" { \"LaunchOptions\" \"valor\" } }` num VDF do Steam, sem depender de um parser completo.
fn vdf_launch_options(text: &str, app: &str) -> Option<String> {
    let tokens = vdf_tokens(text);
    let mut stack: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        match (&tokens[i], tokens.get(i + 1)) {
            ((true, key), Some((false, b))) if b == "{" => {
                stack.push(key.to_ascii_lowercase());
                i += 2;
            }
            ((false, b), _) if b == "}" => {
                stack.pop();
                i += 1;
            }
            ((true, key), Some((true, value))) => {
                if key.eq_ignore_ascii_case("LaunchOptions") && stack.len() >= 2 && stack[stack.len() - 1] == app && stack[stack.len() - 2] == "apps" {
                    return Some(value.clone());
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    None
}

pub fn launch_options_ok(opts: &str) -> bool {
    opts.to_ascii_lowercase().contains("+exec w/config.cfg")
}

/// Opções que o botão Play passa ao Steam: as que o usuário já tem + `+exec w/config.cfg` se faltar.
/// `None` = não precisa acrescentar nada (já está nas opções do Steam).
pub fn extra_launch_args(steam_opts: Option<&str>) -> Option<String> {
    let current = steam_opts.map(str::trim).unwrap_or("");
    if launch_options_ok(current) {
        return None;
    }
    Some(if current.is_empty() { "+exec w/config.cfg".to_string() } else { format!("{current} +exec w/config.cfg") })
}

pub fn status(l: &Layout, tf_dir: Option<&Path>, game_applied: Option<bool>) -> Value {
    let rt = runtime(l);
    let state = load_state(l);
    let applied_here = tf_dir.and_then(|d| state.applied.get(&state_key(d)).cloned());
    let launch = steam_launch_options();
    json!({
        "runtime": rt,
        "version": installed_version(l),
        "pinned_version": ReleaseSpec::pinned().version,
        "root": l.root,
        "addons": if rt == Runtime::Missing { Vec::new() } else { list_addons_in(l, tf_dir.map(|d| d.join("custom")).as_deref()) },
        "applied_here": applied_here,
        "changed": state.changed,
        "game_applied": game_applied,
        "launch_options": launch,
        "launch_options_ok": launch.as_deref().map(launch_options_ok),
        "job": job_view(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("suny_casual_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, data) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn requirements_lose_only_the_gui_dependency() {
        let r = core_requirements("# GUI\nPyQt6~=6.11\n\n# core\ncappa~=0.32.2\npyqt6-qt6\nvalve-parsers>=1.2.2\n");
        assert_eq!(r, "cappa~=0.32.2\nvalve-parsers>=1.2.2\n");
    }

    #[test]
    fn runtime_is_detected_in_three_steps() {
        let root = tmp("runtime");
        let l = Layout::new(&root);
        assert_eq!(runtime(&l), Runtime::Missing);
        let dir = l.preloader_dir();
        std::fs::create_dir_all(dir.join("core")).unwrap();
        std::fs::write(l.python(), b"x").unwrap();
        std::fs::write(dir.join("core").join("version.py"), "VERSION = \"2.3.1\"\n").unwrap();
        assert_eq!(runtime(&l), Runtime::NeedsDeps);
        assert_eq!(installed_version(&l).as_deref(), Some("2.3.1"));
        std::fs::create_dir_all(dir.join("Lib").join("site-packages").join("cappa")).unwrap();
        std::fs::create_dir_all(dir.join("Lib").join("site-packages").join("valve_parsers")).unwrap();
        assert_eq!(runtime(&l), Runtime::Ready);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn addon_names_cannot_escape_the_addons_folder() {
        for bad in ["", "..", ".", "a/b", "a\\b", "C:x", ".oculto", " x", "x ", "a\0b"] {
            assert!(!valid_addon_name(bad), "{bad:?}");
        }
        for ok in ["pink_charge", "Meu Mod (v2)", "hud-1.2"] {
            assert!(valid_addon_name(ok), "{ok:?}");
        }
        let root = tmp("remove");
        let l = Layout::new(&root);
        std::fs::create_dir_all(l.addons_dir().join("a")).unwrap();
        std::fs::write(root.join("importante.txt"), b"x").unwrap();
        assert!(remove_addon(&l, "..").is_err());
        assert!(remove_addon(&l, "../../importante.txt").is_err());
        assert!(remove_addon(&l, "nao_existe").is_err());
        assert!(root.join("importante.txt").exists());
        remove_addon(&l, "a").unwrap();
        assert!(!l.addons_dir().join("a").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn addons_list_with_enabled_state_and_source() {
        let root = tmp("list");
        let l = Layout::new(&root);
        std::fs::create_dir_all(l.addons_dir().join("Beta")).unwrap();
        std::fs::create_dir_all(l.addons_dir().join("alfa").join("sound")).unwrap();
        std::fs::write(l.addons_dir().join("alfa").join("sound").join("a.wav"), b"12345").unwrap();
        std::fs::write(l.addons_dir().join("alfa").join("mod.json"), r#"{"type":"Sound"}"#).unwrap();
        std::fs::write(l.addons_dir().join("solto.txt"), b"x").unwrap();
        let src = AddonSource { kind: "Sound".into(), gb_id: 1, file_id: 2, file_name: "a.zip".into(), name: "Alfa".into(), version: None, submitter: Some("eu".into()), profile_url: "u".into(), md5: None, added_at: 1 };
        write_atomic(&l.meta_dir().join("alfa.json"), &serde_json::to_vec(&src).unwrap()).unwrap();
        set_enabled(&l, vec!["alfa".into(), "alfa".into()]).unwrap();
        assert!(set_enabled(&l, vec!["fantasma".into()]).is_err());

        let list = list_addons(&l);
        assert_eq!(list.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["alfa", "Beta"], "ordem alfabética sem diferenciar caixa; arquivos soltos ficam de fora");
        assert!(list[0].enabled && !list[1].enabled);
        assert_eq!((list[0].file_count, list[0].size_bytes, list[0].kind.as_str()), (2, 5 + 16, "Sound"));
        assert_eq!(list[0].source.as_ref().unwrap().gb_id, 1);
        assert_eq!(list[1].kind, "Unknown");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn addons_know_when_they_have_models_and_when_they_are_also_installed_in_custom() {
        let root = tmp("models_flag");
        let l = Layout::new(&root);
        std::fs::create_dir_all(l.addons_dir().join("Espada").join("models").join("weapons")).unwrap();
        std::fs::write(l.addons_dir().join("Espada").join("models").join("weapons").join("a.mdl"), b"x").unwrap();
        std::fs::create_dir_all(l.addons_dir().join("Som").join("sound")).unwrap();
        std::fs::write(l.addons_dir().join("Som").join("sound").join("a.wav"), b"x").unwrap();
        let src = |name: &str, id: u64| AddonSource { kind: "Mod".into(), gb_id: id, file_id: 1, file_name: "a.zip".into(), name: name.into(), version: None, submitter: None, profile_url: String::new(), md5: None, added_at: 0 };
        write_atomic(&l.meta_dir().join("Espada.json"), &serde_json::to_vec(&src("Espada", 7)).unwrap()).unwrap();
        write_atomic(&l.meta_dir().join("Som.json"), &serde_json::to_vec(&src("Som", 8)).unwrap()).unwrap();

        // o item 7 também foi instalado em custom por este app
        let custom = root.join("custom");
        let zip = root.join("m.zip");
        make_zip(&zip, &[("sound/ui/x.wav", b"RIFF")]);
        let meta = crate::installer::ModMeta { gb_id: 7, kind: "Mod".into(), file_id: 1, file_name: "m.zip".into(), name: "Espada".into(), version: None, submitter: None, profile_url: "u".into(), license: None, md5: None };
        crate::installer::install_zip(&zip, &custom, meta).unwrap();

        let list = list_addons_in(&l, Some(&custom));
        let by = |n: &str| list.iter().find(|a| a.name == n).unwrap();
        assert!(by("Espada").has_models && !by("Som").has_models);
        assert!(by("Espada").also_in_custom && !by("Som").also_in_custom);
        assert!(list_addons(&l).iter().all(|a| !a.also_in_custom), "sem a pasta custom não há como saber");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn precheck_refuses_executables_particles_and_traversal() {
        let dir = tmp("precheck");
        let ok = dir.join("ok.zip");
        make_zip(&ok, &[("sound/ui/hitsound.wav", b"RIFF"), ("leia-me.txt", b"oi")]);
        precheck_zip(&ok).unwrap();

        let exe = dir.join("exe.zip");
        make_zip(&exe, &[("sound/a.wav", b"x"), ("setup.exe", b"MZ")]);
        assert!(matches!(precheck_zip(&exe), Err(Error::Blocked(_))));

        let pcf = dir.join("pcf.zip");
        make_zip(&pcf, &[("Skin/particles/blood.pcf", b"x")]);
        let e = precheck_zip(&pcf).unwrap_err().to_string();
        assert!(e.contains("partículas"), "{e}");

        let up = dir.join("up.zip");
        make_zip(&up, &[("../fora.wav", b"x")]);
        assert!(precheck_zip(&up).is_err());

        let not_zip = dir.join("x.zip");
        std::fs::write(&not_zip, b"isto nao e um zip").unwrap();
        assert!(matches!(precheck_zip(&not_zip), Err(Error::Archive(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn release_extraction_blocks_traversal_symlink_and_oversize() {
        let dir = tmp("extract");
        let good = dir.join("good.zip");
        make_zip(&good, &[("RUNME.bat", b"x"), ("casual-preloader/core/version.py", b"VERSION = '1'")]);
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        extract_release(&good, &out).unwrap();
        assert!(out.join("casual-preloader").join("core").join("version.py").is_file());

        let bad = dir.join("bad.zip");
        make_zip(&bad, &[("../../escapou.txt", b"x")]);
        let out2 = dir.join("out2");
        std::fs::create_dir_all(&out2).unwrap();
        assert!(extract_release(&bad, &out2).is_err());
        assert!(!dir.join("escapou.txt").exists());

        let ads = dir.join("ads.zip");
        make_zip(&ads, &[("a.txt:segredo", b"x")]);
        assert!(extract_release(&ads, &out2).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn play_adds_the_exec_only_when_it_is_missing() {
        assert_eq!(extra_launch_args(None).as_deref(), Some("+exec w/config.cfg"));
        assert_eq!(extra_launch_args(Some("   ")).as_deref(), Some("+exec w/config.cfg"));
        assert_eq!(extra_launch_args(Some("-novid -windowed")).as_deref(), Some("-novid -windowed +exec w/config.cfg"));
        assert_eq!(extra_launch_args(Some("-novid +exec w/config.cfg")), None);
        assert_eq!(extra_launch_args(Some("+EXEC W/CONFIG.CFG")), None);
    }

    /// Uma Steam de mentira com contas: (número da pasta, opções do TF2 ou nenhuma).
    fn fake_steam(tag: &str, accounts: &[(u64, Option<&str>)]) -> PathBuf {
        let root = tmp(tag);
        for (id, options) in accounts {
            let dir = root.join("userdata").join(id.to_string()).join("config");
            std::fs::create_dir_all(&dir).unwrap();
            let line = options.map(|o| format!("\"LaunchOptions\" \"{o}\"")).unwrap_or_default();
            std::fs::write(dir.join("localconfig.vdf"), format!("\"UserLocalConfigStore\" {{ \"Software\" {{ \"Valve\" {{ \"Steam\" {{ \"apps\" {{ \"440\" {{ \"LastPlayed\" \"1\" {line} }} }} }} }} }} }}")).unwrap();
            std::thread::sleep(Duration::from_millis(30)); // o último gravado fica com a data mais nova
        }
        root
    }

    fn login(root: &Path, users: &[(u64, &str, u64)]) {
        let body: String = users.iter().map(|(id, recent, at)| format!("\"{}\" {{ \"AccountName\" \"x\" \"MostRecent\" \"{recent}\" \"Timestamp\" \"{at}\" }}\n", STEAM_ID_BASE + id)).collect();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config").join("loginusers.vdf"), format!("\"users\"\n{{\n{body}}}\n")).unwrap();
    }

    #[test]
    fn only_the_account_in_use_counts_for_the_launch_option() {
        // a conta antiga tem a opção; a que está em uso não tem nenhuma
        let root = fake_steam("steam_two", &[(222, None), (111, Some("+exec w/config.cfg"))]);
        login(&root, &[(111, "0", 100), (222, "1", 200)]);
        let found = launch_options_in(&root);
        assert_eq!(found.as_deref(), Some(""), "não pode pegar a opção da outra conta");
        assert_eq!(extra_launch_args(found.as_deref()).as_deref(), Some("+exec w/config.cfg"), "o Jogar precisa acrescentar");
        // trocando a conta em uso, vale a dela, mesmo com o arquivo mais antigo ou mais novo
        login(&root, &[(111, "1", 300), (222, "0", 200)]);
        assert_eq!(launch_options_in(&root).as_deref(), Some("+exec w/config.cfg"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn without_a_mark_the_newest_login_wins_and_without_the_list_the_newest_file() {
        let root = fake_steam("steam_fallback", &[(111, Some("-antiga")), (222, None)]);
        assert_eq!(launch_options_in(&root).as_deref(), Some(""), "sem loginusers: o arquivo gravado por último, mesmo sem opção");
        login(&root, &[(111, "0", 900), (222, "0", 100)]);
        assert_eq!(launch_options_in(&root).as_deref(), Some("-antiga"), "sem marca: o login mais novo");
        // a conta em uso é conhecida mas não tem arquivo: não vale o de outra
        login(&root, &[(333, "1", 1000), (111, "0", 900)]);
        assert_eq!(launch_options_in(&root), None);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(launch_options_in(&root), None, "sem Steam não há o que ler");
        assert_eq!(recent_account("lixo { \"x\""), None);
    }

    #[test]
    fn launch_options_are_found_for_tf2_only() {
        let vdf = r#"
"UserLocalConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "apps"
                {
                    "730"
                    {
                        "LaunchOptions"		"-novid"
                    }
                    "440"
                    {
                        "LastPlayed"		"1"
                        "LaunchOptions"		"-condebug +exec w/config.cfg \"x\""
                    }
                }
            }
        }
    }
}"#;
        let o = vdf_launch_options(vdf, "440").unwrap();
        assert_eq!(o, "-condebug +exec w/config.cfg \"x\"");
        assert!(launch_options_ok(&o));
        assert_eq!(vdf_launch_options(vdf, "730").as_deref(), Some("-novid"));
        assert!(!launch_options_ok("-novid"));
        assert_eq!(vdf_launch_options(vdf, "999"), None);
        assert_eq!(vdf_launch_options("", "440"), None);
    }

    #[test]
    fn backups_keep_only_the_five_latest() {
        let root = tmp("backup");
        let l = Layout::new(&root);
        let tf = root.join("tf");
        std::fs::create_dir_all(&tf).unwrap();
        std::fs::write(tf.join("gameinfo.txt"), b"info").unwrap();
        for i in 0..8 {
            std::fs::create_dir_all(l.backups_dir().join(format!("{:010}", i))).unwrap();
        }
        let dir = backup_gameinfo(&l, &tf).unwrap();
        assert_eq!(std::fs::read(dir.join("gameinfo.txt")).unwrap(), b"info");
        let n = std::fs::read_dir(l.backups_dir()).unwrap().count();
        assert_eq!(n, 5);
        assert!(dir.exists(), "o backup recém feito nunca é apagado");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn only_one_job_runs_at_a_time_and_progress_never_goes_back() {
        let a = start_job("teste", "primeira", |rep| async move {
            rep.progress(50, "meio");
            rep.progress(40, "voltou");
            tokio::time::sleep(Duration::from_millis(300)).await;
            Ok(json!({ "ok": 1 }))
        })
        .unwrap();
        assert_eq!(a.state, "running");
        let e = start_job("teste", "segunda", |_| async { Ok(json!(null)) }).unwrap_err();
        assert!(e.contains("primeira"), "{e}");
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if job_view().is_some_and(|j| j.state != "running") {
                break;
            }
        }
        let j = job_view().unwrap();
        assert_eq!((j.state, j.pct), ("done", 100));
        assert_eq!(j.result.unwrap()["ok"], 1);
        assert!(j.log.iter().any(|l| l == "voltou"));

        let b = start_job("teste", "falha", |_| async { Err::<Value, _>("deu ruim".to_string()) }).unwrap();
        assert!(b.id > a.id);
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if job_view().is_some_and(|j| j.state != "running") {
                break;
            }
        }
        let j = job_view().unwrap();
        assert_eq!(j.state, "failed");
        assert_eq!(j.error.as_deref(), Some("deu ruim"));
    }

    /// O zip oficial baixado antes (spike): confere tamanho e SHA-256 fixados e extrai de verdade.
    /// `$env:SUNY_TEST_PRELOADER_ZIP='...\casual-preloader-v2.3.1.zip'; cargo test -p suny-core real_release -- --ignored --nocapture`
    #[test]
    #[ignore = "precisa do zip oficial em SUNY_TEST_PRELOADER_ZIP"]
    fn real_release_matches_the_pinned_hash_and_extracts() {
        let zip_path = PathBuf::from(std::env::var("SUNY_TEST_PRELOADER_ZIP").expect("defina SUNY_TEST_PRELOADER_ZIP"));
        let spec = ReleaseSpec::pinned();
        let bytes = std::fs::read(&zip_path).unwrap();
        assert_eq!(bytes.len() as u64, spec.size, "tamanho fixado");
        assert_eq!(hex(&Sha256::digest(&bytes)), spec.sha256, "SHA-256 fixado");

        let root = tmp("real_release");
        let l = Layout::new(&root);
        std::fs::create_dir_all(l.release_dir()).unwrap();
        extract_release(&zip_path, &l.release_dir()).unwrap();
        assert_eq!(runtime(&l), Runtime::NeedsDeps, "extraído, mas sem as bibliotecas Python");
        assert_eq!(installed_version(&l).as_deref(), Some(spec.version.as_str()));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// O passo do pip num runtime que já tem as bibliotecas (não precisa de internet: tudo "already satisfied").
    /// `$env:SUNY_TEST_CASUAL='<pasta do casual de teste>'; cargo test -p suny-core pip_step -- --ignored --nocapture`
    #[test]
    #[ignore = "precisa de um runtime pronto em SUNY_TEST_CASUAL"]
    fn pip_step_runs_on_a_ready_runtime() {
        let l = Layout::new(std::env::var("SUNY_TEST_CASUAL").expect("defina SUNY_TEST_CASUAL"));
        assert_eq!(runtime(&l), Runtime::Ready);
        run_pip(&l, &Reporter { id: 0 }).expect("pip");
        assert_eq!(runtime(&l), Runtime::Ready);
        let filtered = std::fs::read_to_string(l.root.join("requirements.suny.txt")).unwrap();
        assert!(!filtered.to_lowercase().contains("pyqt6"), "{filtered}");
        assert!(filtered.contains("valve-parsers"));
    }

    /// A ponte de verdade: zips embalados (`tf/sound/...`, `Nome/README + Nome/tf/...`) viram uma pasta com o NOME do app,
    /// e dois mods diferentes nunca se sobrescrevem (antes, todo zip `tf/...` virava "tf").
    /// `$env:SUNY_TEST_CASUAL='<pasta do casual de teste>'; cargo test -p suny-core import_names -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "precisa de um runtime pronto em SUNY_TEST_CASUAL (grava na lista de mods dele)"]
    async fn import_names_the_folder_after_the_mod() {
        let l = Layout::new(std::env::var("SUNY_TEST_CASUAL").expect("defina SUNY_TEST_CASUAL"));
        let dir = tmp("import_names");
        let src = |n: &str| AddonSource { kind: "Sound".into(), gb_id: 1, file_id: 1, file_name: "x.zip".into(), name: n.into(), version: None, submitter: None, profile_url: String::new(), md5: None, added_at: 0 };
        let a = dir.join("a.zip");
        make_zip(&a, &[("tf/sound/ui/aaa.wav", b"AAAA")]);
        let b = dir.join("b.zip");
        make_zip(&b, &[("Meu Mod/README.txt", b"oi"), ("Meu Mod/tf/sound/ui/bbb.wav", b"BBBB")]);
        let bad = dir.join("bad.zip");
        make_zip(&bad, &[("so-texto.txt", b"nada de mod aqui")]);

        let rep = Reporter { id: 0 };
        let r1 = add_zip(l.clone(), a, "Teste A (gbs1)".into(), src("A"), rep.clone()).await.unwrap();
        let r2 = add_zip(l.clone(), b, "Teste B (gbs2)".into(), src("B"), rep.clone()).await.unwrap();
        assert_eq!(r1["added"], json!(["Teste A (gbs1)"]));
        assert_eq!(r2["added"], json!(["Teste B (gbs2)"]));
        let names: Vec<String> = list_addons(&l).into_iter().map(|x| x.name).collect();
        assert!(names.contains(&"Teste A (gbs1)".to_string()) && names.contains(&"Teste B (gbs2)".to_string()), "{names:?}");
        assert!(!names.contains(&"tf".to_string()), "nenhum mod pode virar 'tf': {names:?}");
        let b_dir = l.addons_dir().join("Teste B (gbs2)");
        assert!(b_dir.join("sound").join("ui").join("bbb.wav").is_file(), "o conteúdo vem de dentro de tf/, sem o README");
        assert!(!b_dir.join("README.txt").exists());
        assert!(add_zip(l.clone(), bad, "Teste C (gbs3)".into(), src("C"), rep.clone()).await.is_err(), "zip sem estrutura de mod é recusado");
        assert!(add_zip(l.clone(), dir.join("a.zip"), "../fora".into(), src("D"), rep).await.is_err(), "nome perigoso é recusado");

        remove_addon(&l, "Teste A (gbs1)").unwrap();
        remove_addon(&l, "Teste B (gbs2)").unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Servidor local que entrega um corpo fixo, para testar download e conferência sem internet.
    fn serve(body: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/release.zip", server.server_addr().to_ip().unwrap());
        let h = std::thread::spawn(move || {
            if let Ok(req) = server.recv_timeout(Duration::from_secs(10)) {
                if let Some(req) = req {
                    let _ = req.respond(tiny_http::Response::from_data(body));
                }
            }
        });
        (url, h)
    }

    #[tokio::test]
    async fn download_is_checked_against_size_and_sha256() {
        let dir = tmp("download");
        let body = b"conteudo do release".to_vec();
        let sha = hex(&Sha256::digest(&body));
        let rep = Reporter { id: 0 };

        let (url, h) = serve(body.clone());
        let spec = ReleaseSpec { version: "t".into(), url, sha256: sha.clone(), size: body.len() as u64 };
        let dest = dir.join("ok.zip");
        download_release(&spec, &dest, &rep).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        h.join().unwrap();

        // SHA diferente: recusa e não deixa o arquivo
        let (url, h) = serve(body.clone());
        let spec = ReleaseSpec { version: "t".into(), url, sha256: "0".repeat(64), size: body.len() as u64 };
        let dest = dir.join("sha.zip");
        let e = download_release(&spec, &dest, &rep).await.unwrap_err();
        assert!(e.contains("não bate"), "{e}");
        assert!(!dest.exists());
        h.join().unwrap();

        // Tamanho diferente: recusa antes de gravar
        let (url, h) = serve(body.clone());
        let spec = ReleaseSpec { version: "t".into(), url, sha256: sha, size: 5 };
        let e = download_release(&spec, &dir.join("size.zip"), &rep).await.unwrap_err();
        assert!(e.contains("tamanho"), "{e}");
        h.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
