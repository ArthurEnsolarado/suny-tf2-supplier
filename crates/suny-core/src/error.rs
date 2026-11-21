use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("não achei o Steam neste computador: {0}")]
    SteamNotFound(String),

    #[error("a pasta {0} não parece ser a do Team Fortress 2 (falta tf/gameinfo.txt)")]
    NotTf2Dir(PathBuf),

    #[error("erro de arquivo em {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("o GameBanana respondeu com erro ({code}): {message}")]
    GameBanana { code: String, message: String },

    #[error("falha de rede: {0}")]
    Network(String),

    #[error("resposta inesperada do GameBanana: {0}")]
    BadResponse(#[from] serde_json::Error),

    #[error("arquivo de mod inválido: {0}")]
    Archive(String),

    #[error("o MD5 do arquivo não bate (esperado {expected}, obtido {actual})")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("o app recusou este arquivo por segurança: {0}")]
    Blocked(String),

    #[error("não achei nada para instalar neste arquivo: ele não tem pastas do jogo (models, sound, materials, resource...) nem .vpk")]
    NothingToInstall,

    #[error("já existe '{0}' na pasta custom e não foi criado por este app. Nada foi alterado")]
    FolderTaken(String),

    #[error("formato não suportado: {0}. O app abre .zip, .rar e .7z")]
    UnsupportedFormat(String),

    #[error("para abrir arquivos {0} o app usa o 7-Zip, o WinRAR ou o tar do Windows, e nenhum foi encontrado neste computador. Instale o 7-Zip (gratuito, 7-zip.org) e tente de novo")]
    NoExtractor(String),

    #[error("o arquivo passa do limite de segurança de tamanho ({0})")]
    TooLarge(String),

    #[error("o Team Fortress 2 está aberto. Feche o jogo antes de instalar ou remover mods")]
    GameRunning,

    #[error("nome inválido: {0}")]
    BadName(String),

    #[error("o GameBanana devolveu uma resposta vazia. Tente de novo em instantes")]
    EmptyResponse,
}

/// Texto de um erro de rede com a causa de verdade. `reqwest` mostra só "error decoding response body"
/// para qualquer queda no meio do download; aqui vem junto o motivo (tempo esgotado, conexão reiniciada...).
pub fn net_err(e: &reqwest::Error) -> String {
    use std::error::Error as _;
    let mut text = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        let s = c.to_string();
        if !text.contains(&s) {
            text.push_str(": ");
            text.push_str(&s);
        }
        cause = c.source();
    }
    if e.is_timeout() {
        text.push_str(" (tempo esgotado: a rede está lenta ou bloqueando a conexão)");
    }
    text
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Network(net_err(&e))
    }
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io { path: path.into(), source }
    }
}
