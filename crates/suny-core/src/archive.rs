//! Abre `.rar` e `.7z` (e converte para `.zip`) usando um programa que já existe no computador.
//!
//! O app não carrega um descompactador de RAR dentro dele: a licença do código do RAR (UnRAR) não é compatível
//! com a GPL do projeto, e o formato é fechado. Em vez disso usa, nesta ordem, o que estiver instalado:
//! **7-Zip**, **WinRAR (UnRAR.exe)** e o **`tar` do Windows** (bsdtar, vem no Windows 10/11).
//! O arquivo é extraído numa pasta temporária, conferido (tamanho, quantidade, links) e vira um `.zip`,
//! que segue o mesmo caminho de segurança de qualquer outro mod.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zip,
    Rar,
    SevenZ,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Zip => ".zip",
            Format::Rar => ".rar",
            Format::SevenZ => ".7z",
        }
    }
}

/// Formato pelo nome do arquivo (só a extensão decide; o conteúdo é conferido ao extrair).
pub fn format_of(file_name: &str) -> Option<Format> {
    match file_name.rsplit_once('.')?.1.to_ascii_lowercase().as_str() {
        "zip" => Some(Format::Zip),
        "rar" => Some(Format::Rar),
        "7z" => Some(Format::SevenZ),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Extractor {
    SevenZip(PathBuf),
    UnRar(PathBuf),
    Tar(PathBuf),
}

impl Extractor {
    /// Identificador estável para a interface traduzir o nome (`7zip`, `winrar`, `tar`).
    pub fn id(&self) -> &'static str {
        match self {
            Extractor::SevenZip(_) => "7zip",
            Extractor::UnRar(_) => "winrar",
            Extractor::Tar(_) => "tar",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Extractor::SevenZip(_) => "7-Zip",
            Extractor::UnRar(_) => "WinRAR",
            Extractor::Tar(_) => "tar do Windows",
        }
    }
}

fn env_dirs(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn first_file(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| p.is_file())
}

fn find_in_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for n in names {
            let p = dir.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn program_dirs() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"].iter().filter_map(|v| env_dirs(v)).collect()
}

fn find_7z() -> Option<PathBuf> {
    first_file(program_dirs().into_iter().map(|d| d.join("7-Zip").join("7z.exe")).collect()).or_else(|| find_in_path(&["7z.exe", "7zz", "7z"]))
}

fn find_unrar() -> Option<PathBuf> {
    first_file(program_dirs().into_iter().map(|d| d.join("WinRAR").join("UnRAR.exe")).collect()).or_else(|| find_in_path(&["UnRAR.exe", "unrar"]))
}

fn find_tar() -> Option<PathBuf> {
    let sys = env_dirs("SystemRoot").map(|r| r.join("System32").join("tar.exe"));
    sys.filter(|p| p.is_file()).or_else(|| find_in_path(&["bsdtar.exe", "bsdtar", "tar.exe"]))
}

/// Programas disponíveis para este formato, do melhor para o pior.
pub fn extractors_for(fmt: Format) -> Vec<Extractor> {
    let mut v = Vec::new();
    match fmt {
        Format::Zip => {}
        Format::Rar => {
            v.extend(find_7z().map(Extractor::SevenZip));
            v.extend(find_unrar().map(Extractor::UnRar));
            v.extend(find_tar().map(Extractor::Tar));
        }
        Format::SevenZ => {
            v.extend(find_7z().map(Extractor::SevenZip));
            v.extend(find_tar().map(Extractor::Tar));
        }
    }
    v
}

/// Limites de segurança ao extrair.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_bytes: u64,
    pub max_files: u64,
    pub timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_bytes: 1 << 30, max_files: 30_000, timeout: Duration::from_secs(300) }
    }
}

/// Tamanho e quantidade de arquivos de uma pasta, sem seguir links. `Err` se achar link simbólico.
fn tree_stats(dir: &Path) -> std::result::Result<(u64, u64), String> {
    let mut stack = vec![dir.to_path_buf()];
    let (mut bytes, mut files) = (0u64, 0u64);
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(meta) = std::fs::symlink_metadata(e.path()) else { continue };
            let ft = meta.file_type();
            if ft.is_symlink() {
                return Err(format!("o arquivo tem um link simbólico ({}), recusado", e.path().strip_prefix(dir).unwrap_or(&e.path()).display()));
            }
            if ft.is_dir() {
                stack.push(e.path());
            } else if ft.is_file() {
                bytes += meta.len();
                files += 1;
            } else {
                return Err("o arquivo tem um item especial (não é arquivo nem pasta), recusado".into());
            }
        }
    }
    Ok((bytes, files))
}

fn command_for(ex: &Extractor, src: &Path, out: &Path) -> Command {
    let mut cmd;
    match ex {
        Extractor::SevenZip(exe) => {
            cmd = Command::new(exe);
            cmd.arg("x").arg("-y").arg("-bso0").arg("-bsp0").arg(format!("-o{}", out.display())).arg("--").arg(src);
        }
        Extractor::UnRar(exe) => {
            cmd = Command::new(exe);
            let mut dest = out.as_os_str().to_os_string();
            dest.push(std::path::MAIN_SEPARATOR_STR);
            cmd.arg("x").arg("-y").arg("-idq").arg("--").arg(src).arg(dest);
        }
        Extractor::Tar(exe) => {
            cmd = Command::new(exe);
            cmd.arg("-xf").arg(src).arg("-C").arg(out).arg("--no-same-owner").arg("--no-same-permissions");
        }
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // sem janela de console
    }
    cmd
}

/// Roda o extrator vigiando a pasta: passou do limite de tamanho/quantidade ou do tempo, ele é encerrado na hora.
fn run_limited(ex: &Extractor, src: &Path, out: &Path, limits: &Limits) -> Result<()> {
    let mut child = command_for(ex, src, out).spawn().map_err(|e| Error::Archive(format!("não consegui iniciar o {}: {e}", ex.name())))?;
    let mut stderr = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr.as_mut() {
            let mut buf = Vec::new();
            let _ = e.take(8192).read_to_end(&mut buf);
            s = String::from_utf8_lossy(&buf).trim().to_string();
        }
        s
    });

    let start = Instant::now();
    let mut last_check = Instant::now();
    let kill = |child: &mut std::process::Child| {
        let _ = child.kill();
        let _ = child.wait();
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => {
                kill(&mut child);
                return Err(Error::Archive(e.to_string()));
            }
        }
        if start.elapsed() > limits.timeout {
            kill(&mut child);
            return Err(Error::TooLarge(format!("a extração passou de {} s", limits.timeout.as_secs())));
        }
        if last_check.elapsed() >= Duration::from_millis(300) {
            last_check = Instant::now();
            match tree_stats(out) {
                Ok((bytes, files)) if bytes > limits.max_bytes || files > limits.max_files => {
                    kill(&mut child);
                    return Err(Error::TooLarge(format!("{} MB e {files} arquivos descompactados", bytes >> 20)));
                }
                Err(msg) => {
                    kill(&mut child);
                    return Err(Error::Blocked(msg));
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    let stderr_text = err_thread.join().unwrap_or_default();
    // O processo pode terminar entre duas conferências: confere de novo no fim.
    match tree_stats(out) {
        Ok((bytes, files)) if bytes > limits.max_bytes || files > limits.max_files => {
            return Err(Error::TooLarge(format!("{} MB e {files} arquivos descompactados", bytes >> 20)));
        }
        Err(msg) => return Err(Error::Blocked(msg)),
        _ => {}
    }
    if !status.success() {
        let tail: String = stderr_text.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
        return Err(Error::Archive(format!("o {} não conseguiu abrir o arquivo (código {}). {tail}", ex.name(), status.code().unwrap_or(-1))));
    }
    Ok(())
}

/// Empacota a pasta num `.zip` (sem as pastas vazias). Falha se a pasta estiver vazia.
fn zip_tree(dir: &Path, dest: &Path) -> Result<u64> {
    let tmp = dest.with_extension("zip.tmp");
    let file = std::fs::File::create(&tmp).map_err(|e| Error::io(&tmp, e))?;
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    let mut count = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&d).map_err(|e| Error::io(&d, e))?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            let meta = std::fs::symlink_metadata(&path).map_err(|er| Error::io(&path, er))?;
            if meta.file_type().is_symlink() {
                let _ = std::fs::remove_file(&tmp);
                return Err(Error::Blocked("o arquivo tem um link simbólico, recusado".into()));
            }
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                let rel = path.strip_prefix(dir).map_err(|_| Error::Archive("caminho fora da pasta".into()))?;
                let name = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
                w.start_file(name, opts).map_err(|er| Error::Archive(er.to_string()))?;
                let mut f = std::fs::File::open(&path).map_err(|er| Error::io(&path, er))?;
                std::io::copy(&mut f, &mut w).map_err(|er| Error::io(&path, er))?;
                count += 1;
            }
        }
    }
    let mut inner = w.finish().map_err(|e| Error::Archive(e.to_string()))?;
    inner.flush().map_err(|e| Error::io(&tmp, e))?;
    drop(inner);
    if count == 0 {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::Archive("o arquivo está vazio".into()));
    }
    std::fs::rename(&tmp, dest).map_err(|e| Error::io(dest, e))?;
    Ok(count)
}

/// Converte um `.rar`/`.7z` em `.zip` em `dest_zip`. Tenta os programas disponíveis em ordem.
pub fn convert_to_zip(src: &Path, fmt: Format, dest_zip: &Path) -> Result<u64> {
    convert_to_zip_with(src, fmt, dest_zip, &extractors_for(fmt), &Limits::default())
}

pub fn convert_to_zip_with(src: &Path, fmt: Format, dest_zip: &Path, extractors: &[Extractor], limits: &Limits) -> Result<u64> {
    if fmt == Format::Zip {
        return Err(Error::Archive("já é um .zip".into()));
    }
    if extractors.is_empty() {
        return Err(Error::NoExtractor(fmt.label().to_string()));
    }
    let work = dest_zip.with_extension("extraindo");
    let mut errors: Vec<String> = Vec::new();
    for ex in extractors {
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| Error::io(&work, e))?;
        let result = run_limited(ex, src, &work, limits).and_then(|()| zip_tree(&work, dest_zip));
        let _ = std::fs::remove_dir_all(&work);
        match result {
            Ok(n) => return Ok(n),
            // Recusas de segurança não são "tente outro programa": param aqui.
            Err(e @ (Error::TooLarge(_) | Error::Blocked(_))) => return Err(e),
            Err(e) => errors.push(e.to_string()),
        }
    }
    Err(Error::Archive(errors.join(" / ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("suny_arch_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    fn vint(mut n: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let b = (n & 0x7F) as u8;
            n >>= 7;
            out.push(if n > 0 { b | 0x80 } else { b });
            if n == 0 {
                return out;
            }
        }
    }

    fn block(body: Vec<u8>) -> Vec<u8> {
        let mut sized = vint(body.len() as u64);
        sized.extend(body);
        let mut out = crc32(&sized).to_le_bytes().to_vec();
        out.extend(sized);
        out
    }

    /// Um .rar (RAR5, método "armazenar") montado à mão: dá para testar a leitura sem o WinRAR.
    fn rar5(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = b"Rar!\x1a\x07\x01\x00".to_vec();
        out.extend(block([vint(1), vint(0), vint(0)].concat()));
        for (name, data) in files {
            let mut body = [vint(2), vint(0x2), vint(data.len() as u64), vint(0x4), vint(data.len() as u64), vint(0x20)].concat();
            body.extend(crc32(data).to_le_bytes());
            body.extend([vint(0), vint(1), vint(name.len() as u64)].concat());
            body.extend(name.as_bytes());
            out.extend(block(body));
            out.extend(*data);
        }
        out.extend(block([vint(5), vint(0), vint(0)].concat()));
        out
    }

    fn zip_names(p: &Path) -> Vec<String> {
        let mut a = zip::ZipArchive::new(std::fs::File::open(p).unwrap()).unwrap();
        let mut v: Vec<String> = (0..a.len()).map(|i| a.by_index(i).unwrap().name().unwrap().to_string()).collect();
        v.sort();
        v
    }

    #[test]
    fn format_is_decided_by_extension_only() {
        assert_eq!(format_of("Mod.ZIP"), Some(Format::Zip));
        assert_eq!(format_of("a.b.rar"), Some(Format::Rar));
        assert_eq!(format_of("x.7z"), Some(Format::SevenZ));
        assert_eq!(format_of("x.exe"), None);
        assert_eq!(format_of("semextensao"), None);
        assert_eq!(format_of("x.rar.txt"), None);
    }

    #[test]
    fn rar_is_converted_with_every_extractor_available() {
        let dir = tmp("rar");
        let src = dir.join("a.rar");
        std::fs::write(&src, rar5(&[("Meu Mod/sound/ui/hitsound.wav", b"RIFFdata"), ("Meu Mod/leia-me.txt", "olá".as_bytes())])).unwrap();
        let found = extractors_for(Format::Rar);
        if found.is_empty() {
            eprintln!("sem 7-Zip, WinRAR nem tar neste computador: teste de .rar pulado");
            return;
        }
        for ex in &found {
            let dest = dir.join(format!("{}.zip", ex.name().replace(' ', "_")));
            let n = convert_to_zip_with(&src, Format::Rar, &dest, std::slice::from_ref(ex), &Limits::default()).unwrap_or_else(|e| panic!("{}: {e}", ex.name()));
            assert_eq!(n, 2, "{}", ex.name());
            assert_eq!(zip_names(&dest), ["Meu Mod/leia-me.txt", "Meu Mod/sound/ui/hitsound.wav"], "{}", ex.name());
            assert!(!dir.join("a.extraindo").exists(), "a pasta temporária some");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_archive_is_an_error_and_leaves_no_zip_behind() {
        let dir = tmp("broken");
        let src = dir.join("ruim.rar");
        std::fs::write(&src, b"isto nao e um rar").unwrap();
        let found = extractors_for(Format::Rar);
        if found.is_empty() {
            return;
        }
        let dest = dir.join("ruim.zip");
        let e = convert_to_zip_with(&src, Format::Rar, &dest, &found, &Limits::default()).unwrap_err();
        assert!(matches!(e, Error::Archive(_)), "{e}");
        assert!(!dest.exists());
        assert!(!dir.join("ruim.extraindo").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_extractor_says_what_to_install() {
        let e = convert_to_zip_with(Path::new("x.rar"), Format::Rar, Path::new("x.zip"), &[], &Limits::default()).unwrap_err();
        assert!(matches!(e, Error::NoExtractor(_)));
        assert!(e.to_string().contains("7-Zip"), "{e}");
    }

    #[test]
    fn the_size_limit_stops_a_zip_bomb_style_archive() {
        let Some(sevenz) = find_7z() else { return };
        let dir = tmp("bomb");
        std::fs::write(dir.join("zeros.bin"), vec![0u8; 3 << 20]).unwrap();
        let archive = dir.join("bomba.7z");
        let st = Command::new(&sevenz).arg("a").arg("-bso0").arg(&archive).arg(dir.join("zeros.bin")).status().unwrap();
        assert!(st.success());
        assert!(std::fs::metadata(&archive).unwrap().len() < 4096, "comprime muito bem");
        let limits = Limits { max_bytes: 1 << 20, ..Limits::default() };
        let dest = dir.join("bomba.zip");
        let e = convert_to_zip_with(&archive, Format::SevenZ, &dest, &[Extractor::SevenZip(sevenz)], &limits).unwrap_err();
        assert!(matches!(e, Error::TooLarge(_)), "{e}");
        assert!(!dest.exists());
        assert!(!dir.join("bomba.extraindo").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sevenz_round_trip_keeps_names() {
        let Some(sevenz) = find_7z() else { return };
        let dir = tmp("sevenz");
        let src_dir = dir.join("Mod");
        std::fs::create_dir_all(src_dir.join("sound")).unwrap();
        std::fs::write(src_dir.join("sound").join("a.wav"), b"RIFF").unwrap();
        std::fs::write(src_dir.join("b.txt"), b"x").unwrap();
        let archive = dir.join("m.7z");
        let st = Command::new(&sevenz).current_dir(&dir).args(["a", "-bso0", "m.7z", "Mod"]).status().unwrap();
        assert!(st.success());
        let dest = dir.join("m.zip");
        convert_to_zip(&archive, Format::SevenZ, &dest).unwrap();
        assert_eq!(zip_names(&dest), ["Mod/b.txt", "Mod/sound/a.wav"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
