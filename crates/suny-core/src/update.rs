//! Atualizações do próprio app.
//!
//! O app consulta as **releases estáveis** do repositório no GitHub (rascunhos e pré-lançamentos são ignorados),
//! junta o changelog de todas as versões novas e, só quando o usuário manda, baixa o instalador, confere o
//! SHA-256 e o abre. Sem SHA-256 publicado junto da release, o app recusa instalar sozinho.
//!
//! Limite honesto: a integridade vem do GitHub (HTTPS + hash da release). Não é assinatura de código: o Windows
//! pode mostrar o aviso do SmartScreen enquanto o instalador não for assinado.

use std::cmp::Ordering;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::casual::Reporter;

pub const REPO: &str = "ArthurEnsolarado/suny-tf2-supplier";
pub const API_BASE: &str = "https://api.github.com";
/// Tamanho máximo aceito para o instalador.
const MAX_INSTALLER: u64 = 250 << 20;
const MAX_NOTES: usize = 8000;
const MAX_RELEASES: usize = 10;

/// Versão `maior.menor.correção`. O sufixo (`-beta.1`, `+build`) é ignorado na comparação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches(['v', 'V']);
        let core = s.split(['-', '+']).next()?;
        let mut it = core.split('.');
        let a = it.next()?.parse().ok()?;
        let b = it.next().map_or(Some(0), |x| x.parse().ok())?;
        let c = it.next().map_or(Some(0), |x| x.parse().ok())?;
        if it.next().is_some() {
            return None;
        }
        Some(Self(a, b, c))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    #[serde(default)]
    size: u64,
    browser_download_url: String,
    /// `sha256:<64 hex>`: o GitHub calcula sozinho para cada arquivo de release.
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseNote {
    pub version: String,
    pub name: String,
    /// Texto da release (Markdown simples). A interface mostra como texto, nunca como HTML.
    pub notes: String,
    pub published: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Installer {
    pub name: String,
    pub size: u64,
    pub url: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub available: bool,
    /// Versões mais novas que a atual, da mais nova para a mais antiga.
    pub releases: Vec<ReleaseNote>,
    pub installer: Option<Installer>,
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("suny-tf2-supplier/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `sha256:<hex>` → `<hex>` em minúsculas.
fn digest_sha256(d: &str) -> Option<String> {
    let h = d.strip_prefix("sha256:")?;
    is_hex64(h).then(|| h.to_ascii_lowercase())
}

/// Procura `<hex>  <nome>` (formato do `sha256sum`) num texto de SHA256SUMS.
fn sums_lookup(text: &str, file: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let (hash, name) = (it.next()?, it.next()?.trim_start_matches('*'));
        (name == file && is_hex64(hash)).then(|| hash.to_ascii_lowercase())
    })
}

fn pick_installer<'a>(r: &'a GhRelease, dl_prefix: &str) -> Option<&'a GhAsset> {
    r.assets.iter().find(|a| a.name.to_ascii_lowercase().ends_with("-setup.exe") && a.browser_download_url.starts_with(dl_prefix))
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.trim().to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>().trim_end())
    }
}

/// Consulta as releases e diz se existe versão estável mais nova que `current`.
pub async fn check(current: &str) -> Result<UpdateInfo, String> {
    // Só em builds de desenvolvimento dá para apontar para um servidor falso (SUNY_UPDATE_API) e ver a tela de atualização.
    // O app final (release) ignora essa variável: a fonte das atualizações é sempre o repositório oficial.
    #[cfg(debug_assertions)]
    if let Ok(base) = std::env::var("SUNY_UPDATE_API") {
        return check_with(&base, &format!("{base}/download/"), REPO, current).await;
    }
    check_with(API_BASE, &format!("https://github.com/{REPO}/releases/download/"), REPO, current).await
}

pub async fn check_with(api_base: &str, dl_prefix: &str, repo: &str, current: &str) -> Result<UpdateInfo, String> {
    let cur = Version::parse(current).ok_or_else(|| format!("versão atual inválida: {current}"))?;
    let client = http()?;
    let url = format!("{api_base}/repos/{repo}/releases?per_page=30");
    let res = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("não consegui consultar as atualizações: {e}"))?;
    let status = res.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateInfo { current: cur.to_string(), latest: None, available: false, releases: vec![], installer: None });
    }
    if !status.is_success() {
        return Err(format!("o GitHub respondeu {status} ao consultar as atualizações"));
    }
    let list: Vec<GhRelease> = res.json().await.map_err(|e| format!("resposta inesperada do GitHub: {e}"))?;

    let mut stable: Vec<(Version, GhRelease)> = list
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| Version::parse(&r.tag_name).map(|v| (v, r)))
        .collect();
    stable.sort_by(|a, b| b.0.cmp(&a.0));

    let latest = stable.first().map(|(v, _)| *v);
    let newer: Vec<&(Version, GhRelease)> = stable.iter().filter(|(v, _)| v.cmp(&cur) == Ordering::Greater).take(MAX_RELEASES).collect();
    let releases: Vec<ReleaseNote> = newer
        .iter()
        .map(|(v, r)| ReleaseNote {
            version: v.to_string(),
            name: r.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("v{v}")),
            notes: clip(r.body.as_deref().unwrap_or(""), MAX_NOTES),
            published: r.published_at.clone(),
            url: r.html_url.clone(),
        })
        .collect();

    let mut installer = None;
    if let Some((_, r)) = newer.first() {
        if let Some(a) = pick_installer(r, dl_prefix) {
            let mut sha = a.digest.as_deref().and_then(digest_sha256);
            if sha.is_none() {
                // sem digest: tenta um SHA256SUMS.txt da mesma release
                if let Some(sums) = r.assets.iter().find(|x| x.name.eq_ignore_ascii_case("SHA256SUMS.txt") && x.browser_download_url.starts_with(dl_prefix) && x.size <= 64 * 1024) {
                    if let Ok(text) = client.get(&sums.browser_download_url).send().await.and_then(|x| x.error_for_status()) {
                        if let Ok(body) = text.text().await {
                            sha = sums_lookup(&body, &a.name);
                        }
                    }
                }
            }
            installer = Some(Installer { name: a.name.clone(), size: a.size, url: a.browser_download_url.clone(), sha256: sha });
        }
    }

    Ok(UpdateInfo { current: cur.to_string(), latest: latest.map(|v| v.to_string()), available: !releases.is_empty(), releases, installer })
}

/// Baixa o instalador, confere tamanho e SHA-256 e o abre. Recusa se a release não publicou o hash.
pub async fn download_and_launch(info: Installer, rep: Reporter) -> Result<serde_json::Value, String> {
    let sha = info.sha256.clone().ok_or_else(|| "A release não publicou o SHA-256 do instalador, então o app não instala sozinho. Baixe pelo site do projeto.".to_string())?;
    if info.size == 0 || info.size > MAX_INSTALLER {
        return Err(format!("o instalador tem um tamanho inesperado ({} bytes). Recusado.", info.size));
    }
    let safe_name: String = info.name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')).collect();
    let dest = std::env::temp_dir().join("suny-tf2-supplier").join("update").join(&safe_name);
    rep.progress(1, "Baixando a atualização…");
    download_verified(&info.url, &dest, info.size, &sha, &rep).await?;
    rep.progress(97, "Abrindo o instalador…");
    launch(&dest)?;
    Ok(serde_json::json!({ "launched": true, "installer": dest }))
}

async fn download_verified(url: &str, dest: &Path, size: u64, sha256: &str, rep: &Reporter) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("suny-tf2-supplier/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(900))
        .build()
        .map_err(|e| e.to_string())?;
    let mut res = client.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|e| format!("não consegui baixar a atualização: {e}"))?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = std::fs::File::create(dest).map_err(|e| format!("não consegui criar {}: {e}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut got = 0u64;
    loop {
        let Some(chunk) = res.chunk().await.map_err(|e| format!("a conexão caiu durante o download: {e}"))? else { break };
        got += chunk.len() as u64;
        if got > size {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err("o download passou do tamanho esperado. Recusado.".into());
        }
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|e| format!("não consegui gravar o download: {e}"))?;
        rep.progress(1 + (got * 94 / size.max(1)) as u8, &format!("Baixando a atualização… {} de {} MB", got >> 20, size >> 20));
    }
    drop(file);
    let actual: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if got != size || !actual.eq_ignore_ascii_case(sha256) {
        let _ = std::fs::remove_file(dest);
        return Err(format!("o download não bate com o SHA-256 publicado na release ({actual}). Nada foi aberto."));
    }
    Ok(())
}

fn launch(installer: &Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new(installer);
    cmd.current_dir(installer.parent().unwrap_or_else(|| Path::new(".")));
    cmd.spawn().map(|_| ()).map_err(|e| format!("não consegui abrir o instalador: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_parse_and_compare_numerically() {
        assert_eq!(Version::parse("v0.7.0"), Some(Version(0, 7, 0)));
        assert_eq!(Version::parse("1.2"), Some(Version(1, 2, 0)));
        assert_eq!(Version::parse("2.0.1-beta.3+abc"), Some(Version(2, 0, 1)));
        assert!(Version::parse("0.10.0") > Version::parse("0.9.9"), "10 é maior que 9 (não é texto)");
        for bad in ["", "v", "x.y.z", "1.2.3.4", "1..2"] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
        assert_eq!(Version(0, 7, 3).to_string(), "0.7.3");
    }

    #[test]
    fn digests_and_sums_are_parsed_strictly() {
        let h = "a".repeat(64);
        assert_eq!(digest_sha256(&format!("sha256:{h}")), Some(h.clone()));
        assert_eq!(digest_sha256(&format!("sha256:{}", h.to_uppercase())), Some(h.clone()), "vira minúsculo");
        assert_eq!(digest_sha256("sha1:abc"), None);
        assert_eq!(digest_sha256("sha256:xyz"), None);
        let sums = format!("{h}  app_setup.exe\n{}  *outro.exe\nlixo\n", "b".repeat(64));
        assert_eq!(sums_lookup(&sums, "app_setup.exe"), Some(h));
        assert_eq!(sums_lookup(&sums, "outro.exe"), Some("b".repeat(64)));
        assert_eq!(sums_lookup(&sums, "nao.exe"), None);
    }

    fn release(tag: &str, pre: bool, draft: bool, installer: bool) -> serde_json::Value {
        let assets = if installer {
            serde_json::json!([{ "name": format!("suny-tf2-supplier_{tag}_x64-setup.exe"), "size": 5, "browser_download_url": format!("https://github.com/{REPO}/releases/download/v{tag}/suny-tf2-supplier_{tag}_x64-setup.exe"), "digest": format!("sha256:{}", "c".repeat(64)) },
                { "name": "evil.exe", "size": 5, "browser_download_url": "https://evil.example/x-setup.exe" }])
        } else {
            serde_json::json!([])
        };
        serde_json::json!({ "tag_name": format!("v{tag}"), "name": format!("Versão {tag}"), "body": format!("- mudança {tag}"), "html_url": format!("https://github.com/{REPO}/releases/tag/v{tag}"), "published_at": "2026-11-21T10:00:00Z", "prerelease": pre, "draft": draft, "assets": assets })
    }

    fn serve(status: u16, body: String) -> (String, std::thread::JoinHandle<()>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.server_addr().to_ip().unwrap());
        let h = std::thread::spawn(move || {
            if let Ok(Some(req)) = server.recv_timeout(Duration::from_secs(10)) {
                let _ = req.respond(tiny_http::Response::from_string(body).with_status_code(status).with_header(tiny_http::Header::from_bytes("content-type", "application/json").unwrap()));
            }
        });
        (base, h)
    }

    #[tokio::test]
    async fn only_stable_releases_count_and_changelog_lists_every_newer_one() {
        let body = serde_json::json!([
            release("0.9.0-beta.1", true, false, true),
            release("0.8.0", false, false, true),
            release("0.7.5", false, true, true),
            release("0.7.1", false, false, false),
            release("0.7.0", false, false, true),
            release("0.6.0", false, false, true),
        ])
        .to_string();
        let (base, h) = serve(200, body);
        let prefix = format!("https://github.com/{REPO}/releases/download/");
        let info = check_with(&base, &prefix, REPO, "0.7.0").await.unwrap();
        h.join().unwrap();
        assert!(info.available);
        assert_eq!(info.latest.as_deref(), Some("0.8.0"), "pré-lançamento e rascunho não valem");
        assert_eq!(info.releases.iter().map(|r| r.version.as_str()).collect::<Vec<_>>(), ["0.8.0", "0.7.1"], "todas as novas, da mais nova para a mais antiga");
        assert_eq!(info.releases[0].notes, "- mudança 0.8.0");
        let inst = info.installer.unwrap();
        assert!(inst.name.ends_with("-setup.exe") && inst.url.starts_with(&prefix), "só aceita arquivo do repositório: {inst:?}");
        assert_eq!(inst.sha256, Some("c".repeat(64)));
    }

    #[tokio::test]
    async fn up_to_date_and_no_releases_are_not_errors() {
        let (base, h) = serve(200, serde_json::json!([release("0.7.0", false, false, true)]).to_string());
        let info = check_with(&base, "https://github.com/", REPO, "0.7.0").await.unwrap();
        h.join().unwrap();
        assert!(!info.available && info.releases.is_empty() && info.installer.is_none());
        assert_eq!(info.latest.as_deref(), Some("0.7.0"));

        let (base, h) = serve(404, "{}".into());
        let info = check_with(&base, "https://github.com/", REPO, "0.7.0").await.unwrap();
        h.join().unwrap();
        assert!(!info.available && info.latest.is_none());

        let (base, h) = serve(500, "{}".into());
        assert!(check_with(&base, "https://github.com/", REPO, "0.7.0").await.is_err());
        h.join().unwrap();
    }

    #[tokio::test]
    async fn installer_without_a_published_hash_is_refused() {
        let info = Installer { name: "x-setup.exe".into(), size: 5, url: "https://github.com/x".into(), sha256: None };
        let e = download_and_launch(info, crate::casual::Reporter::for_tests()).await.unwrap_err();
        assert!(e.contains("SHA-256"), "{e}");
        let big = Installer { name: "x-setup.exe".into(), size: MAX_INSTALLER + 1, url: "https://github.com/x".into(), sha256: Some("a".repeat(64)) };
        assert!(download_and_launch(big, crate::casual::Reporter::for_tests()).await.is_err());
    }

    #[tokio::test]
    async fn download_is_checked_against_the_published_hash() {
        let body = b"instalador falso".to_vec();
        let sha: String = Sha256::digest(&body).iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("suny_upd_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let rep = crate::casual::Reporter::for_tests();

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/x-setup.exe", server.server_addr().to_ip().unwrap());
        let b2 = body.clone();
        let h = std::thread::spawn(move || {
            for _ in 0..2 {
                if let Ok(Some(req)) = server.recv_timeout(Duration::from_secs(10)) {
                    let _ = req.respond(tiny_http::Response::from_data(b2.clone()));
                }
            }
        });
        let ok = dir.join("ok.exe");
        download_verified(&url, &ok, body.len() as u64, &sha, &rep).await.unwrap();
        assert_eq!(std::fs::read(&ok).unwrap(), body);

        let bad = dir.join("bad.exe");
        let e = download_verified(&url, &bad, body.len() as u64, &"0".repeat(64), &rep).await.unwrap_err();
        assert!(e.contains("SHA-256"), "{e}");
        assert!(!bad.exists(), "hash errado: o arquivo é apagado e nunca é aberto");
        h.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
