//! Analisa um `.zip` de mod SEM extrair nada: lista o que ele muda, calcula a compatibilidade e deixa
//! ouvir os sons antes de instalar.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Serialize;

use crate::replaces::{self, Group};
use crate::scanner::{self, Report, VanillaIndex};
use crate::{Error, Result};

const AUDIO_EXT: &[&str] = &["wav", "mp3", "ogg"];
/// Limite do áudio que vai para a interface como prévia (vira texto base64 na memória).
pub const MAX_PREVIEW_BYTES: u64 = 12 * 1024 * 1024;
/// A lista de áudios da prévia mostra no máximo isto; o total real vem em `audio_total`.
const MAX_AUDIO_LISTED: usize = 80;

#[derive(Debug, Clone, Serialize)]
pub struct AudioItem {
    /// Nome exato da entrada no zip (é a chave para tocar).
    pub entry: String,
    /// Só o nome do arquivo, para exibir.
    pub name: String,
    pub size: u64,
    pub label: String,
    pub class: Option<String>,
    /// `Some(true)` = substitui um som do jogo; `Some(false)` = som novo; `None` = não deu para comparar.
    pub vanilla: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    /// Todos os arquivos do zip (sem pastas).
    pub file_count: usize,
    /// Os que caem em pastas do jogo (models, sound, materials...).
    pub game_files: usize,
    pub report: Report,
    pub groups: Vec<Group>,
    /// Classes que o mod afeta (chaves dos ícones), das mais mexidas para as menos.
    pub classes: Vec<String>,
    pub audio: Vec<AudioItem>,
    pub audio_total: usize,
    /// O índice dos arquivos originais do jogo estava disponível?
    pub vanilla_known: bool,
}

fn ext_of(name: &str) -> String {
    name.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}

pub fn analyze_zip(zip_path: &Path, vanilla: Option<&VanillaIndex>) -> Result<Analysis> {
    let file = File::open(zip_path).map_err(|e| Error::io(zip_path, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Archive(e.to_string()))?;

    let mut names: Vec<String> = Vec::with_capacity(zip.len());
    let mut sizes: Vec<u64> = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let e = zip.by_index_raw(i).map_err(|e| Error::Archive(e.to_string()))?;
        names.push(e.name().map_err(|e| Error::Archive(format!("nome inválido no zip: {e}")))?.into_owned());
        sizes.push(e.size());
    }

    let report = scanner::analyze(&names, vanilla);
    let known = vanilla.is_some_and(|v| !v.is_empty());

    let mut rels: Vec<String> = Vec::new();
    let mut audio: Vec<AudioItem> = Vec::new();
    let mut audio_total = 0usize;
    let mut file_count = 0usize;
    for (i, name) in names.iter().enumerate() {
        if name.ends_with('/') || name.ends_with('\\') {
            continue;
        }
        file_count += 1;
        if report.blocked.contains(name) {
            continue; // perigoso ou com caminho suspeito: não entra em nenhuma lista
        }
        let rel = scanner::game_relative(name);
        if !scanner::is_game_path(&rel) {
            continue;
        }
        if AUDIO_EXT.contains(&ext_of(&rel).as_str()) {
            audio_total += 1;
            if audio.len() < MAX_AUDIO_LISTED {
                let lab = replaces::label_for(&rel);
                audio.push(AudioItem {
                    entry: name.clone(),
                    name: rel.rsplit('/').next().unwrap_or(&rel).to_string(),
                    size: sizes[i],
                    label: lab.text,
                    class: lab.class.map(str::to_string),
                    vanilla: known.then(|| vanilla.is_some_and(|v| v.contains(&rel))),
                });
            }
        }
        rels.push(rel);
    }

    let groups = replaces::group(&rels, vanilla);
    let classes = replaces::classes_affected(&groups);
    Ok(Analysis { file_count, game_files: rels.len(), report, groups, classes, audio, audio_total, vanilla_known: known })
}

/// Lê um áudio de dentro do zip para a prévia. Só `.wav`, `.mp3` e `.ogg`, até `MAX_PREVIEW_BYTES`.
pub fn read_audio(zip_path: &Path, entry: &str) -> Result<(&'static str, Vec<u8>)> {
    read_audio_with_limit(zip_path, entry, MAX_PREVIEW_BYTES)
}

pub fn read_audio_with_limit(zip_path: &Path, entry: &str, limit: u64) -> Result<(&'static str, Vec<u8>)> {
    let mime = match ext_of(entry).as_str() {
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        _ => return Err(Error::Blocked("só dá para ouvir arquivos .wav, .mp3 e .ogg".into())),
    };
    let file = File::open(zip_path).map_err(|e| Error::io(zip_path, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Archive(e.to_string()))?;
    let f = zip.by_name(entry).map_err(|_| Error::Archive("esse arquivo não está no zip".into()))?;
    if f.size() > limit {
        return Err(Error::TooLarge(format!("o áudio tem {} MB; a prévia vai até {} MB", f.size() / 1024 / 1024, limit / 1024 / 1024)));
    }
    let mut buf = Vec::new();
    f.take(limit + 1).read_to_end(&mut buf).map_err(|e| Error::io(zip_path, e))?;
    if buf.len() as u64 > limit {
        return Err(Error::TooLarge("o áudio passou do limite durante a leitura".into()));
    }
    Ok((mime, buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::Compat;
    use std::io::Write;
    use std::path::PathBuf;

    fn make_zip(tag: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("suny_ana_{tag}_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let p = d.join("m.zip");
        let mut w = zip::ZipWriter::new(File::create(&p).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        for (n, b) in entries {
            w.start_file(*n, o).unwrap();
            w.write_all(b).unwrap();
        }
        w.finish().unwrap();
        p
    }

    fn vanilla() -> VanillaIndex {
        VanillaIndex::from_paths(["sound/ui/hitsound.wav", "sound/vo/demoman_battlecry01.mp3"])
    }

    #[test]
    fn explains_what_a_sound_pack_changes_using_the_game_index() {
        let z = make_zip(
            "full",
            &[
                ("Pack/Sound/UI/HitSound.wav", b"RIFF1"),
                ("Pack/sound/vo/demoman_battlecry01.mp3", b"ID3a"),
                ("Pack/sound/vo/demoman_novo.mp3", b"ID3b"),
                ("Pack/leia-me.txt", b"oi"),
            ],
        );
        let a = analyze_zip(&z, Some(&vanilla())).unwrap();
        assert_eq!(a.file_count, 4);
        assert_eq!(a.game_files, 3, "o leia-me não conta");
        assert!(a.vanilla_known);
        assert_eq!(a.classes, vec!["demoman".to_string()]);
        let voz = a.groups.iter().find(|g| g.label == "Voz do Demoman").unwrap();
        assert_eq!((voz.total, voz.replaces, voz.added), (2, Some(1), Some(1)));
        assert_eq!(a.audio_total, 3);
        let hit = a.audio.iter().find(|x| x.name == "hitsound.wav").unwrap();
        assert_eq!((hit.label.as_str(), hit.vanilla, hit.class.as_deref()), ("Hitsound", Some(true), None));
        let novo = a.audio.iter().find(|x| x.name == "demoman_novo.mp3").unwrap();
        assert_eq!((novo.vanilla, novo.class.as_deref()), (Some(false), Some("demoman")));
        assert_eq!(a.report.compat, Compat::Partial, "um som novo e dois que existem");
        assert_eq!(a.audio.iter().find(|x| x.name == "hitsound.wav").unwrap().entry, "Pack/Sound/UI/HitSound.wav", "a chave para tocar é o nome original no zip");
    }

    #[test]
    fn without_the_game_index_it_does_not_invent_numbers() {
        let z = make_zip("noidx", &[("sound/ui/hitsound.wav", b"RIFF")]);
        let a = analyze_zip(&z, None).unwrap();
        assert!(!a.vanilla_known);
        assert!(a.audio[0].vanilla.is_none());
        assert!(a.groups[0].replaces.is_none());
        assert_eq!(a.report.compat, Compat::NeedsIndex);
    }

    #[test]
    fn dangerous_files_are_flagged_and_left_out_of_every_list() {
        let z = make_zip("exe", &[("sound/ui/hitsound.wav", b"RIFF"), ("setup.exe", b"MZ"), ("../../x/sound/ui/killsound.wav", b"RIFF")]);
        let a = analyze_zip(&z, Some(&vanilla())).unwrap();
        assert_eq!(a.report.compat, Compat::Blocked);
        assert_eq!(a.audio.len(), 1, "só o hitsound normal; o caminho suspeito não vira prévia");
        assert!(a.audio.iter().all(|x| !x.entry.contains("..")));
    }

    #[test]
    fn the_audio_list_is_capped_but_the_total_is_real() {
        let names: Vec<String> = (0..100).map(|i| format!("sound/vo/demoman_{i:03}.wav")).collect();
        let entries: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b"RIFF"[..])).collect();
        let a = analyze_zip(&make_zip("cap", &entries), None).unwrap();
        assert_eq!(a.audio.len(), 80);
        assert_eq!(a.audio_total, 100);
        assert_eq!(a.groups[0].total, 100);
    }

    #[test]
    fn audio_preview_reads_only_audio_inside_the_zip_within_the_limit() {
        let z = make_zip("prev", &[("sound/ui/hitsound.wav", b"RIFFdata"), ("sound/vo/a.mp3", b"ID3"), ("readme.txt", b"oi"), ("sound/big.ogg", &[7u8; 50])]);
        let (mime, bytes) = read_audio(&z, "sound/ui/hitsound.wav").unwrap();
        assert_eq!((mime, bytes.as_slice()), ("audio/wav", &b"RIFFdata"[..]));
        assert_eq!(read_audio(&z, "sound/vo/a.mp3").unwrap().0, "audio/mpeg");

        assert!(matches!(read_audio(&z, "readme.txt"), Err(Error::Blocked(_))), "texto não é áudio");
        assert!(matches!(read_audio(&z, "sound/nao/existe.wav"), Err(Error::Archive(_))));
        assert!(matches!(read_audio_with_limit(&z, "sound/big.ogg", 10), Err(Error::TooLarge(_))));
        // nome de entrada nunca vira caminho de disco: só procura dentro do zip
        assert!(read_audio(&z, "../../Windows/win.wav").is_err());
        assert!(read_audio(&z, "C:/Windows/media/tada.wav").is_err());
    }

    #[test]
    fn a_broken_zip_is_an_error_not_a_panic() {
        let d = std::env::temp_dir().join(format!("suny_ana_bad_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("x.zip");
        std::fs::write(&p, b"isto nao e um zip").unwrap();
        assert!(matches!(analyze_zip(&p, None), Err(Error::Archive(_))));
        assert!(read_audio(&p, "a.wav").is_err());
        assert!(analyze_zip(&d.join("nao_existe.zip"), None).is_err());
    }
}
