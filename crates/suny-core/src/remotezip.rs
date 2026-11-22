//! Prévia de áudio SEM baixar o pacote inteiro.
//!
//! Um `.zip` guarda o índice no FIM do arquivo. Pedimos só esse final (Range) e, depois de escolher o primeiro
//! áudio tocável, pedimos só os bytes dele. Um pacote de 18 MB vira uns 300 KB na rede, o que importa em conexão
//! lenta. Se o servidor não aceitar Range, o índice for grande demais ou o arquivo não for zip, quem chama cai no
//! caminho de sempre (baixar tudo).

use std::io::{self, Read, Seek, SeekFrom};

use crate::gamebanana::{Client, FileInfo, RangeSpec};
use crate::scanner;
use crate::{Error, Result};

/// Quanto do FIM do arquivo pedimos para achar o índice (cabe um índice de milhares de entradas).
const TAIL_BYTES: u64 = 512 * 1024;
/// Folga depois do cabeçalho local de uma entrada (nome e campos extras variam).
const LOCAL_HEADER_SLACK: u64 = 4096;

/// "Arquivo" montado só com os trechos baixados. Ler fora deles dá erro (nunca devolve zeros inventados).
pub struct Sparse {
    len: u64,
    chunks: Vec<(u64, Vec<u8>)>,
    pos: u64,
}

impl Sparse {
    pub fn new(len: u64) -> Self {
        Self {
            len,
            chunks: Vec::new(),
            pos: 0,
        }
    }
    pub fn add(&mut self, start: u64, data: Vec<u8>) {
        self.chunks.push((start, data));
    }
}

impl Read for Sparse {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.pos >= self.len {
            return Ok(0);
        }
        let pos = self.pos;
        let Some((start, data)) = self
            .chunks
            .iter()
            .find(|(s, d)| pos >= *s && pos < *s + d.len() as u64)
        else {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "trecho do arquivo não baixado",
            ));
        };
        let off = (pos - *start) as usize;
        let n = buf.len().min(data.len() - off);
        buf[..n].copy_from_slice(&data[off..off + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for Sparse {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let new = match from {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::End(d) => self.len as i128 + d as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
        };
        if new < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "posição negativa",
            ));
        }
        self.pos = new as u64;
        Ok(self.pos)
    }
}

#[derive(Debug)]
pub struct Audio {
    /// Só o nome do arquivo de áudio.
    pub name: String,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    /// Quantos áudios o pacote tem no total.
    pub total: usize,
}

fn mime_of(name: &str) -> Option<&'static str> {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "wav" => Some("audio/wav"),
        "mp3" => Some("audio/mpeg"),
        "ogg" => Some("audio/ogg"),
        _ => None,
    }
}

/// Uma entrada do índice central do zip.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub comp: u64,
    pub header_start: u64,
    pub is_dir: bool,
}

fn le16(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn le32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]))
}

/// Lê o índice central a partir do FIM do arquivo (`tail`, que começa em `tail_start`). Não usa nada além do índice.
/// Zip64 e índices fora do trecho baixado dão erro: quem chama cai no caminho de baixar tudo.
pub fn parse_central(tail: &[u8], tail_start: u64) -> Result<Vec<Entry>> {
    let bad = |m: &str| Error::Archive(format!("índice do zip: {m}"));
    // fim do índice (EOCD): procura a assinatura de trás para frente
    let lo = tail.len().saturating_sub(22 + 65_535);
    let eocd = (lo..=tail.len().saturating_sub(22))
        .rev()
        .find(|&i| le32(tail, i) == Some(0x0605_4b50))
        .ok_or_else(|| bad("fim do índice não encontrado"))?;
    let entries = le16(tail, eocd + 10).ok_or_else(|| bad("cortado"))? as usize;
    let cd_size = le32(tail, eocd + 12).ok_or_else(|| bad("cortado"))?;
    let cd_off = le32(tail, eocd + 16).ok_or_else(|| bad("cortado"))?;
    if entries == 0xFFFF || cd_size == u32::MAX || cd_off == u32::MAX {
        return Err(bad("zip64 não é lido no modo rápido"));
    }
    if (cd_off as u64) < tail_start {
        return Err(bad("o índice é maior que o trecho final baixado"));
    }
    let start = (cd_off as u64 - tail_start) as usize;
    let cd = tail.get(start..start + cd_size as usize).ok_or_else(|| bad("índice incompleto"))?;

    let mut out = Vec::with_capacity(entries);
    let mut o = 0usize;
    for _ in 0..entries {
        if le32(cd, o) != Some(0x0201_4b50) {
            return Err(bad("entrada inválida"));
        }
        let flags = le16(cd, o + 8).ok_or_else(|| bad("cortado"))?;
        let comp = le32(cd, o + 20).ok_or_else(|| bad("cortado"))?;
        let size = le32(cd, o + 24).ok_or_else(|| bad("cortado"))?;
        let nlen = le16(cd, o + 28).ok_or_else(|| bad("cortado"))? as usize;
        let elen = le16(cd, o + 30).ok_or_else(|| bad("cortado"))? as usize;
        let clen = le16(cd, o + 32).ok_or_else(|| bad("cortado"))? as usize;
        let off = le32(cd, o + 42).ok_or_else(|| bad("cortado"))?;
        if comp == u32::MAX || size == u32::MAX || off == u32::MAX {
            return Err(bad("zip64 não é lido no modo rápido"));
        }
        let raw = cd.get(o + 46..o + 46 + nlen).ok_or_else(|| bad("nome cortado"))?;
        // bit 11 = UTF-8; sem ele o nome está em CP437, e a decodificação perdida só afeta a exibição
        let name = if flags & 0x0800 != 0 { String::from_utf8_lossy(raw).into_owned() } else { raw.iter().map(|&b| b as char).collect() };
        let is_dir = name.ends_with('/') || name.ends_with('\\');
        out.push(Entry { name, size: size as u64, comp: comp as u64, header_start: off as u64, is_dir });
        o += 46 + nlen + elen + clen;
    }
    Ok(out)
}

/// Caminho que um zip malicioso poderia usar para sair da pasta: nem chegamos a tocar nesses.
fn suspicious(name: &str) -> bool {
    let p = name.replace('\\', "/");
    p.starts_with('/') || p.split('/').any(|s| s == "..") || p.chars().nth(1) == Some(':')
}

/// Escolhe o áudio e lê o conteúdo a partir de trechos já baixados (`fetch` pede um intervalo `[start, end]` inclusive).
/// Separado da rede para dar para testar sem servidor. `Ok(None)`: o pacote não tem áudio que caiba na prévia.
pub async fn pick_audio<F, Fut>(
    total: u64,
    tail_start: u64,
    tail: Vec<u8>,
    max_entry: u64,
    mut fetch: F,
) -> Result<Option<Audio>>
where
    F: FnMut(u64, u64) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>>>,
{
    let entries = parse_central(&tail, tail_start)?;
    let mut sparse = Sparse::new(total);
    sparse.add(tail_start, tail);
    let mut zip = zip::ZipArchive::new(sparse)
        .map_err(|e| Error::Archive(format!("índice do zip fora do trecho final: {e}")))?;

    let mut audio_total = 0usize;
    let mut chosen: Option<(usize, String, u64, u64, u64)> = None; // índice, nome, tamanho, comprimido, início do cabeçalho local
    for (i, e) in entries.iter().enumerate() {
        let name = &e.name;
        if e.is_dir || mime_of(name).is_none() || suspicious(name) {
            continue;
        }
        // mesmo critério da análise: só áudio que o jogo carrega (sound/...)
        if !scanner::is_game_path(&scanner::game_relative(name)) {
            continue;
        }
        audio_total += 1;
        if chosen.is_none() && e.size <= max_entry {
            chosen = Some((i, name.clone(), e.size, e.comp, e.header_start));
        }
    }
    let Some((index, name, size, comp, header_start)) = chosen else {
        return Ok(None);
    };
    if size > max_entry || comp > max_entry.saturating_mul(2) {
        return Ok(None);
    }

    // Cabeçalho local + dados da entrada escolhida.
    let end = (header_start + 30 + name.len() as u64 + LOCAL_HEADER_SLACK + comp)
        .min(total.saturating_sub(1));
    let data = fetch(header_start, end).await?;
    let mut sparse = zip.into_inner();
    sparse.add(header_start, data);
    let mut zip = zip::ZipArchive::new(sparse).map_err(|e| Error::Archive(e.to_string()))?;
    let mut f = zip
        .by_index(index)
        .map_err(|e| Error::Archive(format!("não deu para abrir o áudio: {e}")))?;
    let mut bytes = Vec::new();
    (&mut f)
        .take(max_entry + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Archive(format!("não deu para ler o áudio: {e}")))?;
    if bytes.len() as u64 > max_entry {
        return Ok(None);
    }
    let short = name.rsplit(['/', '\\']).next().unwrap_or(&name).to_string();
    let mime = mime_of(&name).unwrap_or("audio/wav");
    Ok(Some(Audio {
        name: short,
        mime,
        bytes,
        total: audio_total,
    }))
}

/// Prévia de áudio de um arquivo `.zip` do GameBanana lendo só o necessário (Range).
pub async fn preview_audio(
    client: &Client,
    file: &FileInfo,
    max_entry: u64,
) -> Result<Option<Audio>> {
    let first = client
        .get_range(file, RangeSpec::Suffix(TAIL_BYTES))
        .await?;
    let total = first
        .total
        .ok_or_else(|| Error::Network("o servidor não informou o tamanho do arquivo".into()))?;
    if first.start + first.bytes.len() as u64 > total {
        return Err(Error::Network("resposta de Range incoerente".into()));
    }
    pick_audio(
        total,
        first.start,
        first.bytes,
        max_entry,
        |start, end| async move {
            let r = client.get_range(file, RangeSpec::Span(start, end)).await?;
            if r.start != start {
                return Err(Error::Network("o servidor devolveu outro trecho".into()));
            }
            Ok(r.bytes)
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(entries: &[(&str, Vec<u8>)], deflate: bool) -> Vec<u8> {
        let mut buf = io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let method = if deflate {
                zip::CompressionMethod::Deflated
            } else {
                zip::CompressionMethod::Stored
            };
            let o = zip::write::SimpleFileOptions::default().compression_method(method);
            for (n, b) in entries {
                w.start_file(*n, o).unwrap();
                w.write_all(b).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    /// "Rede" de mentira: devolve o intervalo pedido de um zip em memória e conta os bytes que passaram.
    fn fake_net(
        zip: Vec<u8>,
    ) -> (
        u64,
        impl FnMut(u64, u64) -> std::future::Ready<Result<Vec<u8>>>,
        std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) {
        let sent = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let s2 = sent.clone();
        let total = zip.len() as u64;
        (
            total,
            move |a: u64, b: u64| {
                let part = zip[a as usize..=(b as usize).min(zip.len() - 1)].to_vec();
                s2.fetch_add(part.len() as u64, std::sync::atomic::Ordering::Relaxed);
                std::future::ready(Ok(part))
            },
            sent,
        )
    }

    #[tokio::test]
    async fn plays_the_first_audio_reading_only_the_index_and_that_entry() {
        let big_junk = vec![7u8; 2 * 1024 * 1024]; // dado que NUNCA deve ser baixado
        let wav = b"RIFF....WAVEdata-of-the-sample".to_vec();
        for deflate in [false, true] {
            let zip = make_zip(
                &[
                    ("readme.txt", b"oi".to_vec()),
                    ("materials/junk.vtf", big_junk.clone()),
                    ("sound/ui/hitsound.wav", wav.clone()),
                    ("sound/ui/second.mp3", b"ID3xx".to_vec()),
                ],
                deflate,
            );
            let (total, fetch, sent) = fake_net(zip.clone());
            let tail_start = total.saturating_sub(4096);
            let tail = zip[tail_start as usize..].to_vec();
            sent.fetch_add(tail.len() as u64, std::sync::atomic::Ordering::Relaxed);
            let got = pick_audio(total, tail_start, tail, 12 << 20, fetch)
                .await
                .unwrap()
                .expect("tem áudio");
            assert_eq!(
                (got.name.as_str(), got.mime, got.total),
                ("hitsound.wav", "audio/wav", 2)
            );
            assert_eq!(got.bytes, wav, "conteúdo idêntico (deflate={deflate})");
            let n = sent.load(std::sync::atomic::Ordering::Relaxed);
            if !deflate {
                assert!(
                    n < 64 * 1024,
                    "só o índice e a amostra passaram pela rede, não os {} bytes do pacote: {n}",
                    total
                );
            }
        }
    }

    #[tokio::test]
    async fn packs_without_playable_audio_or_with_only_oversized_audio_give_none() {
        let zip = make_zip(
            &[
                ("materials/a.vtf", vec![1; 100]),
                ("readme.txt", b"x".to_vec()),
            ],
            false,
        );
        let (total, fetch, _) = fake_net(zip.clone());
        assert!(pick_audio(total, 0, zip.clone(), 12 << 20, fetch)
            .await
            .unwrap()
            .is_none());

        let zip = make_zip(&[("sound/big.wav", vec![0; 5000])], false);
        let (total, fetch, _) = fake_net(zip.clone());
        assert!(
            pick_audio(total, 0, zip, 1000, fetch)
                .await
                .unwrap()
                .is_none(),
            "maior que o limite da prévia"
        );
    }

    #[tokio::test]
    async fn suspicious_paths_and_non_game_paths_are_never_picked_and_a_cut_index_is_an_error() {
        let zip = make_zip(
            &[
                ("../evil/sound/x.wav", b"a".to_vec()),
                ("other/readme.wav", b"b".to_vec()),
            ],
            false,
        );
        let (total, fetch, _) = fake_net(zip.clone());
        assert!(pick_audio(total, 0, zip.clone(), 12 << 20, fetch)
            .await
            .unwrap()
            .is_none());

        // só o finalzinho (sem o índice inteiro): erro, para quem chamou cair no caminho antigo
        let (total, fetch, _) = fake_net(zip.clone());
        let tail = zip[zip.len() - 10..].to_vec();
        assert!(pick_audio(total, total - 10, tail, 12 << 20, fetch)
            .await
            .is_err());
    }

    /// Servidor local que entende `Range` (sufixo e intervalo) e conta quantos bytes mandou.
    fn serve_ranges(body: Vec<u8>) -> (String, std::sync::Arc<std::sync::atomic::AtomicU64>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let sent = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let s2 = sent.clone();
        std::thread::spawn(move || {
            while let Ok(Some(req)) = server.recv_timeout(std::time::Duration::from_secs(10)) {
                let range = req.headers().iter().find(|h| h.field.equiv("Range")).map(|h| h.value.as_str().to_string()).unwrap_or_default();
                let n = body.len() as u64;
                let spec = range.strip_prefix("bytes=").unwrap_or("");
                let (a, b) = if let Some(suffix) = spec.strip_prefix('-') {
                    let k: u64 = suffix.parse().unwrap_or(0);
                    (n.saturating_sub(k), n - 1)
                } else {
                    let (x, y) = spec.split_once('-').unwrap_or(("0", "0"));
                    (x.parse().unwrap_or(0), y.parse::<u64>().unwrap_or(n - 1).min(n - 1))
                };
                let part = body[a as usize..=b as usize].to_vec();
                s2.fetch_add(part.len() as u64, std::sync::atomic::Ordering::Relaxed);
                let cr = tiny_http::Header::from_bytes(&b"Content-Range"[..], format!("bytes {a}-{b}/{n}").as_bytes()).unwrap();
                let _ = req.respond(tiny_http::Response::from_data(part).with_status_code(206).with_header(cr));
            }
        });
        (format!("http://127.0.0.1:{port}/dl/"), sent)
    }

    #[tokio::test]
    async fn previews_a_big_pack_over_http_reading_a_fraction_of_it() {
        // 3 MB de "textura" que não deve ser baixada, e uma amostra de áudio de verdade (deflate)
        let wav: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
        let zip = make_zip(&[("materials/big.vtf", vec![9u8; 3 * 1024 * 1024]), ("sound/vo/line01.wav", wav.clone()), ("sound/vo/line02.wav", wav.clone())], true);
        let total = zip.len() as u64;
        let (prefix, sent) = serve_ranges(zip);
        let client = crate::gamebanana::Client::new("test").unwrap().with_download_prefix(prefix.clone());
        let file = crate::gamebanana::FileInfo { id: 1, file_name: "pack.zip".into(), size: total, download_url: format!("{prefix}1"), md5: None, av_result: None, analysis_warnings: None };
        let got = preview_audio(&client, &file, 12 << 20).await.unwrap().expect("áudio");
        assert_eq!((got.name.as_str(), got.total), ("line01.wav", 2));
        assert_eq!(got.bytes, wav);
        assert!(sent.load(std::sync::atomic::Ordering::Relaxed) < total / 2 + 100_000, "baixou {} de {total}", sent.load(std::sync::atomic::Ordering::Relaxed));
    }

    /// Teste contra o GameBanana de verdade: o pacote do Hank Hill (17,7 MB) tem que dar uma amostra lendo bem menos que isso.
    /// Rode: `cargo test -p suny-core live_preview -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "usa a internet"]
    async fn live_preview_of_a_big_zip_reads_only_a_fraction() {
        let c = crate::gamebanana::Client::new("test").unwrap();
        let d = c.details(crate::gamebanana::Kind::Sound, 56495).await.unwrap();
        let f = d.files.iter().find(|f| f.file_name.ends_with(".zip")).expect("zip");
        let t0 = std::time::Instant::now();
        let got = preview_audio(&c, f, 12 << 20).await.unwrap().expect("áudio");
        println!("amostra {} ({} bytes, {} áudios no pacote de {} bytes) em {:?}", got.name, got.bytes.len(), got.total, f.size, t0.elapsed());
        assert!(!got.bytes.is_empty());
    }

    #[tokio::test]
    async fn counts_every_audio_when_the_local_headers_were_not_downloaded() {
        // 300 áudios de 2 KB: o índice cabe no final, mas os cabeçalhos locais ficam longe dele
        let mut entries: Vec<(String, Vec<u8>)> = (0..300).map(|i| (format!("Pack/sound/vo/line_{i:03}.mp3"), vec![(i % 200) as u8; 2048])).collect();
        entries.push(("Pack/readme.txt".into(), b"oi".to_vec()));
        let refs: Vec<(&str, Vec<u8>)> = entries.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
        let zip = make_zip(&refs, false);
        let total = zip.len() as u64;
        let tail_start = total - 40_000; // só o final (cabe o índice); a maioria dos cabeçalhos locais está antes disso
        assert!(tail_start > 100_000);
        let tail = zip[tail_start as usize..].to_vec();
        let (_, fetch, _) = fake_net(zip.clone());
        let got = pick_audio(total, tail_start, tail, 12 << 20, fetch).await.unwrap().expect("áudio");
        assert_eq!((got.name.as_str(), got.total), ("line_000.mp3", 300), "conta todos, não só os do final");
        assert_eq!(got.bytes, vec![0u8; 2048]);
    }

    #[test]
    fn central_directory_parser_reads_names_sizes_and_offsets_and_refuses_what_it_cannot_trust() {
        let zip = make_zip(&[("a/b.txt", vec![1; 10]), ("sound/x.wav", vec![2; 20]), ("dir/", vec![])], false);
        let e = parse_central(&zip, 0).unwrap();
        assert_eq!(e.len(), 3);
        assert_eq!((e[1].name.as_str(), e[1].size, e[1].comp), ("sound/x.wav", 20, 20));
        assert!(e[2].is_dir && e[0].header_start == 0 && e[1].header_start > 0);
        // sem o índice no trecho: erro, em vez de adivinhar
        assert!(parse_central(&zip[zip.len() - 10..], zip.len() as u64 - 10).is_err());
        assert!(parse_central(b"nao e um zip", 0).is_err());
    }

    #[test]
    fn sparse_reads_only_what_was_added_and_never_invents_data() {
        let mut s = Sparse::new(100);
        s.add(10, vec![1, 2, 3, 4]);
        s.seek(SeekFrom::Start(11)).unwrap();
        let mut b = [0u8; 8];
        assert_eq!(s.read(&mut b).unwrap(), 3);
        assert_eq!(&b[..3], &[2, 3, 4]);
        assert_eq!(
            s.read(&mut b).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof,
            "depois do trecho não há dados"
        );
        s.seek(SeekFrom::Start(0)).unwrap();
        assert!(s.read(&mut b).is_err(), "antes do trecho não há dados");
        s.seek(SeekFrom::End(0)).unwrap();
        assert_eq!(s.read(&mut b).unwrap(), 0, "no fim lê 0");
        assert!(s.seek(SeekFrom::Current(-1000)).is_err());
    }
}
