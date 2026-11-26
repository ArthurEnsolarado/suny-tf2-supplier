//! Comandos que a interface pode chamar, em um só lugar.
//!
//! O app Tauri e o servidor de desenvolvimento (`examples/devserver.rs`) usam esta mesma
//! função, então a interface se comporta igual nos dois. Nada aqui altera o jogo: só leitura.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use base64::{engine::general_purpose::STANDARD, Engine};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::gamebanana::{Client, Kind, Query, Sort};
use crate::steam::Installation;
use crate::installer::{self, ModMeta};
use crate::{analysis, archive, art, casual, conflicts, contents, gameicons, library, localmod, soundpick, steam, system, update, vanilla, Error};

#[derive(Deserialize)]
struct ListArgs {
    kind: Kind,
    sort: Sort,
    #[serde(default)]
    query: Query,
}

#[derive(Deserialize)]
struct DetailsArgs {
    kind: Kind,
    id: u64,
}

#[derive(Deserialize)]
struct CategoriesArgs {
    kind: Kind,
    /// Sem `parent`: categorias raiz. Com `parent`: as filhas dessa categoria.
    parent: Option<u32>,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct InstallArgs {
    path: String,
    kind: Kind,
    id: u64,
    file_id: u64,
}

#[derive(Deserialize)]
struct LocalInspectArgs {
    /// Arquivos ou pastas que o usuário escolheu (ou soltou na janela).
    paths: Vec<String>,
    /// Pasta do TF2, para comparar os nomes com os arquivos originais. Opcional.
    path: Option<String>,
}

#[derive(Deserialize)]
struct LocalInstallArgs {
    paths: Vec<String>,
    path: Option<String>,
    name: String,
    /// `custom` (tf/custom) ou `casual` (lista do Casual Preloader).
    dest: String,
}

#[derive(Deserialize)]
struct LocalPreviewArgs {
    id: String,
    entry: String,
}

#[derive(Deserialize)]
struct PickFilesArgs {
    /// `true` escolhe uma pasta; `false` escolhe arquivos.
    folder: bool,
}

#[derive(Deserialize)]
struct UninstallArgs {
    path: String,
    slug: String,
}

#[derive(Deserialize)]
struct GameStateArgs {
    path: Option<String>,
}

#[derive(Deserialize)]
struct FolderArgs {
    /// Só `casual` ou `cache`: a interface nunca manda um caminho.
    which: String,
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

fn open_folder(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct CasualStatusArgs {
    path: Option<String>,
}

#[derive(Deserialize)]
struct CasualAddArgs {
    /// Raiz do TF2 (opcional): para achar a pasta certa de arquivos soltos.
    path: Option<String>,
    kind: Kind,
    id: u64,
    file_id: u64,
}

#[derive(Deserialize)]
struct CasualNameArgs {
    name: String,
}

#[derive(Deserialize)]
struct SoundChooseArgs {
    name: String,
    family: String,
    choice: String,
}

#[derive(Deserialize)]
struct SoundPlayArgs {
    name: String,
    file: String,
}

#[derive(Deserialize)]
struct CasualNamesArgs {
    /// Raiz do TF2 (opcional): para comparar também com o que está em custom.
    path: Option<String>,
    names: Vec<String>,
}

#[derive(Deserialize)]
struct CasualApplyArgs {
    path: String,
    addons: Vec<String>,
    #[serde(default)]
    options: casual::ApplyOptions,
}

/// Prévia direto do catálogo: só arquivos de até 25 MiB (o resto pede o Analisar da ficha).
const PREVIEW_MAX_FILE: u64 = 25 << 20;
/// A leitura rápida (índice + uma amostra) não pode demorar mais que isto; passou, tenta o caminho completo.
const PREVIEW_QUICK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
/// Baixar o pacote inteiro só para a prévia: passou disto, avisa em vez de rodar para sempre.
const PREVIEW_FULL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Limite de download de um mod (1 GiB).
const MAX_DOWNLOAD: u64 = 1 << 30;

/// Para só ANALISAR (ver o que muda e ouvir os sons) o limite é menor: 300 MiB.
const MAX_ANALYSIS_DOWNLOAD: u64 = 300 << 20;
/// Zips baixados para análise ficam em cache até 24 h, no máximo 12 arquivos.
const CACHE_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
const CACHE_MAX_FILES: usize = 12;

#[derive(Deserialize)]
struct AnalyzeArgs {
    /// Raiz do TF2 (opcional: sem ela a análise não compara com os arquivos originais do jogo).
    path: Option<String>,
    kind: Kind,
    id: u64,
    file_id: u64,
}

#[derive(Deserialize)]
struct PreviewArgs {
    kind: Kind,
    file_id: u64,
    entry: String,
}

fn analysis_cache_dir() -> PathBuf {
    std::env::temp_dir().join("suny-tf2-supplier").join("analysis")
}

/// O nome vem só de tipo + número: nada que a interface mande vira caminho de disco.
fn cached_zip(kind: Kind, file_id: u64) -> PathBuf {
    analysis_cache_dir().join(format!("{}-{file_id}.zip", kind.as_str()))
}

/// Apaga zips antigos (mais de 24 h) e, se ainda houver mais de 12, os mais velhos. Nunca apaga `keep`.
fn prune_cache(dir: &Path, keep: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let now = SystemTime::now();
    let mut files: Vec<(PathBuf, SystemTime)> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "zip") && e.path() != keep)
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
        .collect();
    files.retain(|(p, m)| {
        if now.duration_since(*m).unwrap_or_default() > CACHE_MAX_AGE {
            let _ = std::fs::remove_file(p);
            false
        } else {
            true
        }
    });
    files.sort_by_key(|(_, m)| *m);
    while files.len() >= CACHE_MAX_FILES {
        let (p, _) = files.remove(0);
        let _ = std::fs::remove_file(p);
    }
}

/// Um pedido por vez para cada arquivo do cache: a ficha analisa sozinha ao abrir, e Analisar, a prévia ou "Adicionar
/// ao casual" podem pedir o mesmo arquivo ao mesmo tempo.
fn file_lock(path: &Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    static LOCKS: std::sync::Mutex<Option<std::collections::HashMap<PathBuf, std::sync::Arc<tokio::sync::Mutex<()>>>>> = std::sync::Mutex::new(None);
    let mut all = LOCKS.lock().unwrap_or_else(|p| p.into_inner());
    all.get_or_insert_with(Default::default).entry(path.to_path_buf()).or_default().clone()
}

/// Onde o arquivo é montado antes de ficar pronto.
fn partial_path(dest_zip: &Path) -> PathBuf {
    dest_zip.with_extension("part")
}

/// Baixa o arquivo do GameBanana e entrega sempre um `.zip` em `dest_zip`. Se ele já está lá, não faz nada.
/// `dest_zip` só aparece **inteiro**: o download (e a conversão de `.rar`/`.7z`) acontece ao lado e é renomeado no fim.
/// Antes disso, um segundo pedido via o arquivo pela metade e dava "Could not find EOCD".
async fn fetch_as_zip(client: &Client, file: &crate::gamebanana::FileInfo, dest_zip: &Path, max_bytes: u64) -> Result<(), String> {
    let lock = file_lock(dest_zip);
    let _turn = lock.lock().await;
    if dest_zip.is_file() {
        return Ok(()); // outro pedido terminou enquanto este esperava
    }
    let part = partial_path(dest_zip);
    let made: Result<(), String> = async {
        let fmt = archive::format_of(&file.file_name).ok_or_else(|| Error::UnsupportedFormat(file.file_name.clone()).to_string())?;
        if fmt == archive::Format::Zip {
            client.download_to(file, &part, max_bytes).await.map_err(|e| e.to_string())?;
        } else {
            if archive::extractors_for(fmt).is_empty() {
                return Err(Error::NoExtractor(fmt.label().to_string()).to_string());
            }
            let original = dest_zip.with_extension(fmt.label().trim_start_matches('.'));
            client.download_to(file, &original, max_bytes).await.map_err(|e| e.to_string())?;
            let (src, dest) = (original.clone(), part.clone());
            let res = tokio::task::spawn_blocking(move || archive::convert_to_zip(&src, fmt, &dest)).await.map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&original); // o arquivo original nunca fica no computador
            res.map_err(|e| e.to_string())?;
        }
        std::fs::rename(&part, dest_zip).map_err(|e| Error::io(dest_zip, e).to_string())
    }
    .await;
    if made.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    made
}

/// Um pacote do catálogo só com arquivos soltos ("instale em tf/materials/…") ganha as pastas antes de ser analisado,
/// instalado ou entregue ao casual. Pacotes que já têm a estrutura passam sem mudança.
async fn organize_download(zip: &Path, tf_dir: Option<PathBuf>, description: Option<&str>) -> Result<(), String> {
    let hints = localmod::install_hints(description.unwrap_or(""));
    let lock = file_lock(zip);
    let _turn = lock.lock().await;
    let zip = zip.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let index = tf_dir.as_deref().map(vanilla::index_for);
        localmod::organize_zip(&zip, index.as_deref(), &hints).map(|_| ()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Instala em `custom` só o que não briga com o que já está lá nem com o que está marcado no casual.
fn guarded_install(zip: &Path, custom: &Path, meta: ModMeta) -> crate::Result<installer::InstallOutcome> {
    conflicts::check_install(zip, custom, &meta, &casual::Layout::default())?;
    installer::install_zip(zip, custom, meta)
}

/// `SUNY_NO_GAME_ART=1`: roda sem ler banner, logo e ícones de classe do jogo (arte da Valve). Serve para tirar
/// capturas de tela que podem ir para o repositório.
fn no_game_art() -> bool {
    std::env::var_os("SUNY_NO_GAME_ART").is_some_and(|v| v == "1")
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Endereço que manda o Steam abrir um jogo (com as opções de inicialização que o usuário já configurou lá).
pub fn steam_run_url(app_id: u32) -> String {
    format!("steam://rungameid/{app_id}")
}

/// Igual a [`steam_run_url`], mas com opções de inicialização: `steam://run/<id>//<opções codificadas>/`.
pub fn steam_run_url_with_args(app_id: u32, args: &str) -> String {
    let enc: String = args.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    format!("steam://run/{app_id}//{enc}/")
}

/// Só deixamos abrir `steam://rungameid/<número>` ou `steam://run/<número>//<opções codificadas>/`:
/// nada que venha da interface vira comando do sistema (as opções só têm letras, números, `-_.` e `%XX`).
fn is_safe_steam_url(url: &str) -> bool {
    let digits = |id: &str| !id.is_empty() && id.len() <= 10 && id.bytes().all(|b| b.is_ascii_digit());
    if let Some(id) = url.strip_prefix("steam://rungameid/") {
        return digits(id);
    }
    let Some(rest) = url.strip_prefix("steam://run/") else { return false };
    let Some((id, opts)) = rest.split_once("//") else { return false };
    let Some(opts) = opts.strip_suffix('/') else { return false };
    digits(id) && opts.len() <= 400 && opts.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'%'))
}

/// Só endereços do repositório do projeto (nada vindo da interface vira comando do sistema).
fn is_safe_project_url(url: &str) -> bool {
    url.strip_prefix(&format!("https://github.com/{}/", update::REPO)).is_some_and(|rest| !rest.contains(['"', ' ', '&', '|', '<', '>', '^']))
        || url == format!("https://github.com/{}", update::REPO)
}

fn open_url(url: &str) -> Result<(), String> {
    if !is_safe_project_url(url) {
        return Err("endereço não permitido".into());
    }
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(url).spawn().map(|_| ()).map_err(|e| format!("não consegui abrir o navegador: {e}"))
}

fn open_steam_url(url: &str) -> Result<(), String> {
    if !is_safe_steam_url(url) {
        return Err("endereço não permitido".into());
    }
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(url).spawn().map(|_| ()).map_err(|e| format!("não consegui abrir o Steam: {e}"))
}

/// O TF2 aberto trava arquivos e desfaz mudanças: instalar e remover só com o jogo fechado.
async fn ensure_game_closed(root: PathBuf) -> Result<(), String> {
    let running = tokio::task::spawn_blocking(move || system::snapshot(Some(&root)).game_running)
        .await
        .map_err(|e| e.to_string())?;
    if running {
        Err(Error::GameRunning.to_string())
    } else {
        Ok(())
    }
}

fn args<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| format!("argumentos inválidos: {e}"))
}

fn out<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// Acha a instalação: se o Steam conhece essa pasta usa os dados dele (inclui o build), senão valida à mão.
fn resolve(path: &str) -> Result<Installation, String> {
    let wanted = PathBuf::from(path);
    if let Ok(list) = steam::find_installations() {
        if let Some(found) = list.into_iter().find(|i| i.root == wanted || i.tf_dir == wanted) {
            return Ok(found);
        }
    }
    Installation::from_path(Path::new(path)).map_err(|e| e.to_string())
}

/// Abre o TF2 pelo Steam (o jogo precisa estar fechado). Com o Casual Preloader aplicado, o jogo precisa abrir com
/// `+exec w/config.cfg` (é ele que deixa os modelos aparecerem): se o Steam não tem isso nas opções do usuário, acrescenta.
async fn launch_tf2(path: Option<String>) -> Result<Value, String> {
    let extra = tokio::task::spawn_blocking(move || {
        let found = path.and_then(|s| resolve(&s).ok())?;
        if !library::preloader_state(&found).applied() {
            return None;
        }
        casual::extra_launch_args(casual::steam_launch_options().as_deref())
    })
    .await
    .map_err(|e| e.to_string())?;
    let url = match &extra {
        Some(opts) => steam_run_url_with_args(crate::TF2_APP_ID, opts),
        None => steam_run_url(crate::TF2_APP_ID),
    };
    open_steam_url(&url)?;
    Ok(json!({ "launched": true, "added_launch_options": extra }))
}

pub async fn dispatch(client: &Client, name: &str, a: Value) -> Result<Value, String> {
    match name {
        // Steam + pastas comuns. `tried` e `steam_dir` explicam o resultado quando nada é achado.
        "detect" => tokio::task::spawn_blocking(|| out(steam::detect_all())).await.map_err(|e| e.to_string())?,

        // O caminho digitado serve? Diz o motivo quando não (para a tela mostrar na hora).
        "validate_path" => {
            let p: PathArgs = args(a)?;
            tokio::task::spawn_blocking(move || {
                if p.path.trim().is_empty() {
                    return Ok::<Value, String>(json!({ "ok": false, "empty": true }));
                }
                Ok(match Installation::from_path(Path::new(&p.path)) {
                    Ok(i) => json!({
                        "ok": true,
                        "root": i.root,
                        "build_id": i.build_id,
                        "has_custom": i.custom_dir().is_dir(),
                    }),
                    Err(e) => json!({ "ok": false, "error": e.to_string() }),
                })
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // Janela nativa para escolher a pasta do jogo. Devolve `null` se o usuário cancelar.
        "pick_folder" => tokio::task::spawn_blocking(|| {
            #[cfg(windows)]
            {
                let picked = rfd::FileDialog::new().set_title("Team Fortress 2").pick_folder();
                Ok::<Value, String>(json!({ "path": picked }))
            }
            #[cfg(not(windows))]
            {
                Err::<Value, String>("escolher pasta pela janela só existe no Windows".to_string())
            }
        })
        .await
        .map_err(|e| e.to_string())?,

        // Prévia rápida de um som do catálogo: baixa o primeiro arquivo (cache) e devolve um áudio para tocar.
        "sound_preview" => {
            let p: DetailsArgs = args(a)?;
            let details = client.details(p.kind, p.id).await.map_err(|e| e.to_string())?;
            let file = details
                .files
                .iter()
                .find(|f| !f.contains_exe() && archive::format_of(&f.file_name).is_some() && f.size <= PREVIEW_MAX_FILE)
                .ok_or_else(|| "Este item não tem um arquivo pequeno o bastante para a prévia. Abra a ficha e use Analisar.".to_string())?;
            let dest = cached_zip(p.kind, file.id);
            // 1º: tenta ler só o índice do zip e UMA amostra (poucos KB na rede). Só cai no download inteiro se der errado.
            if !dest.is_file() && archive::format_of(&file.file_name) == Some(archive::Format::Zip) {
                let quick = tokio::time::timeout(PREVIEW_QUICK_TIMEOUT, crate::remotezip::preview_audio(client, file, analysis::MAX_PREVIEW_BYTES)).await;
                match quick {
                    Ok(Ok(Some(a))) => {
                        return Ok(json!({
                            "file_id": file.id,
                            "name": a.name,
                            "total": a.total,
                            "data_url": format!("data:{};base64,{}", a.mime, STANDARD.encode(a.bytes)),
                        }));
                    }
                    Ok(Ok(None)) => return Err("Este pacote não tem áudio que dê para tocar (.wav, .mp3 ou .ogg de até 12 MB).".to_string()),
                    _ => {} // Range indisponível, índice grande demais, rede lenta...: baixa o pacote como antes
                }
            }
            if !dest.is_file() {
                let fetched = tokio::time::timeout(PREVIEW_FULL_TIMEOUT, fetch_as_zip(client, file, &dest, MAX_ANALYSIS_DOWNLOAD)).await;
                match fetched {
                    Ok(r) => r?,
                    Err(_) => {
                        let _ = std::fs::remove_file(partial_path(&dest));
                        return Err("A prévia demorou demais: este pacote é grande e a conexão está lenta. Abra a ficha e use Analisar, ou tente de novo.".to_string());
                    }
                }
            }
            prune_cache(&analysis_cache_dir(), &dest);
            let file_id = file.id;
            tokio::task::spawn_blocking(move || -> Result<Value, String> {
                let a = analysis::analyze_zip(&dest, None).map_err(|e| e.to_string())?;
                let item = a
                    .audio
                    .iter()
                    .find(|s| s.size <= analysis::MAX_PREVIEW_BYTES)
                    .ok_or_else(|| "Este pacote não tem áudio que dê para tocar (.wav, .mp3 ou .ogg de até 12 MB).".to_string())?;
                let (mime, bytes) = analysis::read_audio(&dest, &item.entry).map_err(|e| e.to_string())?;
                Ok(json!({
                    "file_id": file_id,
                    "name": item.name,
                    "total": a.audio_total,
                    "data_url": format!("data:{mime};base64,{}", STANDARD.encode(bytes)),
                }))
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "inspect" => {
            let p: PathArgs = args(a)?;
            tokio::task::spawn_blocking(move || {
                let install = resolve(&p.path)?;
                let preloader = library::preloader_state(&install);
                let custom = library::scan_custom(&install.custom_dir()).map_err(|e| e.to_string())?;
                let snapshot = system::snapshot(Some(&install.root));
                Ok(json!({
                    "installation": install,
                    "preloader": preloader,
                    "preloader_applied": preloader.applied(),
                    "preloader_leftover": preloader.leftover(),
                    "custom": custom,
                    "system": snapshot,
                }))
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // O que cada item de `custom` muda e quais itens disputam o mesmo arquivo. Só leitura; pode levar alguns segundos.
        "custom_report" => {
            let p: PathArgs = args(a)?;
            tokio::task::spawn_blocking(move || {
                let install = resolve(&p.path)?;
                let custom_dir = install.custom_dir();
                let entries = library::scan_custom(&custom_dir).map_err(|e| e.to_string())?;
                Ok(json!(contents::report(&custom_dir, &entries)))
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // Arte do jogo lida da instalação do Steam do usuário (nada é distribuído com o app).
        "art" => tokio::task::spawn_blocking(|| match steam::steam_dir() {
            _ if no_game_art() => out(art::ArtSet::default()),
            Some(dir) => out(art::find_art(&dir, crate::TF2_APP_ID)),
            None => out(art::ArtSet::default()),
        })
        .await
        .map_err(|e| e.to_string())?,

        // Baixa do GameBanana e instala em tf/custom. Com o jogo aberto só instala item NOVO: os arquivos novos entram sem
        // problema (o jogo os lê quando abrir de novo), mas reinstalar por cima mexeria em arquivos que o jogo segura.
        "install_mod" => {
            let p: InstallArgs = args(a)?;
            let install = resolve(&p.path)?;
            let game_open = {
                let root = install.root.clone();
                tokio::task::spawn_blocking(move || system::game_running(Some(&root))).await.map_err(|e| e.to_string())?
            };

            let details = client.details(p.kind, p.id).await.map_err(|e| e.to_string())?;
            let file = details
                .files
                .iter()
                .find(|f| f.id == p.file_id)
                .ok_or_else(|| "esse arquivo não existe nesse item do GameBanana".to_string())?;
            if file.contains_exe() {
                return Err(Error::Blocked("o GameBanana avisou que este arquivo contém executável".into()).to_string());
            }
            if archive::format_of(&file.file_name).is_none() {
                return Err(Error::UnsupportedFormat(file.file_name.clone()).to_string());
            }

            let staging = std::env::temp_dir().join("suny-tf2-supplier").join(format!("{}-{}.zip", p.kind.as_str(), file.id));
            fetch_as_zip(client, file, &staging, MAX_DOWNLOAD).await?;
            organize_download(&staging, Some(install.tf_dir.clone()), details.description.as_deref()).await?;

            let meta = ModMeta {
                gb_id: details.id,
                kind: p.kind.as_str().to_string(),
                file_id: file.id,
                file_name: file.file_name.clone(),
                name: details.name.clone(),
                version: details.version.clone().filter(|v| !v.is_empty()),
                submitter: details.submitter.as_ref().map(|s| s.name.clone()),
                profile_url: details.profile_url.clone(),
                license: details.license.as_deref().map(strip_tags).filter(|l| !l.is_empty()).map(|l| l.chars().take(300).collect()),
                md5: file.md5.clone(),
            };
            let custom = install.custom_dir();
            if game_open && custom.join(installer::slug_for(&meta)).exists() {
                let _ = std::fs::remove_file(&staging);
                return Err(Error::GameRunning.to_string());
            }
            let zip_path = staging.clone();
            let outcome = tokio::task::spawn_blocking(move || guarded_install(&zip_path, &custom, meta))
                .await
                .map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&staging); // o zip temporário nunca fica no computador
            out(outcome.map_err(|e| e.to_string())?)
        }

        // ---- Mod que o usuário já tem (arquivo, pasta ou .vpk): o app descobre a estrutura e arruma como um mod do catálogo.
        // `local_inspect` só lê e monta um zip temporário; nada vai para o jogo.
        "local_inspect" => {
            let p: LocalInspectArgs = args(a)?;
            let tf_dir = p.path.and_then(|s| resolve(&s).ok()).map(|i| i.tf_dir);
            let paths: Vec<PathBuf> = p.paths.iter().map(PathBuf::from).collect();
            tokio::task::spawn_blocking(move || {
                let index = tf_dir.as_deref().map(vanilla::index_for);
                let cache = analysis_cache_dir();
                let staged = localmod::stage(&paths, &cache, index.as_deref()).map_err(|e| e.to_string())?;
                prune_cache(&cache, staged.zip_path());
                let analysis = analysis::analyze_zip(staged.zip_path(), index.as_deref()).map_err(|e| e.to_string())?;
                Ok(json!({ "staged": staged, "analysis": analysis }))
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // Ouvir um som do mod local já arrumado (o zip temporário criado por `local_inspect`).
        "local_preview_audio" => {
            let p: LocalPreviewArgs = args(a)?;
            let zip = localmod::staged_zip(&analysis_cache_dir(), &p.id).ok_or_else(|| "id inválido".to_string())?;
            if !zip.is_file() {
                return Err("Escolha o mod de novo para ouvir os sons.".into());
            }
            let (mime, bytes) = tokio::task::spawn_blocking(move || analysis::read_audio(&zip, &p.entry))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(json!({ "mime": mime, "data_url": format!("data:{mime};base64,{}", STANDARD.encode(bytes)) }))
        }

        // Instala o mod local em tf/custom ou o coloca na lista do casual. Mesmas regras do catálogo: com o jogo aberto
        // só entra item NOVO; mapa nunca vai para o casual.
        "local_install" => {
            let p: LocalInstallArgs = args(a)?;
            if p.dest != "custom" && p.dest != "casual" {
                return Err("argumentos inválidos: dest".into());
            }
            let found = p.path.as_deref().and_then(|s| resolve(s).ok());
            let tf_dir = found.as_ref().map(|i| i.tf_dir.clone());
            let paths: Vec<PathBuf> = p.paths.iter().map(PathBuf::from).collect();
            let staged = tokio::task::spawn_blocking(move || {
                let index = tf_dir.as_deref().map(vanilla::index_for);
                let cache = analysis_cache_dir();
                let s = localmod::stage(&paths, &cache, index.as_deref());
                if let Ok(s) = &s {
                    prune_cache(&cache, s.zip_path());
                }
                s
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
            let meta = localmod::meta_for(&staged, &p.name);

            if p.dest == "custom" {
                let install = found.ok_or_else(|| "Escolha a pasta do TF2 primeiro (aba Início).".to_string())?;
                let game_open = {
                    let root = install.root.clone();
                    tokio::task::spawn_blocking(move || system::game_running(Some(&root))).await.map_err(|e| e.to_string())?
                };
                let custom = install.custom_dir();
                if game_open && custom.join(installer::slug_for(&meta)).exists() {
                    return Err(Error::GameRunning.to_string());
                }
                let zip_path = staged.zip.clone();
                let outcome = tokio::task::spawn_blocking(move || guarded_install(&zip_path, &custom, meta))
                    .await
                    .map_err(|e| e.to_string())?;
                return out(outcome.map_err(|e| e.to_string())?);
            }

            // casual
            let l = casual::Layout::default();
            if casual::runtime(&l) != casual::Runtime::Ready {
                return Err("Configure o Casual Preloader primeiro, na aba Casual.".into());
            }
            if let Some(cur) = casual::job_view().filter(|j| j.state == "running") {
                return Err(format!("Já existe uma operação em andamento: {}. Espere terminar.", cur.title));
            }
            let zip_for_scan = staged.zip.clone();
            let has_map = tokio::task::spawn_blocking(move || -> Result<bool, String> {
                let names = crate::scanner::zip_paths(std::fs::File::open(&zip_for_scan).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                Ok(names.iter().any(|n| n.to_ascii_lowercase().ends_with(".bsp")))
            })
            .await
            .map_err(|e| e.to_string())??;
            if has_map {
                return Err("Mapas (.bsp) não funcionam no casual: o Casual Preloader não carrega mapas. Instale em tf/custom.".into());
            }
            let source = casual::AddonSource {
                kind: meta.kind.clone(),
                gb_id: 0,
                file_id: meta.file_id,
                file_name: meta.file_name.clone(),
                name: meta.name.clone(),
                version: None,
                submitter: None,
                profile_url: String::new(),
                md5: None,
                added_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
            };
            let title = format!("Adicionar \"{}\" ao casual", meta.name);
            let addon_name = installer::slug_parts("Local", &meta.name, meta.file_id);
            let zip = staged.zip.clone();
            out(casual::start_job("add", &title, move |rep| casual::add_zip(l, zip, addon_name, source, rep))?)
        }

        // Janela nativa para escolher arquivos de mod ou uma pasta. Devolve `paths: []` se o usuário cancelar.
        "pick_files" => {
            let p: PickFilesArgs = args(a)?;
            tokio::task::spawn_blocking(move || {
                #[cfg(windows)]
                {
                    let dlg = rfd::FileDialog::new().set_title("Mod");
                    let paths: Vec<PathBuf> = if p.folder {
                        dlg.pick_folder().into_iter().collect()
                    } else {
                        dlg.add_filter("Mods (.zip .rar .7z .vpk .wav .mp3 .bsp ...)", &["zip", "rar", "7z", "vpk", "wav", "mp3", "ogg", "vtf", "vmt", "mdl", "bsp", "cfg", "pcf"])
                            .pick_files()
                            .unwrap_or_default()
                    };
                    Ok::<Value, String>(json!({ "paths": paths }))
                }
                #[cfg(not(windows))]
                {
                    let _ = p;
                    Err::<Value, String>("escolher arquivos pela janela só existe no Windows".to_string())
                }
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "uninstall_mod" => {
            let p: UninstallArgs = args(a)?;
            let install = resolve(&p.path)?;
            ensure_game_closed(install.root.clone()).await?;
            let custom = install.custom_dir();
            let manifest = tokio::task::spawn_blocking(move || installer::uninstall(&custom, &p.slug))
                .await
                .map_err(|e| e.to_string())?;
            out(manifest.map_err(|e| e.to_string())?)
        }

        // Abre o TF2 pelo Steam. Não faz nada se o jogo já estiver aberto.
        "launch_game" => {
            let running = tokio::task::spawn_blocking(|| system::snapshot(None).game_running).await.map_err(|e| e.to_string())?;
            if running {
                return Err("O TF2 já está aberto.".into());
            }
            let p: GameStateArgs = args(a)?;
            launch_tf2(p.path).await
        }

        // Fecha o TF2 (pedido educado, nunca força), espera ele sair e abre de novo. Serve para um mod instalado com o
        // jogo aberto passar a valer. Se o jogo não fechar sozinho em ~40 s, avisa e não faz mais nada.
        "restart_game" => {
            let p: GameStateArgs = args(a)?;
            let root = p.path.as_deref().and_then(|s| resolve(s).ok()).map(|i| i.root);
            let r2 = root.clone();
            let asked = tokio::task::spawn_blocking(move || system::request_game_close(r2.as_deref())).await.map_err(|e| e.to_string())?;
            if asked == 0 {
                return launch_tf2(p.path).await; // já estava fechado: só abre
            }
            let mut closed = false;
            for _ in 0..80 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let r3 = root.clone();
                if !tokio::task::spawn_blocking(move || system::game_running(r3.as_deref())).await.map_err(|e| e.to_string())? {
                    closed = true;
                    break;
                }
            }
            if !closed {
                return Err("O TF2 não fechou sozinho. Feche o jogo você mesmo (talvez ele esteja numa tela de confirmação) e abra de novo pelo botão Jogar.".into());
            }
            tokio::time::sleep(std::time::Duration::from_secs(3)).await; // dá tempo ao Steam de notar que o jogo saiu
            launch_tf2(p.path).await
        }

        // Atualizações do app: consulta as releases estáveis do repositório no GitHub (não manda nenhum dado seu).
        "update_check" => out(update::check(env!("CARGO_PKG_VERSION")).await?),

        // Baixa o instalador da versão mais nova, confere o SHA-256 e o abre. Só quando o usuário manda.
        "update_install" => {
            let info = update::check(env!("CARGO_PKG_VERSION")).await?;
            let installer = info
                .installer
                .filter(|_| info.available)
                .ok_or_else(|| "Não há atualização com instalador publicado.".to_string())?;
            out(casual::start_job("update", "Baixar a atualização", move |rep| update::download_and_launch(installer, rep))?)
        }

        // Abre a página de versões do projeto no navegador. O endereço é fixo: a interface não manda URL.
        "open_releases" => {
            open_url(&format!("https://github.com/{}/releases", update::REPO))?;
            Ok(json!({ "opened": true }))
        }

        // Só "o TF2 está aberto?": barato, a interface pergunta a cada poucos segundos.
        "game_state" => {
            let p: GameStateArgs = args(a)?;
            tokio::task::spawn_blocking(move || {
                let found = p.path.and_then(|s| resolve(&s).ok());
                let (running, preload) = system::game_and_preload(found.as_ref().map(|i| i.root.as_path()));
                // o jogo está com os mods do casual aplicados, mas abriu sem mandar carregá-los
                let no_preload = preload == Some(false) && found.as_ref().is_some_and(|i| library::preloader_state(i).applied());
                json!({ "running": running, "no_preload": no_preload })
            })
            .await
            .map_err(|e| e.to_string())
        }

        // Dados para a tela de Configurações.
        "app_info" => tokio::task::spawn_blocking(|| {
            let l = casual::Layout::default();
            let names = |f: archive::Format| archive::extractors_for(f).iter().map(|e| e.id()).collect::<Vec<_>>();
            json!({
                "version": env!("CARGO_PKG_VERSION"),
                "casual_dir": l.root,
                "cache_dir": analysis_cache_dir(),
                "cache_bytes": dir_size(&analysis_cache_dir()),
                "casual_bytes": dir_size(&l.root),
                "extractors": { "rar": names(archive::Format::Rar), "7z": names(archive::Format::SevenZ) },
            })
        })
        .await
        .map_err(|e| e.to_string()),

        // Apaga os zips baixados só para análise (cache). Não toca em mods instalados nem no Casual.
        "clear_cache" => tokio::task::spawn_blocking(|| {
            let dir = analysis_cache_dir();
            let before = dir_size(&dir);
            let mut removed = 0u32;
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "zip")) {
                    if std::fs::remove_file(e.path()).is_ok() {
                        removed += 1;
                    }
                }
            }
            json!({ "removed": removed, "freed_bytes": before.saturating_sub(dir_size(&dir)) })
        })
        .await
        .map_err(|e| e.to_string()),

        // Abre uma pasta do app no Explorer. Só duas pastas fixas.
        "open_folder" => {
            let p: FolderArgs = args(a)?;
            let dir = match p.which.as_str() {
                "casual" => casual::Layout::default().root,
                "cache" => analysis_cache_dir(),
                _ => return Err("pasta não permitida".into()),
            };
            open_folder(&dir)?;
            Ok(json!({ "opened": true }))
        }

        // Ícones de classe do próprio jogo, lidos da instalação do usuário (nada é distribuído com o app).
        "game_icons" => {
            let p: PathArgs = args(a)?;
            tokio::task::spawn_blocking(move || match resolve(&p.path) {
                _ if no_game_art() => out(gameicons::GameIcons::default()),
                Ok(install) => out(gameicons::load_class_icons(&install.tf_dir)),
                Err(_) => out(gameicons::GameIcons::default()),
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // Retratos das classes (o rosto dos personagens), lidos do jogo do usuário. Vazio sem o jogo.
        "class_portraits" => {
            let p: PathArgs = args(a)?;
            tokio::task::spawn_blocking(move || match resolve(&p.path) {
                _ if no_game_art() => out(gameicons::GameIcons::default()),
                Ok(install) => out(gameicons::load_class_portraits(&install.tf_dir)),
                Err(_) => out(gameicons::GameIcons::default()),
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // Baixa o arquivo (só quando o usuário pede) e explica o que ele muda, sem instalar nada.
        "analyze_file" => {
            let p: AnalyzeArgs = args(a)?;
            let details = client.details(p.kind, p.id).await.map_err(|e| e.to_string())?;
            let file = details
                .files
                .iter()
                .find(|f| f.id == p.file_id)
                .ok_or_else(|| "esse arquivo não existe nesse item do GameBanana".to_string())?;
            if file.contains_exe() {
                return Err(Error::Blocked("o GameBanana avisou que este arquivo contém executável".into()).to_string());
            }
            if archive::format_of(&file.file_name).is_none() {
                return Err(Error::UnsupportedFormat(file.file_name.clone()).to_string());
            }

            let dest = cached_zip(p.kind, file.id);
            if !dest.is_file() {
                fetch_as_zip(client, file, &dest, MAX_ANALYSIS_DOWNLOAD).await?;
            }
            prune_cache(&analysis_cache_dir(), &dest);

            let tf_dir = p.path.and_then(|s| resolve(&s).ok()).map(|i| i.tf_dir);
            organize_download(&dest, tf_dir.clone(), details.description.as_deref()).await?;
            let analysed = tokio::task::spawn_blocking(move || {
                let index = tf_dir.as_deref().map(vanilla::index_for);
                analysis::analyze_zip(&dest, index.as_deref())
            })
            .await
            .map_err(|e| e.to_string())?;
            out(analysed.map_err(|e| e.to_string())?)
        }

        // Tudo que a ficha precisa saber antes de o usuário clicar em instalar: o que o arquivo muda, com quem ele
        // brigaria e se este item já está no casual ou em custom. Baixa para o cache; não instala nada.
        "prepare_file" => {
            let p: AnalyzeArgs = args(a)?;
            let details = client.details(p.kind, p.id).await.map_err(|e| e.to_string())?;
            let file = details
                .files
                .iter()
                .find(|f| f.id == p.file_id)
                .ok_or_else(|| "esse arquivo não existe nesse item do GameBanana".to_string())?;
            if file.contains_exe() {
                return Err(Error::Blocked("o GameBanana avisou que este arquivo contém executável".into()).to_string());
            }
            if archive::format_of(&file.file_name).is_none() {
                return Err(Error::UnsupportedFormat(file.file_name.clone()).to_string());
            }

            let dest = cached_zip(p.kind, file.id);
            fetch_as_zip(client, file, &dest, MAX_ANALYSIS_DOWNLOAD).await?;
            prune_cache(&analysis_cache_dir(), &dest);

            let found = p.path.and_then(|s| resolve(&s).ok());
            organize_download(&dest, found.as_ref().map(|i| i.tf_dir.clone()), details.description.as_deref()).await?;
            let meta = ModMeta {
                gb_id: details.id,
                kind: p.kind.as_str().to_string(),
                file_id: file.id,
                file_name: file.file_name.clone(),
                name: details.name.clone(),
                version: None,
                submitter: None,
                profile_url: details.profile_url.clone(),
                license: None,
                md5: None,
            };
            tokio::task::spawn_blocking(move || -> Result<Value, String> {
                let tf_dir = found.as_ref().map(|i| i.tf_dir.clone());
                let index = tf_dir.as_deref().map(vanilla::index_for);
                let analysis = analysis::analyze_zip(&dest, index.as_deref()).map_err(|e| e.to_string())?;
                let custom = found.as_ref().map(|i| i.custom_dir());
                let l = casual::Layout::default();
                // um arquivo que não dá para instalar não briga com ninguém: a análise já explica o motivo
                let clashes = conflicts::preview_install(&dest, custom.as_deref(), &meta, &l).unwrap_or_default();
                let applied = found.as_ref().is_some_and(|i| library::preloader_state(i).applied());
                let here = casual::installed_state(&l, tf_dir.as_deref(), applied, &meta.kind, meta.gb_id);
                let in_custom = custom.is_some_and(|c| installer::list_managed(&c).iter().any(|m| m.meta.kind == meta.kind && m.meta.gb_id == meta.gb_id));
                Ok(json!({ "analysis": analysis, "clashes": clashes, "in_casual": here, "in_custom": in_custom }))
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // Toca um áudio de dentro do zip já analisado (data URL). Só áudio, só o que está no zip em cache.
        "preview_audio" => {
            let p: PreviewArgs = args(a)?;
            let zip = cached_zip(p.kind, p.file_id);
            if !zip.is_file() {
                return Err("Analise o arquivo antes de ouvir os sons.".into());
            }
            let (mime, bytes) = tokio::task::spawn_blocking(move || analysis::read_audio(&zip, &p.entry))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            Ok(json!({ "mime": mime, "data_url": format!("data:{mime};base64,{}", STANDARD.encode(bytes)) }))
        }

        // ---- Casual Preloader dentro do app. Tudo que é lento vira uma tarefa (`casual_job` mostra o andamento).
        "casual_status" => {
            let p: CasualStatusArgs = args(a)?;
            tokio::task::spawn_blocking(move || {
                let l = casual::Layout::default();
                let found = p.path.and_then(|s| resolve(&s).ok());
                let applied = found.as_ref().map(|i| library::preloader_state(i).applied());
                Ok(casual::status(&l, found.as_ref().map(|i| i.tf_dir.as_path()), applied))
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "casual_job" => out(casual::job_view()),

        // Baixa o Casual Preloader oficial (versão fixada, SHA-256 conferido) e instala as bibliotecas dele.
        "casual_setup" => {
            let l = casual::Layout::default();
            out(casual::start_job("setup", "Configurar o Casual Preloader", move |rep| casual::setup(l, casual::ReleaseSpec::pinned(), rep))?)
        }

        // Baixa o arquivo do GameBanana e o coloca na lista do casual (ainda não mexe no jogo).
        "casual_add" => {
            let p: CasualAddArgs = args(a)?;
            let l = casual::Layout::default();
            if casual::runtime(&l) != casual::Runtime::Ready {
                return Err("Configure o Casual Preloader primeiro, na aba Casual.".into());
            }
            let details = client.details(p.kind, p.id).await.map_err(|e| e.to_string())?;
            let file = details
                .files
                .iter()
                .find(|f| f.id == p.file_id)
                .ok_or_else(|| "esse arquivo não existe nesse item do GameBanana".to_string())?;
            if file.contains_exe() {
                return Err(Error::Blocked("o GameBanana avisou que este arquivo contém executável".into()).to_string());
            }
            if archive::format_of(&file.file_name).is_none() {
                return Err(Error::UnsupportedFormat(file.file_name.clone()).to_string());
            }
            if let Some(cur) = casual::job_view().filter(|j| j.state == "running") {
                return Err(format!("Já existe uma operação em andamento: {}. Espere terminar.", cur.title));
            }
            let dest = cached_zip(p.kind, file.id);
            if !dest.is_file() {
                fetch_as_zip(client, file, &dest, MAX_DOWNLOAD).await?;
            }
            prune_cache(&analysis_cache_dir(), &dest);
            let tf_dir = p.path.and_then(|s| resolve(&s).ok()).map(|i| i.tf_dir);
            organize_download(&dest, tf_dir, details.description.as_deref()).await?;
            let source = casual::AddonSource {
                kind: p.kind.as_str().to_string(),
                gb_id: details.id,
                file_id: file.id,
                file_name: file.file_name.clone(),
                name: details.name.clone(),
                version: details.version.clone().filter(|v| !v.is_empty()),
                submitter: details.submitter.as_ref().map(|s| s.name.clone()),
                profile_url: details.profile_url.clone(),
                md5: file.md5.clone(),
                added_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
            };
            let title = format!("Adicionar \"{}\" ao casual", details.name);
            let addon_name = installer::slug_parts(p.kind.as_str(), &details.name, details.id);
            out(casual::start_job("add", &title, move |rep| casual::add_zip(l, dest, addon_name, source, rep))?)
        }

        "casual_remove" => {
            let p: CasualNameArgs = args(a)?;
            if casual::job_view().is_some_and(|j| j.state == "running") {
                return Err("Espere a operação em andamento terminar.".into());
            }
            tokio::task::spawn_blocking(move || casual::remove_addon(&casual::Layout::default(), &p.name))
                .await
                .map_err(|e| e.to_string())??;
            Ok(json!({ "removed": true }))
        }

        // Quais mods entram na próxima aplicação.
        "casual_enable" => {
            let p: CasualNamesArgs = args(a)?;
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                let l = casual::Layout::default();
                let custom = p.path.as_deref().and_then(|s| resolve(s).ok()).map(|i| i.custom_dir());
                let before = casual::enabled_names(&l);
                let added: Vec<String> = p.names.iter().filter(|n| !before.contains(n)).cloned().collect();
                if let Some(c) = conflicts::casual_block(&l, custom.as_deref(), &p.names, Some(&added)).map_err(|e| e.to_string())? {
                    return Err(conflicts::refusal(&c));
                }
                casual::set_enabled(&l, p.names)
            })
            .await
            .map_err(|e| e.to_string())??;
            Ok(json!({ "saved": true }))
        }

        // Os grupos de sons de um mod do casual e a escolha de cada um.
        "casual_sounds" => {
            let p: CasualNameArgs = args(a)?;
            tokio::task::spawn_blocking(move || soundpick::families(&casual::Layout::default(), &p.name).map(|f| json!({ "families": f })))
                .await
                .map_err(|e| e.to_string())?
        }

        // Escolhe qual som toca num grupo (só na cópia do preloader; o jogo muda quando aplicar de novo).
        "casual_sound_choose" => {
            let p: SoundChooseArgs = args(a)?;
            if casual::job_view().is_some_and(|j| j.state == "running") {
                return Err("Espere a operação em andamento terminar.".into());
            }
            tokio::task::spawn_blocking(move || -> Result<Value, String> {
                let l = casual::Layout::default();
                soundpick::choose(&l, &p.name, &p.family, &p.choice)?;
                soundpick::families(&l, &p.name).map(|f| json!({ "families": f }))
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // Ouvir um som do pacote antes de escolher.
        "casual_sound_play" => {
            let p: SoundPlayArgs = args(a)?;
            tokio::task::spawn_blocking(move || -> Result<Value, String> {
                let path = soundpick::option_path(&casual::Layout::default(), &p.name, &p.file)?;
                if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(u64::MAX) > analysis::MAX_PREVIEW_BYTES {
                    return Err("este som é grande demais para a prévia".into());
                }
                let bytes = std::fs::read(&path).map_err(|e| Error::io(&path, e).to_string())?;
                let mime = match path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
                    "mp3" => "audio/mpeg",
                    "ogg" => "audio/ogg",
                    _ => "audio/wav",
                };
                Ok(json!({ "data_url": format!("data:{mime};base64,{}", STANDARD.encode(bytes)) }))
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // Quem briga com quem: tudo que está na lista do casual e em custom. Só leitura.
        "conflicts" => {
            let p: GameStateArgs = args(a)?;
            tokio::task::spawn_blocking(move || {
                let l = casual::Layout::default();
                let mut mods = conflicts::casual_mods(&l);
                if let Some(install) = p.path.as_deref().and_then(|s| resolve(s).ok()) {
                    mods.extend(conflicts::custom_mods(&install.custom_dir()).map_err(|e| e.to_string())?);
                }
                out(json!({ "clashes": conflicts::find(&mods) }))
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // Aplica os mods escolhidos no jogo. Só com o TF2 fechado.
        "casual_apply" => {
            let p: CasualApplyArgs = args(a)?;
            let l = casual::Layout::default();
            if casual::runtime(&l) != casual::Runtime::Ready {
                return Err("Configure o Casual Preloader primeiro, na aba Casual.".into());
            }
            let install = resolve(&p.path)?;
            ensure_game_closed(install.root.clone()).await?;
            let (custom, selected, layout) = (install.custom_dir(), p.addons.clone(), casual::Layout::default());
            let hit = tokio::task::spawn_blocking(move || conflicts::casual_block(&layout, Some(&custom), &selected, None))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            if let Some(c) = hit {
                return Err(conflicts::refusal(&c));
            }
            let tf_dir = install.tf_dir.clone();
            out(casual::start_job("apply", "Aplicar mods no jogo", move |rep| casual::apply(l, tf_dir, p.addons, p.options, rep))?)
        }

        // Desfaz o preloader no jogo (arquivos originais de volta). Só com o TF2 fechado.
        "casual_revert" => {
            let p: PathArgs = args(a)?;
            let l = casual::Layout::default();
            if casual::runtime(&l) != casual::Runtime::Ready {
                return Err("Configure o Casual Preloader primeiro, na aba Casual.".into());
            }
            let install = resolve(&p.path)?;
            ensure_game_closed(install.root.clone()).await?;
            let tf_dir = install.tf_dir.clone();
            out(casual::start_job("revert", "Reverter o jogo ao original", move |rep| casual::revert(l, tf_dir, rep))?)
        }

        "gb_list" => {
            let p: ListArgs = args(a)?;
            out(client.list(p.kind, p.sort, &p.query).await.map_err(|e| e.to_string())?)
        }

        "gb_details" => {
            let p: DetailsArgs = args(a)?;
            out(client.details(p.kind, p.id).await.map_err(|e| e.to_string())?)
        }

        "gb_categories" => {
            let p: CategoriesArgs = args(a)?;
            out(client.categories(p.kind, p.parent).await.map_err(|e| e.to_string())?)
        }

        other => Err(format!("comando desconhecido: {other}")),
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test(flavor = "multi_thread")]
    async fn two_requests_for_the_same_file_never_see_it_half_downloaded() {
        use std::io::{Read, Write};
        let mut zip_bytes = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut zip_bytes);
            w.start_file("sound/ui/hitsound.wav", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).unwrap();
            w.write_all(&vec![7u8; 200_000]).unwrap();
            w.finish().unwrap();
        }
        let body = zip_bytes.into_inner();
        // servidor que atende UMA vez e manda metade, espera e manda o resto
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/dl/", listener.local_addr().unwrap());
        let served = body.clone();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut req = [0u8; 2048];
            let _ = s.read(&mut req);
            write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", served.len()).unwrap();
            let half = served.len() / 2;
            s.write_all(&served[..half]).unwrap();
            s.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(600));
            s.write_all(&served[half..]).unwrap();
        });

        let client = Client::new("teste").unwrap().with_dl_prefix(base.clone());
        let file = crate::gamebanana::FileInfo { id: 9, file_name: "x.zip".into(), size: body.len() as u64, download_url: format!("{base}x.zip"), md5: None, av_result: None, analysis_warnings: None };
        let dir = std::env::temp_dir().join(format!("suny_fetch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dest = dir.join("Sound-9.zip");

        let first = fetch_as_zip(&client, &file, &dest, 1 << 20);
        let second = async {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            assert!(!dest.is_file(), "o destino não pode aparecer pela metade");
            fetch_as_zip(&client, &file, &dest, 1 << 20).await?;
            crate::scanner::zip_paths(std::fs::File::open(&dest).unwrap()).map_err(|e| e.to_string())
        };
        let (a, b) = tokio::join!(first, second);
        a.unwrap();
        assert_eq!(b.unwrap(), vec!["sound/ui/hitsound.wav".to_string()]);
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert!(!partial_path(&dest).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    use super::*;

    fn client() -> Client {
        Client::new("test").unwrap()
    }

    #[tokio::test]
    async fn unknown_command_is_an_error() {
        let e = dispatch(&client(), "apagar_tudo", json!({})).await.unwrap_err();
        assert!(e.contains("desconhecido"));
    }

    #[tokio::test]
    async fn bad_arguments_are_reported_not_panicked() {
        let e = dispatch(&client(), "gb_list", json!({ "kind": "Banana" })).await.unwrap_err();
        assert!(e.contains("argumentos inválidos"), "{e}");
        let e = dispatch(&client(), "inspect", json!({})).await.unwrap_err();
        assert!(e.contains("argumentos inválidos"), "{e}");
    }

    #[tokio::test]
    async fn inspect_rejects_a_folder_that_is_not_tf2() {
        let tmp = std::env::temp_dir().join(format!("suny_cmd_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let e = dispatch(&client(), "inspect", json!({ "path": tmp.to_string_lossy() })).await.unwrap_err();
        assert!(e.contains("Team Fortress 2"), "{e}");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn inspect_reports_preloader_and_custom_for_a_fake_install() {
        let root = std::env::temp_dir().join(format!("suny_cmd_ok_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("tf/custom/Meu Mod")).unwrap();
        std::fs::write(root.join("tf/custom/Meu Mod/a.wav"), b"RIFF").unwrap();
        std::fs::write(root.join("tf/gameinfo.txt"), "\t//type multiplayer_only").unwrap();

        let v = dispatch(&client(), "inspect", json!({ "path": root.to_string_lossy() })).await.unwrap();
        assert_eq!(v["preloader"]["gameinfo_modified"], true);
        assert_eq!(v["preloader"]["vpk_present"], false);
        assert_eq!(v["preloader_leftover"], true, "só um sinal = sobra de instalação antiga");
        assert_eq!(v["custom"][0]["name"], "Meu Mod");
        assert_eq!(v["installation"]["source"], "manual");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Teste REAL: baixa um som pequeno do GameBanana, instala numa CÓPIA do TF2 e remove.
    /// Rode: `$env:SUNY_LIVE_TF2_COPY='<raiz da cópia>'; cargo test -p suny-core live_install -- --ignored --nocapture`
    /// Nunca aponte para a instalação real do jogo.
    #[tokio::test]
    #[ignore = "usa a internet e escreve numa cópia do TF2 (SUNY_LIVE_TF2_COPY)"]
    async fn live_install_and_uninstall_small_sound() {
        let root = std::env::var("SUNY_LIVE_TF2_COPY").expect("defina SUNY_LIVE_TF2_COPY com a raiz da CÓPIA de teste");
        let c = client();

        let out = dispatch(&c, "install_mod", json!({ "path": root, "kind": "Sound", "id": 16166, "file_id": 188106 }))
            .await
            .expect("instalação real");
        let slug = out["manifest"]["slug"].as_str().unwrap().to_string();
        println!("instalado: {slug}");
        println!("arquivos: {}", out["manifest"]["files"]);
        println!("ignorados: {}", out["manifest"]["skipped"]);
        println!("compat: {} | flags: {}", out["report"]["compat"], out["report"]["flags"]);
        println!("licença: {}", out["manifest"]["meta"]["license"]);
        println!("md5: {}", out["manifest"]["meta"]["md5"]);

        let custom = std::path::Path::new(&root).join("tf").join("custom");
        assert!(custom.join(&slug).is_dir(), "a pasta do mod existe");
        assert!(!out["manifest"]["files"].as_array().unwrap().is_empty());

        // aparece em "Instalado" como gerenciado
        let v = dispatch(&c, "inspect", json!({ "path": root })).await.unwrap();
        let managed = v["custom"].as_array().unwrap().iter().filter(|e| !e["managed"].is_null()).count();
        assert_eq!(managed, 1, "um mod gerenciado na lista");

        // remove e confere que nada sobrou
        dispatch(&c, "uninstall_mod", json!({ "path": root, "slug": slug })).await.expect("remoção");
        assert!(!custom.join(&slug).exists());
        assert!(!custom.join(".suny").exists(), "sem manifestos, .suny some");
        let tmp = std::env::temp_dir().join("suny-tf2-supplier").join("Sound-188106.zip");
        assert!(!tmp.exists(), "o zip temporário foi apagado");
    }

    #[tokio::test]
    async fn preview_needs_the_analysed_zip_and_only_plays_audio() {
        // sem o zip em cache: pede para analisar primeiro
        let e = dispatch(&client(), "preview_audio", json!({ "kind": "Sound", "file_id": 987001, "entry": "sound/ui/hitsound.wav" })).await.unwrap_err();
        assert!(e.contains("Analise"), "{e}");

        // com o zip em cache
        let zip_path = cached_zip(Kind::Sound, 987002);
        std::fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
        {
            use std::io::Write;
            let mut w = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("sound/ui/hitsound.wav", o).unwrap();
            w.write_all(b"RIFFdata").unwrap();
            w.start_file("leia-me.txt", o).unwrap();
            w.write_all(b"oi").unwrap();
            w.finish().unwrap();
        }
        let ok = dispatch(&client(), "preview_audio", json!({ "kind": "Sound", "file_id": 987002, "entry": "sound/ui/hitsound.wav" })).await.unwrap();
        assert_eq!(ok["mime"], "audio/wav");
        assert_eq!(ok["data_url"], format!("data:audio/wav;base64,{}", STANDARD.encode(b"RIFFdata")));
        assert!(dispatch(&client(), "preview_audio", json!({ "kind": "Sound", "file_id": 987002, "entry": "leia-me.txt" })).await.is_err(), "texto não toca");
        assert!(dispatch(&client(), "preview_audio", json!({ "kind": "Sound", "file_id": 987002, "entry": "../../x.wav" })).await.is_err());
        assert!(dispatch(&client(), "preview_audio", json!({ "kind": "Banana", "file_id": 1, "entry": "a.wav" })).await.is_err(), "tipo inválido");
        let _ = std::fs::remove_file(&zip_path);
    }

    #[test]
    fn the_analysis_cache_is_pruned_by_age_and_count_and_keeps_the_current_file() {
        let dir = std::env::temp_dir().join(format!("suny_cache_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mk = |name: &str, age_h: u64| {
            let p = dir.join(name);
            let f = std::fs::File::create(&p).unwrap();
            f.set_modified(SystemTime::now() - Duration::from_secs(age_h * 3600)).unwrap();
            p
        };
        let old = mk("velho.zip", 30);
        let keep = mk("atual.zip", 48); // mesmo velho, o arquivo em uso não pode ser apagado
        let fresh: Vec<_> = (0..14).map(|i| mk(&format!("novo{i:02}.zip"), 1 + (i as u64 % 5))).collect();
        let not_zip = dir.join("nota.txt");
        std::fs::write(&not_zip, b"x").unwrap();

        prune_cache(&dir, &keep);
        assert!(!old.exists(), "mais de 24 h sai");
        assert!(keep.exists(), "o arquivo atual nunca é apagado");
        assert!(not_zip.exists(), "só mexe em .zip");
        let left = fresh.iter().filter(|p| p.exists()).count();
        assert!(left < CACHE_MAX_FILES, "no máximo {} + o atual, sobraram {left}", CACHE_MAX_FILES - 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_the_project_pages_can_be_opened_from_the_app() {
        assert!(is_safe_project_url("https://github.com/ArthurEnsolarado/suny-tf2-supplier"));
        assert!(is_safe_project_url("https://github.com/ArthurEnsolarado/suny-tf2-supplier/releases"));
        for bad in ["", "https://github.com/other/repo", "https://github.com/ArthurEnsolarado/suny-tf2-supplier-evil", "https://evil.example/ArthurEnsolarado/suny-tf2-supplier/", "https://github.com/ArthurEnsolarado/suny-tf2-supplier/releases && calc", "file:///C:/Windows/System32/calc.exe", "calc.exe"] {
            assert!(!is_safe_project_url(bad), "{bad:?}");
        }
        assert!(open_url("calc.exe").is_err());
    }

    #[test]
    fn play_url_carries_the_launch_options_encoded_and_still_passes_the_safety_check() {
        let url = steam_run_url_with_args(440, "-novid +exec w/config.cfg");
        assert_eq!(url, "steam://run/440//-novid%20%2Bexec%20w%2Fconfig.cfg/");
        assert!(is_safe_steam_url(&url));
        // nada que escape do formato vira comando
        for bad in ["steam://run/440//a b/", "steam://run/440//a&calc/", "steam://run/440//a\"b/", "steam://run/x//a/", "steam://run/440//a", "steam://run//a/", "steam://run/440//a/../b/"] {
            assert!(!is_safe_steam_url(bad), "{bad:?}");
        }
        assert!(!is_safe_steam_url(&format!("steam://run/440//{}/", "a".repeat(401))));
    }

    #[test]
    fn only_steam_run_urls_can_be_opened() {
        assert_eq!(steam_run_url(440), "steam://rungameid/440");
        assert!(is_safe_steam_url("steam://rungameid/440"));
        for bad in ["", "steam://rungameid/", "steam://rungameid/44 0", "steam://rungameid/440&calc", "steam://rungameid/../x", "https://evil.example", "file:///C:/Windows/System32/calc.exe", "steam://run/440", "steam://rungameid/12345678901"] {
            assert!(!is_safe_steam_url(bad), "{bad:?}");
        }
        assert!(open_steam_url("calc.exe").is_err());
    }

    #[test]
    fn gamebanana_output_uses_clean_field_names() {
        let s: crate::gamebanana::Summary = serde_json::from_value(json!({
            "_idRow": 7, "_sName": "x", "_sProfileUrl": "u"
        }))
        .unwrap();
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["id"], 7);
        assert!(v.get("_idRow").is_none());
    }
}
