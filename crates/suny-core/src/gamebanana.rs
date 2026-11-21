//! Cliente tipado da API pública do GameBanana (`apiv11`).
//!
//! Endpoints e limites foram testados em 2026-11-21 (ver `docs/RESEARCH.md`):
//! `_nPerpage` máximo é 50 e a API não tem tag "casual compatible".

use std::time::Duration;

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::{Error, Result, TF2_GAMEBANANA_ID};

const BASE: &str = "https://gamebanana.com/apiv11";
pub const MAX_PER_PAGE: u32 = 50;

/// Tipo de submissão. Áudio é um tipo separado de "Mod".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Mod,
    Sound,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        self.path()
    }

    fn path(self) -> &'static str {
        match self {
            Kind::Mod => "Mod",
            Kind::Sound => "Sound",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    MostDownloaded,
    Newest,
    MostLiked,
}

impl Sort {
    fn param(self) -> &'static str {
        match self {
            Sort::MostDownloaded => "Generic_MostDownloaded",
            // Nomes conferidos na API real em 2026-11-21. "new" e "default" dão UNKNOWN_SORT.
            Sort::Newest => "Generic_Newest",
            Sort::MostLiked => "Generic_MostLiked",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Query {
    pub page: u32,
    pub per_page: u32,
    pub category: Option<u32>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Page<T> {
    pub total: u64,
    pub is_complete: bool,
    /// Tamanho de página que o GameBanana usou de verdade (a busca por nome devolve 15, mesmo pedindo mais).
    pub per_page: u32,
    pub records: Vec<T>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Submitter {
    #[serde(rename(deserialize = "_idRow"))]
    pub id: u64,
    #[serde(rename(deserialize = "_sName"))]
    pub name: String,
    #[serde(rename(deserialize = "_sProfileUrl"))]
    pub profile_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Category {
    #[serde(rename(deserialize = "_idRow"), default)]
    pub id: u64,
    #[serde(rename(deserialize = "_sName"), default)]
    pub name: String,
    #[serde(rename(deserialize = "_nItemCount"), default)]
    pub item_count: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Summary {
    #[serde(rename(deserialize = "_idRow"))]
    pub id: u64,
    #[serde(rename(deserialize = "_sName"))]
    pub name: String,
    #[serde(rename(deserialize = "_sProfileUrl"))]
    pub profile_url: String,
    #[serde(rename(deserialize = "_aSubmitter"))]
    pub submitter: Option<Submitter>,
    #[serde(rename(deserialize = "_aRootCategory"))]
    pub root_category: Option<Category>,
    #[serde(rename(deserialize = "_nLikeCount"), default)]
    pub like_count: u64,
    #[serde(rename(deserialize = "_nViewCount"), default)]
    pub view_count: u64,
    /// Miniatura (URL do CDN do GameBanana), quando a submissão tem imagem.
    #[serde(default, rename(deserialize = "_aPreviewMedia"), deserialize_with = "thumb_from_media")]
    pub thumb: Option<String>,
}

/// Só aceitamos imagens do CDN do próprio GameBanana (a interface carrega essas URLs direto).
const IMAGE_HOST: &str = "https://images.gamebanana.com/";

/// Monta as URLs das imagens de divulgação, preferindo os campos de `prefer` na ordem dada.
fn media_urls(media: &serde_json::Value, prefer: &[&str], limit: usize) -> Vec<String> {
    let Some(images) = media.get("_aImages").and_then(|v| v.as_array()) else { return Vec::new() };
    images
        .iter()
        .filter_map(|img| {
            let base = img.get("_sBaseUrl")?.as_str()?;
            let file = prefer.iter().find_map(|k| img.get(*k).and_then(|f| f.as_str()))?;
            let url = format!("{base}/{file}");
            url.starts_with(IMAGE_HOST).then_some(url)
        })
        .take(limit)
        .collect()
}

fn thumb_from_media<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|m| media_urls(&m, &["_sFile530", "_sFile220", "_sFile100", "_sFile"], 1).into_iter().next()))
}

/// Uma imagem da galeria: a grande (para o visualizador) e a miniatura (para a tira de baixo).
#[derive(Debug, Clone, Serialize)]
pub struct GalleryImage {
    pub url: String,
    pub thumb: String,
}

/// Até 12 imagens, só do CDN do GameBanana. Ignora vídeos e o que não for imagem.
fn images_from_media<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<GalleryImage>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    let Some(m) = v else { return Ok(Vec::new()) };
    let Some(images) = m.get("_aImages").and_then(|v| v.as_array()) else { return Ok(Vec::new()) };
    let pick = |img: &serde_json::Value, keys: &[&str]| -> Option<String> {
        let base = img.get("_sBaseUrl")?.as_str()?;
        let file = keys.iter().find_map(|k| img.get(*k).and_then(|f| f.as_str()))?;
        let url = format!("{base}/{file}");
        url.starts_with(IMAGE_HOST).then_some(url)
    };
    Ok(images
        .iter()
        .filter_map(|img| {
            let url = pick(img, &["_sFile800", "_sFile530", "_sFile"])?;
            let thumb = pick(img, &["_sFile220", "_sFile100", "_sFile530", "_sFile"]).unwrap_or_else(|| url.clone());
            Some(GalleryImage { url, thumb })
        })
        .take(12)
        .collect())
}

/// Texto da descrição sem HTML: parágrafos viram linhas, entidades comuns são trocadas, no máximo 3000 caracteres.
pub fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(4000));
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for n in chars.by_ref() {
                if n == '>' {
                    break;
                }
                tag.push(n);
            }
            let name = tag.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("").to_ascii_lowercase();
            if matches!(name.as_str(), "br" | "p" | "div" | "li" | "h1" | "h2" | "h3" | "h4") {
                out.push('\n');
            }
        } else {
            out.push(c);
        }
        if out.len() > 6000 {
            break;
        }
    }
    let out = out.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'");
    // no máximo uma linha em branco seguida
    let mut lines: Vec<&str> = Vec::new();
    for l in out.lines().map(str::trim) {
        if l.is_empty() && lines.last().is_none_or(|p| p.is_empty()) {
            continue;
        }
        lines.push(l);
    }
    let joined = lines.join("\n").trim().to_string();
    joined.chars().take(3000).collect()
}

fn text_from_html<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(d)?.map(|s| strip_html(&s)).filter(|s| !s.is_empty()))
}

fn tags_from_list<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<String>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|t| t.get("_sValue").or_else(|| t.get("_sTitle")).and_then(|x| x.as_str()).map(str::to_string))
        .filter(|s| !s.is_empty())
        .take(12)
        .collect())
}

/// Um arquivo baixável de uma submissão.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileInfo {
    #[serde(rename(deserialize = "_idRow"))]
    pub id: u64,
    #[serde(rename(deserialize = "_sFile"))]
    pub file_name: String,
    #[serde(rename(deserialize = "_nFilesize"), default)]
    pub size: u64,
    #[serde(rename(deserialize = "_sDownloadUrl"))]
    pub download_url: String,
    #[serde(rename = "_sMd5Checksum", default)]
    pub md5: Option<String>,
    /// Resultado do antivírus do GameBanana (`clean`, ...).
    #[serde(rename(deserialize = "_sAvResult"), default)]
    pub av_result: Option<String>,
    /// Avisos da análise (ex.: `contains_exe`). Vem como objeto ou nulo.
    #[serde(rename(deserialize = "_aAnalysisWarnings"), default)]
    pub analysis_warnings: Option<serde_json::Value>,
}

impl FileInfo {
    /// `true` se o GameBanana avisou que há executável dentro do arquivo.
    pub fn contains_exe(&self) -> bool {
        self.analysis_warnings
            .as_ref()
            .and_then(|w| w.get("contains_exe"))
            .is_some()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Details {
    #[serde(rename(deserialize = "_idRow"))]
    pub id: u64,
    #[serde(rename(deserialize = "_sName"))]
    pub name: String,
    #[serde(rename(deserialize = "_sProfileUrl"))]
    pub profile_url: String,
    #[serde(rename(deserialize = "_aSubmitter"))]
    pub submitter: Option<Submitter>,
    #[serde(rename(deserialize = "_aCategory"))]
    pub category: Option<Category>,
    #[serde(rename(deserialize = "_aFiles"), default)]
    pub files: Vec<FileInfo>,
    #[serde(rename(deserialize = "_sLicense"), default)]
    pub license: Option<String>,
    #[serde(rename(deserialize = "_sVersion"), default)]
    pub version: Option<String>,
    #[serde(rename(deserialize = "_bIsObsolete"), default)]
    pub is_obsolete: bool,
    /// Até 12 imagens de divulgação (URLs do CDN do GameBanana).
    #[serde(default, rename(deserialize = "_aPreviewMedia"), deserialize_with = "images_from_media")]
    pub images: Vec<GalleryImage>,
    /// Descrição do autor, só texto.
    #[serde(default, rename(deserialize = "_sText"), deserialize_with = "text_from_html")]
    pub description: Option<String>,
    #[serde(default, rename(deserialize = "_nLikeCount"))]
    pub like_count: u64,
    #[serde(default, rename(deserialize = "_nViewCount"))]
    pub view_count: u64,
    #[serde(default, rename(deserialize = "_nDownloadCount"))]
    pub download_count: u64,
    /// Datas em segundos desde 1970.
    #[serde(default, rename(deserialize = "_tsDateAdded"))]
    pub date_added: Option<u64>,
    #[serde(default, rename(deserialize = "_tsDateModified"))]
    pub date_modified: Option<u64>,
    #[serde(default, rename(deserialize = "_aTags"), deserialize_with = "tags_from_list")]
    pub tags: Vec<String>,
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    /// Só baixamos de endereços que começam com isto (nunca de uma URL qualquer).
    dl_prefix: String,
    /// Pausa entre tentativas quando o GameBanana responde vazio ou com erro de servidor.
    retry_delay: Duration,
}

/// 1 tentativa + 2 repetições.
const ATTEMPTS: u32 = 3;
/// Tentativas de um download: cada uma continua de onde a anterior caiu.
const DOWNLOAD_ATTEMPTS: u32 = 6;
/// Limite das consultas curtas (listas, fichas).
const API_TIMEOUT: Duration = Duration::from_secs(20);

const DEFAULT_DL_PREFIX: &str = "https://gamebanana.com/dl/";

impl Client {
    /// `app_version` entra no User-Agent, para o GameBanana saber quem somos.
    pub fn new(app_version: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(format!("suny-tf2-supplier/{app_version}"))
            // Nada de limite total: ele cortava qualquer download que passasse de 20 s (rede lenta, WARP, faculdade).
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30)) // só conta se a conexão ficar PARADA por 30 s
            .build()?;
        Ok(Self { http, base: BASE.to_string(), dl_prefix: DEFAULT_DL_PREFIX.to_string(), retry_delay: Duration::from_millis(500) })
    }

    /// Só para testes ou espelho: troca o endereço base.
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    pub async fn list(&self, kind: Kind, sort: Sort, q: &Query) -> Result<Page<Summary>> {
        let url = self.list_url(kind, sort, q);
        let body = self.get_text(&url).await?;
        parse_page(&body)
    }

    pub async fn details(&self, kind: Kind, id: u64) -> Result<Details> {
        let url = format!("{}/{}/{}/ProfilePage", self.base, kind.path(), id);
        let body = self.get_text(&url).await?;
        check_error(&body)?;
        Ok(serde_json::from_str(&body)?)
    }

    /// Categorias de primeiro nível (`parent = None`) ou as filhas de uma categoria.
    /// A API devolve no máximo 50 e IGNORA `_nPage` (testado em 2026-11-21): não há paginação aqui.
    pub async fn categories(&self, kind: Kind, parent: Option<u32>) -> Result<Vec<Category>> {
        let url = self.categories_url(kind, parent);
        let body = self.get_text(&url).await?;
        check_error(&body)?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let records = v.get("_aRecords").cloned().unwrap_or(v);
        Ok(serde_json::from_value(records)?)
    }

    /// Só para testes: pausa curta entre tentativas.
    pub fn with_retry_delay(mut self, d: Duration) -> Self {
        self.retry_delay = d;
        self
    }

    /// Só para testes: troca o prefixo de download permitido.
    pub fn with_download_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.dl_prefix = prefix.into();
        self
    }

    /// Baixa um arquivo para o disco, aos poucos, conferindo o MD5 informado pelo GameBanana.
    /// Recusa endereços fora do GameBanana e arquivos acima de `max_bytes` (declarado ou real).
    /// Se algo falhar, o arquivo parcial é apagado.
    pub async fn download_to(&self, file: &FileInfo, dest: &std::path::Path, max_bytes: u64) -> Result<u64> {
        use std::io::Write;

        if !file.download_url.starts_with(&self.dl_prefix) {
            return Err(Error::Blocked(format!("endereço de download fora do GameBanana: {}", file.download_url)));
        }
        if file.size > max_bytes {
            return Err(Error::TooLarge(format!("{} bytes declarados", file.size)));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }

        let result = async {
            let mut out = std::fs::File::create(dest).map_err(|e| Error::io(dest, e))?;
            let mut hasher = Md5::new();
            let mut total: u64 = 0;
            let mut last: Option<Error> = None;
            for attempt in 0..DOWNLOAD_ATTEMPTS {
                if attempt > 0 {
                    tokio::time::sleep(self.retry_delay * attempt).await;
                }
                // Depois de uma queda, pede só o que falta (Range). Se o servidor ignorar o pedido, recomeça do zero.
                let mut req = self.http.get(&file.download_url);
                if total > 0 {
                    req = req.header("Range", format!("bytes={total}-"));
                }
                let resp = match req.send().await {
                    Ok(r) => r,
                    Err(e) => {
                        last = Some(e.into());
                        continue;
                    }
                };
                let status = resp.status();
                if status.is_server_error() || status.as_u16() == 429 {
                    last = resp.error_for_status().map(|_| ()).err().map(Error::from);
                    continue;
                }
                if total > 0 && status != reqwest::StatusCode::PARTIAL_CONTENT {
                    if status != reqwest::StatusCode::OK {
                        // 416 e afins: o que já veio não serve de ponto de partida
                        last = Some(Error::Network(format!("o servidor respondeu {status} ao pedir a continuação do download")));
                    }
                    out = std::fs::File::create(dest).map_err(|e| Error::io(dest, e))?;
                    hasher = Md5::new();
                    total = 0;
                    if status != reqwest::StatusCode::OK {
                        continue;
                    }
                }
                let mut resp = resp.error_for_status()?;
                loop {
                    match resp.chunk().await {
                        Ok(Some(chunk)) => {
                            total += chunk.len() as u64;
                            if total > max_bytes {
                                return Err(Error::TooLarge(format!("passou de {max_bytes} bytes durante o download")));
                            }
                            hasher.update(&chunk);
                            out.write_all(&chunk).map_err(|e| Error::io(dest, e))?;
                        }
                        Ok(None) => {
                            out.flush().map_err(|e| Error::io(dest, e))?;
                            if let Some(expected) = file.md5.as_deref().filter(|m| !m.is_empty()) {
                                let actual = format!("{:x}", hasher.finalize());
                                if !actual.eq_ignore_ascii_case(expected.trim()) {
                                    return Err(Error::ChecksumMismatch { expected: expected.to_string(), actual });
                                }
                            }
                            return Ok(total);
                        }
                        Err(e) => {
                            last = Some(e.into()); // a conexão caiu no meio: tenta continuar de onde parou
                            break;
                        }
                    }
                }
            }
            Err(last.unwrap_or(Error::EmptyResponse))
        }
        .await;

        if result.is_err() {
            let _ = std::fs::remove_file(dest);
        }
        result
    }

    /// Atenção: para listar as FILHAS de uma categoria o `_idGameRow` NÃO pode ir junto
    /// (com ele a API ignora o pai e devolve só as categorias raiz).
    fn categories_url(&self, kind: Kind, parent: Option<u32>) -> String {
        match parent {
            Some(p) => format!("{}/{}/Categories?_idCategoryRow={}&_sSort=a_to_z&_nPerpage={}", self.base, kind.path(), p, MAX_PER_PAGE),
            None => format!(
                "{}/{}/Categories?_idGameRow={}&_sSort=a_to_z&_nPerpage={}",
                self.base,
                kind.path(),
                TF2_GAMEBANANA_ID,
                MAX_PER_PAGE
            ),
        }
    }

    fn list_url(&self, kind: Kind, sort: Sort, q: &Query) -> String {
        let per_page = if q.per_page == 0 { 20 } else { q.per_page.min(MAX_PER_PAGE) };
        let page = q.page.max(1);
        match q.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(text) => format!(
                "{}/Util/Search/Results?_sSearchString={}&_idGameRow={}&_sModelName={}&_nPerpage={}&_nPage={}",
                self.base,
                encode(text),
                TF2_GAMEBANANA_ID,
                kind.path(),
                per_page,
                page
            ),
            None => {
                let mut url = format!(
                    "{}/{}/Index?_aFilters%5BGeneric_Game%5D={}&_sSort={}&_nPerpage={}&_nPage={}",
                    self.base,
                    kind.path(),
                    TF2_GAMEBANANA_ID,
                    sort.param(),
                    per_page,
                    page
                );
                if let Some(cat) = q.category {
                    url.push_str(&format!("&_aFilters%5BGeneric_Category%5D={cat}"));
                }
                url.push_str("&_csvProperties=_idRow,_sName,_sProfileUrl,_aSubmitter,_aRootCategory,_nLikeCount,_nViewCount,_aPreviewMedia");
                url
            }
        }
    }

    /// GET com repetição: o GameBanana (atrás do Cloudflare) às vezes devolve corpo vazio ou 5xx por um instante.
    /// Erros 4xx (pedido errado) não repetem. Se todas as tentativas falharem, devolve o último erro.
    async fn get_text(&self, url: &str) -> Result<String> {
        let mut last = Error::EmptyResponse;
        for attempt in 0..ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(self.retry_delay * attempt).await;
            }
            let resp = match self.http.get(url).timeout(API_TIMEOUT).send().await {
                Ok(r) => r,
                Err(e) => {
                    last = e.into();
                    continue;
                }
            };
            let status = resp.status();
            if status.is_server_error() || status.as_u16() == 429 {
                last = resp.error_for_status().map(|_| ()).err().map(Error::from).unwrap_or(Error::EmptyResponse);
                continue;
            }
            let resp = resp.error_for_status()?;
            match resp.text().await {
                Ok(t) if !t.trim().is_empty() => return Ok(t),
                Ok(_) => last = Error::EmptyResponse,
                Err(e) => last = e.into(),
            }
        }
        Err(last)
    }
}

fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A API devolve erros como `{"_sErrorCode": "...", "_sErrorMessage": "..."}` com HTTP 200.
fn check_error(body: &str) -> Result<()> {
    let v: serde_json::Value = serde_json::from_str(body)?;
    if let Some(code) = v.get("_sErrorCode").and_then(|c| c.as_str()) {
        let message = v
            .get("_sErrorMessage")
            .and_then(|m| m.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| v.get("_aErrorData").map(|d| d.to_string()).unwrap_or_default());
        return Err(Error::GameBanana { code: code.to_string(), message });
    }
    Ok(())
}

fn parse_page(body: &str) -> Result<Page<Summary>> {
    check_error(body)?;
    let v: serde_json::Value = serde_json::from_str(body)?;
    let meta = v.get("_aMetadata");
    Ok(Page {
        total: meta.and_then(|m| m.get("_nRecordCount")).and_then(|n| n.as_u64()).unwrap_or(0),
        is_complete: meta.and_then(|m| m.get("_bIsComplete")).and_then(|b| b.as_bool()).unwrap_or(false),
        per_page: meta.and_then(|m| m.get("_nPerpage")).and_then(|n| n.as_u64()).unwrap_or(0) as u32,
        records: serde_json::from_value(v.get("_aRecords").cloned().unwrap_or_default())?,
    })
}

pub fn verify_md5(bytes: &[u8], expected: &str) -> Result<()> {
    let actual = format!("{:x}", Md5::digest(bytes));
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(Error::ChecksumMismatch { expected: expected.to_string(), actual })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trechos reais (encurtados) das respostas da API em 2026-11-21.
    const INDEX: &str = r#"{
      "_aMetadata": {"_nRecordCount": 1358, "_bIsComplete": false, "_nPerpage": 2},
      "_aRecords": [{
        "_idRow": 435309, "_sName": "square_series",
        "_sProfileUrl": "https://gamebanana.com/mods/435309",
        "_aSubmitter": {"_idRow": 2133251, "_sName": "THE GOAT", "_sProfileUrl": "https://gamebanana.com/members/2133251"},
        "_aRootCategory": {"_idRow": 1090, "_sName": "Effects"},
        "_nLikeCount": 10, "_nViewCount": 99
      }]}"#;

    const PROFILE: &str = r#"{
      "_idRow": 19049, "_sName": "Casual Preloader",
      "_sProfileUrl": "https://gamebanana.com/tools/19049",
      "_sLicense": "GNU GENERAL PUBLIC LICENSE", "_sVersion": "v2.3.0",
      "_aFiles": [{
        "_idRow": 1803523, "_sFile": "casual-preloader-v230.zip", "_nFilesize": 19065278,
        "_sDownloadUrl": "https://gamebanana.com/dl/1803523",
        "_sMd5Checksum": "ce35bcec9bb60c227180a99bb9112b9d", "_sAvResult": "clean",
        "_aAnalysisWarnings": {"contains_exe": ["RUNME.bat"]}
      },{
        "_idRow": 1, "_sFile": "skin.zip", "_nFilesize": 10,
        "_sDownloadUrl": "https://gamebanana.com/dl/1", "_aAnalysisWarnings": null
      }]}"#;

    #[test]
    fn parses_index_page() {
        let page = parse_page(INDEX).unwrap();
        assert_eq!(page.total, 1358);
        assert!(!page.is_complete);
        assert_eq!(page.records[0].name, "square_series");
        assert_eq!(page.records[0].root_category.as_ref().unwrap().name, "Effects");
        assert_eq!(page.records[0].submitter.as_ref().unwrap().name, "THE GOAT");
    }

    #[test]
    fn parses_details_and_exe_warning() {
        let d: Details = serde_json::from_str(PROFILE).unwrap();
        assert_eq!(d.files.len(), 2);
        assert!(d.files[0].contains_exe());
        assert!(!d.files[1].contains_exe());
        assert_eq!(d.files[0].md5.as_deref(), Some("ce35bcec9bb60c227180a99bb9112b9d"));
    }

    #[test]
    fn api_errors_become_typed_errors() {
        let body = r#"{"_sErrorCode":"INPUT_ERRORS","_aErrorData":{"_nPerpage":{"_sErrorCode":"INVALID_PERPAGE"}}}"#;
        match parse_page(body) {
            Err(Error::GameBanana { code, .. }) => assert_eq!(code, "INPUT_ERRORS"),
            other => panic!("esperava erro do GameBanana, veio {other:?}"),
        }
    }

    #[test]
    fn thumbnails_come_only_from_the_gamebanana_cdn() {
        let ok: Summary = serde_json::from_value(serde_json::json!({
            "_idRow": 1, "_sName": "x", "_sProfileUrl": "u",
            "_aPreviewMedia": {"_aImages": [{
                "_sBaseUrl": "https://images.gamebanana.com/img/ss/mods",
                "_sFile": "a.jpg", "_sFile220": "220-90_a.jpg", "_sFile100": "100-90_a.jpg"
            }]}
        })).unwrap();
        assert_eq!(ok.thumb.as_deref(), Some("https://images.gamebanana.com/img/ss/mods/220-90_a.jpg"));

        let evil: Summary = serde_json::from_value(serde_json::json!({
            "_idRow": 1, "_sName": "x", "_sProfileUrl": "u",
            "_aPreviewMedia": {"_aImages": [{"_sBaseUrl": "https://evil.example/img", "_sFile": "a.jpg"}]}
        })).unwrap();
        assert!(evil.thumb.is_none(), "host fora do CDN do GameBanana é recusado");

        let none: Summary = serde_json::from_value(serde_json::json!({"_idRow": 1, "_sName": "x", "_sProfileUrl": "u"})).unwrap();
        assert!(none.thumb.is_none());
    }

    #[test]
    fn details_keep_up_to_twelve_gallery_images_with_thumbs() {
        let imgs: Vec<_> = (0..15)
            .map(|i| serde_json::json!({"_sBaseUrl": "https://images.gamebanana.com/img/ss/mods", "_sFile": format!("{i}.jpg"), "_sFile530": format!("530-90_{i}.jpg"), "_sFile220": format!("220-90_{i}.jpg")}))
            .chain([serde_json::json!({"_sBaseUrl": "https://evil.example/img", "_sFile": "x.jpg"})])
            .collect();
        let d: Details = serde_json::from_value(serde_json::json!({
            "_idRow": 1, "_sName": "x", "_sProfileUrl": "u", "_aPreviewMedia": {"_aImages": imgs}
        })).unwrap();
        assert_eq!(d.images.len(), 12);
        assert!(d.images[0].url.ends_with("/530-90_0.jpg"), "sem _sFile800, usa a de 530");
        assert!(d.images[0].thumb.ends_with("/220-90_0.jpg"));
        assert!(d.images.iter().all(|i| i.url.starts_with("https://images.gamebanana.com/")));
    }

    #[test]
    fn details_description_tags_and_counts_are_cleaned() {
        let d: Details = serde_json::from_value(serde_json::json!({
            "_idRow": 1, "_sName": "x", "_sProfileUrl": "u",
            "_sText": "<p>Oi &amp; tchau</p><p></p><p></p><br>Linha<script>x</script>",
            "_nLikeCount": 5, "_nViewCount": 9, "_nDownloadCount": 7, "_tsDateAdded": 100,
            "_aTags": [{"_sTitle": "Texture", "_sValue": "Custom Texture"}, {"_sTitle": "Model"}]
        })).unwrap();
        let text = d.description.unwrap();
        assert!(text.starts_with("Oi & tchau"), "{text:?}");
        assert!(!text.contains('<') && !text.contains("


"), "{text:?}");
        assert_eq!((d.like_count, d.view_count, d.download_count, d.date_added, d.date_modified), (5, 9, 7, Some(100), None));
        assert_eq!(d.tags, ["Custom Texture", "Model"]);
        let none: Details = serde_json::from_value(serde_json::json!({"_idRow": 1, "_sName": "x", "_sProfileUrl": "u", "_sText": "  <p> </p> "})).unwrap();
        assert!(none.description.is_none());
    }

    /// Servidor local que responde UMA requisição com `body`. Devolve o prefixo `/dl/` dele.
    fn serve_once(body: Vec<u8>) -> String {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        std::thread::spawn(move || {
            if let Ok(Some(req)) = server.recv_timeout(std::time::Duration::from_secs(10)) {
                let _ = req.respond(tiny_http::Response::from_data(body));
            }
        });
        format!("http://127.0.0.1:{port}/dl/")
    }

    fn file_info(url: String, size: u64, md5: Option<String>) -> FileInfo {
        FileInfo { id: 1, file_name: "x.zip".into(), size, download_url: url, md5, av_result: None, analysis_warnings: None }
    }

    fn tmp_file(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("suny_dl_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("staging").join("x.zip")
    }

    #[tokio::test]
    async fn download_to_saves_the_file_and_checks_md5() {
        let body = b"conteudo do mod".to_vec();
        let prefix = serve_once(body.clone());
        let md5 = format!("{:x}", Md5::digest(&body));
        let dest = tmp_file("ok");
        let c = Client::new("test").unwrap().with_download_prefix(prefix.clone());
        let n = c.download_to(&file_info(format!("{prefix}1"), body.len() as u64, Some(md5)), &dest, 1024).await.unwrap();
        assert_eq!(n, body.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        let _ = std::fs::remove_dir_all(dest.parent().unwrap().parent().unwrap());
    }

    /// Servidor "ruim": a 1ª conexão manda metade do arquivo e cai; a 2ª entende `Range` e manda o resto.
    fn serve_dropping_then_resuming(body: Vec<u8>, honor_range: bool) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            for (i, conn) in listener.incoming().take(3).enumerate() {
                let Ok(mut s) = conn else { return };
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let range_from = req.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("range: bytes=").and_then(|r| r.trim_end_matches('-').trim().parse::<usize>().ok()));
                seen2.lock().unwrap().push(format!("{i}:{:?}", range_from));
                let half = body.len() / 2;
                if i == 0 {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = s.write_all(&body[..half]);
                    let _ = s.flush(); // fecha no meio: o cliente vê a conexão cair
                } else if let (true, Some(from)) = (honor_range, range_from) {
                    let rest = &body[from..];
                    let _ = write!(s, "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n", rest.len(), from, body.len() - 1, body.len());
                    let _ = s.write_all(rest);
                } else {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = s.write_all(&body);
                }
            }
        });
        (format!("http://127.0.0.1:{port}/"), seen)
    }

    #[tokio::test]
    async fn download_resumes_after_the_connection_drops_midway() {
        let body: Vec<u8> = (0..4000u32).map(|i| (i % 251) as u8).collect();
        let md5 = format!("{:x}", Md5::digest(&body));
        let (prefix, seen) = serve_dropping_then_resuming(body.clone(), true);
        let dest = tmp_file("resume");
        let c = Client::new("test").unwrap().with_download_prefix(prefix.clone()).with_retry_delay(Duration::from_millis(5));
        let n = c.download_to(&file_info(format!("{prefix}1"), body.len() as u64, Some(md5)), &dest, 1 << 20).await.unwrap();
        assert_eq!(n, body.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), body, "o arquivo final é idêntico (hash conferido)");
        let log = seen.lock().unwrap().clone();
        assert_eq!(log[0], "0:None");
        assert_eq!(log[1], "1:Some(2000)", "a 2ª tentativa pede só o que faltou: {log:?}");
        let _ = std::fs::remove_dir_all(dest.parent().unwrap().parent().unwrap());
    }

    #[tokio::test]
    async fn download_restarts_from_zero_when_the_server_ignores_range() {
        let body: Vec<u8> = (0..3000u32).map(|i| (i % 241) as u8).collect();
        let md5 = format!("{:x}", Md5::digest(&body));
        let (prefix, _seen) = serve_dropping_then_resuming(body.clone(), false);
        let dest = tmp_file("norange");
        let c = Client::new("test").unwrap().with_download_prefix(prefix.clone()).with_retry_delay(Duration::from_millis(5));
        c.download_to(&file_info(format!("{prefix}1"), body.len() as u64, Some(md5)), &dest, 1 << 20).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        let _ = std::fs::remove_dir_all(dest.parent().unwrap().parent().unwrap());
    }

    #[tokio::test]
    async fn download_to_deletes_the_file_when_md5_is_wrong() {
        let prefix = serve_once(b"outra coisa".to_vec());
        let dest = tmp_file("md5");
        let c = Client::new("test").unwrap().with_download_prefix(prefix.clone());
        let err = c
            .download_to(&file_info(format!("{prefix}1"), 11, Some("00000000000000000000000000000000".into())), &dest, 1024)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::ChecksumMismatch { .. }), "{err}");
        assert!(!dest.exists(), "arquivo parcial precisa ser apagado");
        let _ = std::fs::remove_dir_all(dest.parent().unwrap().parent().unwrap());
    }

    #[tokio::test]
    async fn download_to_stops_when_the_real_size_passes_the_limit() {
        // o servidor mente: declara 3 bytes (no FileInfo) mas manda 100
        let prefix = serve_once(vec![1u8; 100]);
        let dest = tmp_file("big");
        let c = Client::new("test").unwrap().with_download_prefix(prefix.clone());
        let err = c.download_to(&file_info(format!("{prefix}1"), 3, None), &dest, 10).await.unwrap_err();
        assert!(matches!(err, Error::TooLarge(_)), "{err}");
        assert!(!dest.exists());

        // e o tamanho declarado acima do limite nem começa a baixar
        let err = c.download_to(&file_info(format!("{prefix}1"), 999, None), &dest, 10).await.unwrap_err();
        assert!(matches!(err, Error::TooLarge(_)), "{err}");
        let _ = std::fs::remove_dir_all(dest.parent().unwrap().parent().unwrap());
    }

    #[tokio::test]
    async fn download_to_refuses_urls_outside_gamebanana() {
        let dest = tmp_file("host");
        let c = Client::new("test").unwrap();
        for url in ["https://evil.example/dl/1", "http://gamebanana.com/dl/1", "https://gamebanana.com.evil.example/dl/1", "file:///C:/Windows/win.ini"] {
            let err = c.download_to(&file_info(url.into(), 1, None), &dest, 1024).await.unwrap_err();
            assert!(matches!(err, Error::Blocked(_)), "{url}: {err}");
        }
        assert!(!dest.exists());
    }

    #[test]
    fn categories_url_for_children_drops_the_game_id() {
        let c = Client::new("test").unwrap();
        let root = c.categories_url(Kind::Mod, None);
        assert!(root.contains("_idGameRow=297") && !root.contains("_idCategoryRow"), "{root}");
        let kids = c.categories_url(Kind::Mod, Some(7951));
        assert!(kids.contains("_idCategoryRow=7951"), "{kids}");
        assert!(!kids.contains("_idGameRow"), "com _idGameRow a API devolve só a raiz: {kids}");
    }

    /// Servidor local que responde as requisições em ordem: (status, corpo). Depois da última, encerra.
    fn serve_sequence(replies: Vec<(u16, &'static str)>) -> String {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        std::thread::spawn(move || {
            for (status, body) in replies {
                match server.recv_timeout(std::time::Duration::from_secs(10)) {
                    Ok(Some(req)) => {
                        let _ = req.respond(tiny_http::Response::from_string(body).with_status_code(status));
                    }
                    _ => return,
                }
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    const OK_BODY: &str = r#"{"_aMetadata":{"_nRecordCount":1,"_bIsComplete":true,"_nPerpage":15},"_aRecords":[{"_idRow":1,"_sName":"x","_sProfileUrl":"u"}]}"#;

    #[tokio::test]
    async fn an_empty_reply_is_retried_and_then_succeeds() {
        let base = serve_sequence(vec![(200, ""), (200, "   \n"), (200, OK_BODY)]);
        let c = Client::new("test").unwrap().with_base(base).with_retry_delay(Duration::from_millis(5));
        let page = c.list(Kind::Mod, Sort::MostLiked, &Query::default()).await.expect("deve recuperar na 3ª tentativa");
        assert_eq!(page.total, 1);
        assert_eq!(page.per_page, 15);
        assert_eq!(page.records[0].name, "x");
    }

    #[tokio::test]
    async fn server_errors_are_retried_but_client_errors_are_not() {
        let base = serve_sequence(vec![(503, "x"), (200, OK_BODY)]);
        let c = Client::new("test").unwrap().with_base(base).with_retry_delay(Duration::from_millis(5));
        assert!(c.list(Kind::Mod, Sort::MostLiked, &Query::default()).await.is_ok());

        // 404 não repete: só uma resposta no servidor, e o erro vem na hora
        let base = serve_sequence(vec![(404, "nada")]);
        let c = Client::new("test").unwrap().with_base(base).with_retry_delay(Duration::from_millis(5));
        assert!(matches!(c.list(Kind::Mod, Sort::MostLiked, &Query::default()).await, Err(Error::Network(_))));
    }

    #[tokio::test]
    async fn always_empty_gives_a_clear_error_instead_of_a_parse_failure() {
        let base = serve_sequence(vec![(200, ""), (200, ""), (200, "")]);
        let c = Client::new("test").unwrap().with_base(base).with_retry_delay(Duration::from_millis(5));
        let err = c.list(Kind::Mod, Sort::MostLiked, &Query::default()).await.unwrap_err();
        assert!(matches!(err, Error::EmptyResponse), "{err}");
        assert!(err.to_string().contains("resposta vazia"));
    }

    #[test]
    fn md5_is_checked() {
        // MD5 de "abc"
        assert!(verify_md5(b"abc", "900150983cd24fb0d6963f7d28e17f72").is_ok());
        assert!(matches!(verify_md5(b"abd", "900150983cd24fb0d6963f7d28e17f72"), Err(Error::ChecksumMismatch { .. })));
    }

    #[test]
    fn list_url_caps_per_page_and_encodes_search() {
        let c = Client::new("test").unwrap();
        let q = Query { page: 0, per_page: 500, category: Some(1090), search: None };
        let u = c.list_url(Kind::Mod, Sort::MostDownloaded, &q);
        assert!(u.contains("_nPerpage=50"));
        assert!(u.contains("_nPage=1"));
        assert!(u.contains("Generic_Category%5D=1090"));

        let q = Query { search: Some("hit sound".into()), ..Default::default() };
        let u = c.list_url(Kind::Sound, Sort::Newest, &q);
        assert!(u.contains("_sSearchString=hit%20sound"));
        assert!(u.contains("_sModelName=Sound"));
    }

    /// Teste contra a API real. Rode com `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "usa a internet"]
    async fn live_tf2_has_mods_and_sounds() {
        let c = Client::new("test").unwrap();
        let q = Query { per_page: 2, ..Default::default() };
        let mods = c.list(Kind::Mod, Sort::MostDownloaded, &q).await.unwrap();
        assert!(mods.total > 1000);
        let sounds = c.list(Kind::Sound, Sort::MostDownloaded, &q).await.unwrap();
        assert!(sounds.total > 1000);
        // Todo critério de ordenação que o app oferece precisa ser aceito pela API.
        for sort in [Sort::MostDownloaded, Sort::Newest, Sort::MostLiked] {
            c.list(Kind::Mod, sort, &q).await.unwrap_or_else(|e| panic!("{sort:?} recusado: {e}"));
        }
        let cats = c.categories(Kind::Sound, None).await.unwrap();
        assert!(cats.iter().any(|c| c.name == "Hitsound"));
        // subcategorias: Skins (7951) tem as classes, e "The Sniper" (7966) tem as armas
        let skins = c.categories(Kind::Mod, Some(7951)).await.unwrap();
        assert!(skins.iter().any(|c| c.name == "The Sniper"), "classes dentro de Skins");
        let sniper = c.categories(Kind::Mod, Some(7966)).await.unwrap();
        assert!(sniper.iter().any(|c| c.name == "Sniper Rifle"), "armas dentro da classe");
    }
}
