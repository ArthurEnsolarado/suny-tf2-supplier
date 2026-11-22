//! Ícones do próprio jogo, lidos **da instalação do usuário** (VPK → VTF → PNG).
//!
//! Como a arte da capa (`art.rs`), nada disso é distribuído com o app: ele só lê, em tempo de
//! execução, o que o TF2 do usuário já tem no disco. Só lê nomes de arquivo de uma lista fixa.
//!
//! - VPK: formato v1/v2 da Valve (árvore de diretórios + arquivos de dados `*_NNN.vpk`).
//! - VTF: textura da Source. Suporta os formatos usados em ícones (RGBA/BGRA/RGB, DXT1/3/5...).
//!   Pega sempre a maior imagem (mip 0, quadro 0).

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;

use crate::{Error, Result};

const VPK_SIGNATURE: u32 = 0x55AA_1234;
const IN_DIR_FILE: u16 = 0x7FFF;
/// Uma textura de ícone nunca é gigante; recusa o que passar disso.
const MAX_ENTRY_BYTES: u32 = 8 * 1024 * 1024;

// ---------------------------------------------------------------------------------------------
// VPK
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Entry {
    archive: u16,
    offset: u32,
    len: u32,
    preload: Vec<u8>,
}

#[derive(Debug)]
pub struct Vpk {
    dir_path: PathBuf,
    /// Caminho dos arquivos de dados sem `_NNN.vpk` (ex.: `.../tf2_textures`).
    prefix: String,
    /// Onde começam os dados que moram no próprio arquivo `_dir.vpk`.
    data_offset: u64,
    entries: HashMap<String, Entry>,
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn u16(&mut self) -> Result<u16> {
        let v = self.b.get(self.i..self.i + 2).ok_or_else(trunc)?;
        self.i += 2;
        Ok(u16::from_le_bytes([v[0], v[1]]))
    }
    fn u32(&mut self) -> Result<u32> {
        let v = self.b.get(self.i..self.i + 4).ok_or_else(trunc)?;
        self.i += 4;
        Ok(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let v = self.b.get(self.i..self.i + n).ok_or_else(trunc)?;
        self.i += n;
        Ok(v)
    }
    fn cstr(&mut self) -> Result<&'a str> {
        let rest = self.b.get(self.i..).ok_or_else(trunc)?;
        let end = rest.iter().position(|&c| c == 0).ok_or_else(trunc)?;
        self.i += end + 1;
        std::str::from_utf8(&rest[..end]).map_err(|_| Error::Archive("nome com bytes inválidos no VPK".into()))
    }
}

fn trunc() -> Error {
    Error::Archive("VPK truncado ou corrompido".into())
}

impl Vpk {
    /// Percorre a árvore de um `*_dir.vpk` e chama `sink(caminho_minúsculo, entrada)` para cada arquivo em que
    /// `wanted` é verdadeiro. Devolve (onde começam os dados no próprio `_dir.vpk`, prefixo dos `_NNN.vpk`).
    fn scan(dir_vpk: &Path, wanted: &dyn Fn(&str) -> bool, mut sink: impl FnMut(String, Entry)) -> Result<(u64, String)> {
        let mut f = File::open(dir_vpk).map_err(|e| Error::io(dir_vpk, e))?;
        let mut head = [0u8; 28];
        let n = f.read(&mut head).map_err(|e| Error::io(dir_vpk, e))?;
        let mut c = Cursor { b: &head[..n], i: 0 };
        if c.u32()? != VPK_SIGNATURE {
            return Err(Error::Archive("não é um arquivo VPK".into()));
        }
        let version = c.u32()?;
        let tree_size = c.u32()? as usize;
        let header_size: usize = match version {
            1 => 12,
            2 => 28,
            v => return Err(Error::Archive(format!("versão de VPK não suportada: {v}"))),
        };
        if tree_size == 0 || tree_size > 256 * 1024 * 1024 {
            return Err(Error::Archive("árvore do VPK com tamanho absurdo".into()));
        }

        f.seek(SeekFrom::Start(header_size as u64)).map_err(|e| Error::io(dir_vpk, e))?;
        let mut tree = vec![0u8; tree_size];
        f.read_exact(&mut tree).map_err(|e| Error::io(dir_vpk, e))?;

        let mut c = Cursor { b: &tree, i: 0 };
        loop {
            let ext = c.cstr()?;
            if ext.is_empty() {
                break;
            }
            loop {
                let dir = c.cstr()?;
                if dir.is_empty() {
                    break;
                }
                loop {
                    let name = c.cstr()?;
                    if name.is_empty() {
                        break;
                    }
                    let _crc = c.u32()?;
                    let preload_len = c.u16()? as usize;
                    let archive = c.u16()?;
                    let offset = c.u32()?;
                    let len = c.u32()?;
                    let _terminator = c.u16()?;
                    let preload = c.bytes(preload_len)?;

                    // " " significa "vazio" na árvore do VPK
                    let dir = if dir == " " { "" } else { dir };
                    let ext = if ext == " " { "" } else { ext };
                    let mut full = String::new();
                    if !dir.is_empty() {
                        full.push_str(dir);
                        full.push('/');
                    }
                    full.push_str(name);
                    if !ext.is_empty() {
                        full.push('.');
                        full.push_str(ext);
                    }
                    let key = full.to_ascii_lowercase();
                    if wanted(&key) {
                        sink(key, Entry { archive, offset, len, preload: preload.to_vec() });
                    }
                }
            }
        }

        let stem = dir_vpk.to_string_lossy();
        let prefix = stem.strip_suffix("_dir.vpk").unwrap_or(&stem).to_string();
        Ok(((header_size + tree_size) as u64, prefix))
    }

    /// Abre um `*_dir.vpk` e guarda só as entradas para as quais `wanted(caminho_minúsculo)` é verdadeiro.
    pub fn open(dir_vpk: &Path, wanted: impl Fn(&str) -> bool) -> Result<Self> {
        let mut entries = HashMap::new();
        let (data_offset, prefix) = Self::scan(dir_vpk, &wanted, |k, e| {
            entries.insert(k, e);
        })?;
        Ok(Self { dir_path: dir_vpk.to_path_buf(), prefix, data_offset, entries })
    }

    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(&path.to_ascii_lowercase())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let e = self.entries.get(&path.to_ascii_lowercase()).ok_or_else(|| Error::Archive(format!("{path} não está no VPK")))?;
        if e.len > MAX_ENTRY_BYTES {
            return Err(Error::TooLarge(format!("{path} tem {} bytes", e.len)));
        }
        let mut out = e.preload.clone();
        if e.len > 0 {
            let (file, base) = if e.archive == IN_DIR_FILE {
                (self.dir_path.clone(), self.data_offset)
            } else {
                (PathBuf::from(format!("{}_{:03}.vpk", self.prefix, e.archive)), 0)
            };
            let mut f = File::open(&file).map_err(|er| Error::io(&file, er))?;
            f.seek(SeekFrom::Start(base + e.offset as u64)).map_err(|er| Error::io(&file, er))?;
            let mut buf = vec![0u8; e.len as usize];
            f.read_exact(&mut buf).map_err(|er| Error::io(&file, er))?;
            out.extend_from_slice(&buf);
        }
        Ok(out)
    }
}

/// Só os caminhos (minúsculos, com `/`) de um `*_dir.vpk` para os quais `wanted` é verdadeiro. Não lê os dados.
pub fn list_paths(dir_vpk: &Path, wanted: impl Fn(&str) -> bool) -> Result<Vec<String>> {
    let mut out = Vec::new();
    Vpk::scan(dir_vpk, &wanted, |k, _| out.push(k))?;
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// VTF
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// RGBA, 8 bits por canal.
    pub rgba: Vec<u8>,
}

const FLAG_ENVMAP: u32 = 0x4000;

fn bad_vtf(msg: impl Into<String>) -> Error {
    Error::Archive(format!("textura VTF inválida: {}", msg.into()))
}

/// Tamanho em bytes de uma imagem `w`×`h` no formato `fmt` (números do `ImageFormat` da Source).
fn format_size(fmt: i32, w: u32, h: u32) -> Option<usize> {
    let (w, h) = (w as usize, h as usize);
    let blocks = w.div_ceil(4) * h.div_ceil(4);
    Some(match fmt {
        0 | 1 | 11 | 12 | 16 => w * h * 4, // RGBA8888, ABGR8888, ARGB8888, BGRA8888, BGRX8888
        2 | 3 => w * h * 3,                // RGB888, BGR888
        5 | 8 => w * h,                    // I8, A8
        6 => w * h * 2,                    // IA88
        13 | 20 => blocks * 8,             // DXT1, DXT1 com 1 bit de alfa
        14 | 15 => blocks * 16,            // DXT3, DXT5
        _ => return None,
    })
}

fn rgb565(v: u16) -> [u8; 3] {
    let r = ((v >> 11) & 31) as u32;
    let g = ((v >> 5) & 63) as u32;
    let b = (v & 31) as u32;
    [((r * 255 + 15) / 31) as u8, ((g * 255 + 31) / 63) as u8, ((b * 255 + 15) / 31) as u8]
}

fn decode_dxt(data: &[u8], w: usize, h: usize, kind: u8) -> Vec<u8> {
    // kind: 1 = DXT1, 3 = DXT3, 5 = DXT5
    let mut out = vec![0u8; w * h * 4];
    let block_size = if kind == 1 { 8 } else { 16 };
    let bw = w.div_ceil(4);
    for (bi, block) in data.chunks_exact(block_size).enumerate() {
        let (bx, by) = ((bi % bw) * 4, (bi / bw) * 4);
        let color = &block[block_size - 8..];
        let c0 = u16::from_le_bytes([color[0], color[1]]);
        let c1 = u16::from_le_bytes([color[2], color[3]]);
        let (p0, p1) = (rgb565(c0), rgb565(c1));
        let mut pal = [[0u8; 4]; 4];
        pal[0] = [p0[0], p0[1], p0[2], 255];
        pal[1] = [p1[0], p1[1], p1[2], 255];
        if kind != 1 || c0 > c1 {
            for k in 0..3 {
                pal[2][k] = ((2 * p0[k] as u32 + p1[k] as u32) / 3) as u8;
                pal[3][k] = ((p0[k] as u32 + 2 * p1[k] as u32) / 3) as u8;
            }
            pal[2][3] = 255;
            pal[3][3] = 255;
        } else {
            for k in 0..3 {
                pal[2][k] = ((p0[k] as u32 + p1[k] as u32) / 2) as u8;
            }
            pal[2][3] = 255;
            pal[3] = [0, 0, 0, 0];
        }
        let idx = u32::from_le_bytes([color[4], color[5], color[6], color[7]]);

        // alfa
        let mut alpha = [255u8; 16];
        match kind {
            3 => {
                for (i, a) in alpha.iter_mut().enumerate() {
                    let nib = (block[i / 2] >> (4 * (i % 2))) & 0xF;
                    *a = nib * 17;
                }
            }
            5 => {
                let (a0, a1) = (block[0] as u32, block[1] as u32);
                let mut table = [0u8; 8];
                table[0] = a0 as u8;
                table[1] = a1 as u8;
                if a0 > a1 {
                    for i in 1..7 {
                        table[i + 1] = (((7 - i as u32) * a0 + i as u32 * a1) / 7) as u8;
                    }
                } else {
                    for i in 1..5 {
                        table[i + 1] = (((5 - i as u32) * a0 + i as u32 * a1) / 5) as u8;
                    }
                    table[6] = 0;
                    table[7] = 255;
                }
                let mut bits = 0u64;
                for (i, b) in block[2..8].iter().enumerate() {
                    bits |= (*b as u64) << (8 * i);
                }
                for (i, a) in alpha.iter_mut().enumerate() {
                    *a = table[((bits >> (3 * i)) & 7) as usize];
                }
            }
            _ => {}
        }

        for py in 0..4 {
            for px in 0..4 {
                let (x, y) = (bx + px, by + py);
                if x >= w || y >= h {
                    continue;
                }
                let i = py * 4 + px;
                let mut c = pal[((idx >> (2 * i)) & 3) as usize];
                if kind != 1 {
                    c[3] = alpha[i];
                }
                out[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&c);
            }
        }
    }
    out
}

fn to_rgba(fmt: i32, data: &[u8], w: usize, h: usize) -> Result<Vec<u8>> {
    let px = w * h;
    let mut out = Vec::with_capacity(px * 4);
    match fmt {
        0 => out.extend_from_slice(&data[..px * 4]),
        1 => data[..px * 4].chunks_exact(4).for_each(|p| out.extend_from_slice(&[p[3], p[2], p[1], p[0]])), // ABGR
        11 => data[..px * 4].chunks_exact(4).for_each(|p| out.extend_from_slice(&[p[1], p[2], p[3], p[0]])), // ARGB
        12 => data[..px * 4].chunks_exact(4).for_each(|p| out.extend_from_slice(&[p[2], p[1], p[0], p[3]])), // BGRA
        16 => data[..px * 4].chunks_exact(4).for_each(|p| out.extend_from_slice(&[p[2], p[1], p[0], 255])), // BGRX
        2 => data[..px * 3].chunks_exact(3).for_each(|p| out.extend_from_slice(&[p[0], p[1], p[2], 255])),
        3 => data[..px * 3].chunks_exact(3).for_each(|p| out.extend_from_slice(&[p[2], p[1], p[0], 255])),
        5 => data[..px].iter().for_each(|&l| out.extend_from_slice(&[l, l, l, 255])),
        8 => data[..px].iter().for_each(|&a| out.extend_from_slice(&[255, 255, 255, a])),
        6 => data[..px * 2].chunks_exact(2).for_each(|p| out.extend_from_slice(&[p[0], p[0], p[0], p[1]])),
        13 | 20 => out = decode_dxt(data, w, h, 1),
        14 => out = decode_dxt(data, w, h, 3),
        15 => out = decode_dxt(data, w, h, 5),
        other => return Err(bad_vtf(format!("formato de imagem {other} não suportado"))),
    }
    Ok(out)
}

pub fn decode_vtf(data: &[u8]) -> Result<Image> {
    if data.len() < 63 || &data[0..4] != b"VTF\0" {
        return Err(bad_vtf("assinatura ausente"));
    }
    let u32at = |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
    let u16at = |o: usize| u16::from_le_bytes([data[o], data[o + 1]]);
    let header_size = u32at(12) as usize;
    let (width, height) = (u16at(16) as u32, u16at(18) as u32);
    let flags = u32at(20);
    let frames = u16at(24).max(1) as usize;
    let fmt = i32::from_le_bytes([data[52], data[53], data[54], data[55]]);
    let mips = data[56].max(1) as u32;
    let low_fmt = i32::from_le_bytes([data[57], data[58], data[59], data[60]]);
    let (low_w, low_h) = (data[61] as u32, data[62] as u32);

    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(bad_vtf(format!("dimensões {width}x{height}")));
    }
    if flags & FLAG_ENVMAP != 0 {
        return Err(bad_vtf("cubemap não suportado"));
    }
    let size = |level: u32| format_size(fmt, (width >> level).max(1), (height >> level).max(1));
    let first = size(0).ok_or_else(|| bad_vtf(format!("formato de imagem {fmt} não suportado")))?;

    // Ordem no arquivo: cabeçalho, miniatura (sempre DXT1), depois os mips do MENOR para o MAIOR.
    let low = if low_fmt >= 0 && low_w > 0 && low_h > 0 { format_size(13, low_w, low_h).unwrap_or(0) } else { 0 };
    let mut offset = header_size + low;
    for level in (1..mips).rev() {
        offset += size(level).ok_or_else(|| bad_vtf("tamanho de mip"))? * frames;
    }
    let body = data.get(offset..offset + first).ok_or_else(|| bad_vtf("dados menores que o esperado"))?;
    Ok(Image { width, height, rgba: to_rgba(fmt, body, width as usize, height as usize)? })
}

pub fn encode_png(img: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| Error::Archive(format!("PNG: {e}")))?;
        w.write_image_data(&img.rgba).map_err(|e| Error::Archive(format!("PNG: {e}")))?;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// API usada pela interface
// ---------------------------------------------------------------------------------------------

/// Ícones de classe do jogo (lista fixa). A chave é o que a interface usa.
pub const CLASS_ICONS: &[(&str, &str)] = &[
    ("all", "materials/vgui/class_icons/filter_all.vtf"),
    ("scout", "materials/vgui/class_icons/filter_scout.vtf"),
    ("soldier", "materials/vgui/class_icons/filter_soldier.vtf"),
    ("pyro", "materials/vgui/class_icons/filter_pyro.vtf"),
    ("demoman", "materials/vgui/class_icons/filter_demo.vtf"),
    ("heavy", "materials/vgui/class_icons/filter_heavy.vtf"),
    ("engineer", "materials/vgui/class_icons/filter_engineer.vtf"),
    ("medic", "materials/vgui/class_icons/filter_medic.vtf"),
    ("sniper", "materials/vgui/class_icons/filter_sniper.vtf"),
    ("spy", "materials/vgui/class_icons/filter_spy.vtf"),
];

#[derive(Debug, Clone, Default, Serialize)]
pub struct GameIcons {
    /// chave → data URL PNG. Vazio se o jogo não está instalado ou o VPK não pôde ser lido.
    pub icons: HashMap<String, String>,
}

/// Os ícones de classe do jogo são quadrados OPACOS (fundo escuro + desenho cinza). Em cima de painéis de cores
/// diferentes uns parecem ter fundo e outros só contorno. Aqui o fundo vira transparência e o desenho vira creme,
/// com a mesma regra para todos os ícones.
pub fn to_glyph(img: &mut Image) {
    let lum = |p: &[u8]| 0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]);
    let Some(first) = img.rgba.chunks_exact(4).next() else { return };
    let bg = lum(first);
    let fg = img.rgba.chunks_exact(4).map(lum).fold(bg, f32::max);
    if fg - bg < 8.0 {
        return; // sem contraste: não dá para separar fundo e desenho, mantém como veio
    }
    for px in img.rgba.chunks_exact_mut(4) {
        let a = ((lum(px) - bg) / (fg - bg)).clamp(0.0, 1.0);
        px.copy_from_slice(&[244, 236, 205, (a * 255.0).round() as u8]);
    }
}

/// Retratos das classes (o rosto de cada personagem), da pasta de texturas do jogo. A chave é a mesma de `CLASS_ICONS`.
pub const CLASS_PORTRAITS: &[(&str, &str)] = &[
    ("all", "materials/vgui/class_portraits/all_class.vtf"),
    ("scout", "materials/vgui/class_portraits/scout.vtf"),
    ("soldier", "materials/vgui/class_portraits/soldier.vtf"),
    ("pyro", "materials/vgui/class_portraits/pyro.vtf"),
    ("demoman", "materials/vgui/class_portraits/demoman.vtf"),
    ("heavy", "materials/vgui/class_portraits/heavy.vtf"),
    ("engineer", "materials/vgui/class_portraits/engineer.vtf"),
    ("medic", "materials/vgui/class_portraits/medic.vtf"),
    ("sniper", "materials/vgui/class_portraits/sniper.vtf"),
    ("spy", "materials/vgui/class_portraits/spy.vtf"),
];

/// Reduz a imagem à metade (média de 2x2 pixels). Os retratos têm 256 px; na interface bastam 128, e o texto que
/// passa pela ponte fica 4 vezes menor.
pub fn downscale_half(img: &Image) -> Image {
    let (w, h) = ((img.width / 2).max(1), (img.height / 2).max(1));
    let src = |x: u32, y: u32, c: usize| u32::from(img.rgba[((y.min(img.height - 1) * img.width + x.min(img.width - 1)) * 4) as usize + c]);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let sum = src(2 * x, 2 * y, c) + src(2 * x + 1, 2 * y, c) + src(2 * x, 2 * y + 1, c) + src(2 * x + 1, 2 * y + 1, c);
                rgba.push(((sum + 2) / 4) as u8);
            }
        }
    }
    Image { width: w, height: h, rgba }
}

/// Lê os retratos das classes do jogo, já em 128 px (data URL PNG). Nunca falha: o que não der para ler fica de fora.
pub fn load_class_portraits(tf_dir: &Path) -> GameIcons {
    let wanted: Vec<&str> = CLASS_PORTRAITS.iter().map(|(_, p)| *p).collect();
    let Ok(vpk) = Vpk::open(&tf_dir.join("tf2_textures_dir.vpk"), |p| wanted.contains(&p)) else {
        return GameIcons::default();
    };
    let mut icons = HashMap::new();
    for (key, path) in CLASS_PORTRAITS {
        let Ok(bytes) = vpk.read(path) else { continue };
        let Ok(img) = decode_vtf(&bytes) else { continue };
        let small = if img.width > 160 { downscale_half(&img) } else { img };
        if let Ok(png) = encode_png(&small) {
            icons.insert((*key).to_string(), format!("data:image/png;base64,{}", STANDARD.encode(png)));
        }
    }
    GameIcons { icons }
}

/// Converte um arquivo do VPK em PNG (data URL).
fn icon_data_url(vpk: &Vpk, path: &str) -> Result<String> {
    let mut img = decode_vtf(&vpk.read(path)?)?;
    to_glyph(&mut img);
    let png = encode_png(&img)?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

/// Lê os ícones de classe da pasta `tf/` do jogo. Nunca falha: o que não der para ler fica de fora.
pub fn load_class_icons(tf_dir: &Path) -> GameIcons {
    let wanted: Vec<&str> = CLASS_ICONS.iter().map(|(_, p)| *p).collect();
    let Ok(vpk) = Vpk::open(&tf_dir.join("tf2_textures_dir.vpk"), |p| wanted.contains(&p)) else {
        return GameIcons::default();
    };
    let mut icons = HashMap::new();
    for (key, path) in CLASS_ICONS {
        if let Ok(url) = icon_data_url(&vpk, path) {
            icons.insert((*key).to_string(), url);
        }
    }
    GameIcons { icons }
}

/// Ferramenta de desenvolvimento: converte entradas do VPK de texturas em PNG (bytes).
pub fn extract_png(tf_dir: &Path, vpk_path: &str) -> Result<Vec<u8>> {
    let want = vpk_path.to_ascii_lowercase();
    let vpk = Vpk::open(&tf_dir.join("tf2_textures_dir.vpk"), |p| p == want)?;
    encode_png(&decode_vtf(&vpk.read(vpk_path)?)?)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;

    /// Monta um VTF 7.2 sem miniatura, com `mips` níveis (dados já na ordem do arquivo).
    pub(crate) fn make_vtf(w: u16, h: u16, fmt: i32, mips: u8, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"VTF\0");
        v.extend_from_slice(&7u32.to_le_bytes());
        v.extend_from_slice(&2u32.to_le_bytes());
        v.extend_from_slice(&80u32.to_le_bytes()); // header_size
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // flags
        v.extend_from_slice(&1u16.to_le_bytes()); // frames
        v.extend_from_slice(&0u16.to_le_bytes()); // first frame
        v.extend_from_slice(&[0u8; 4]);
        v.extend_from_slice(&[0u8; 12]); // reflectivity
        v.extend_from_slice(&[0u8; 4]);
        v.extend_from_slice(&1.0f32.to_le_bytes()); // bumpmap scale
        v.extend_from_slice(&fmt.to_le_bytes());
        v.push(mips);
        v.extend_from_slice(&(-1i32).to_le_bytes()); // sem miniatura
        v.push(0);
        v.push(0);
        v.extend_from_slice(&1u16.to_le_bytes()); // depth
        v.resize(80, 0);
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn decodes_uncompressed_formats() {
        // RGBA8888 2x2
        let rgba = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 128];
        let img = decode_vtf(&make_vtf(2, 2, 0, 1, &rgba)).unwrap();
        assert_eq!((img.width, img.height), (2, 2));
        assert_eq!(img.rgba, rgba);

        // BGRA8888: o pixel azul (B=255) vira RGBA (0,0,255)
        let bgra = [255, 0, 0, 255];
        assert_eq!(decode_vtf(&make_vtf(1, 1, 12, 1, &bgra)).unwrap().rgba, [0, 0, 255, 255]);

        // BGR888 e I8
        assert_eq!(decode_vtf(&make_vtf(1, 1, 3, 1, &[1, 2, 3])).unwrap().rgba, [3, 2, 1, 255]);
        assert_eq!(decode_vtf(&make_vtf(1, 1, 5, 1, &[77])).unwrap().rgba, [77, 77, 77, 255]);
    }

    #[test]
    fn decodes_dxt1_and_dxt5_blocks() {
        // DXT1: c0 = vermelho puro (565 = 0xF800), c1 = 0, todos os índices 0 => tudo vermelho
        let dxt1 = [0x00, 0xF8, 0x00, 0x00, 0, 0, 0, 0];
        let img = decode_vtf(&make_vtf(4, 4, 13, 1, &dxt1)).unwrap();
        assert!(img.rgba.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));

        // DXT5: alfa a0=200, a1=0, índices 0 => alfa 200; cor c0 = verde puro (0x07E0)
        let mut dxt5 = vec![200u8, 0, 0, 0, 0, 0, 0, 0];
        dxt5.extend_from_slice(&[0xE0, 0x07, 0x00, 0x00, 0, 0, 0, 0]);
        let img = decode_vtf(&make_vtf(4, 4, 15, 1, &dxt5)).unwrap();
        assert!(img.rgba.chunks_exact(4).all(|p| p == [0, 255, 0, 200]));
    }

    #[test]
    fn dxt1_three_color_mode_has_transparent_index_3() {
        // c0 < c1 => modo de 3 cores; índice 3 = transparente. Todos os pixels usam o índice 3 (0b11 repetido).
        let block = [0x00, 0x00, 0x00, 0xF8, 0xFF, 0xFF, 0xFF, 0xFF];
        let img = decode_vtf(&make_vtf(4, 4, 13, 1, &block)).unwrap();
        assert!(img.rgba.chunks_exact(4).all(|p| p[3] == 0));
    }

    #[test]
    fn picks_the_largest_mip_which_is_stored_last() {
        // 2 níveis RGBA8888: mip1 (1x1) vem ANTES do mip0 (2x2)
        let mut body = vec![9, 9, 9, 9]; // mip1
        let mip0 = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        body.extend_from_slice(&mip0);
        let img = decode_vtf(&make_vtf(2, 2, 0, 2, &body)).unwrap();
        assert_eq!(img.rgba, mip0);
    }

    #[test]
    fn rejects_garbage_and_unsupported_input_without_panicking() {
        assert!(decode_vtf(b"nao eh vtf").is_err());
        assert!(decode_vtf(&[]).is_err());
        assert!(decode_vtf(&make_vtf(2, 2, 24, 1, &[0; 64])).is_err(), "RGBA16161616F não suportado");
        assert!(decode_vtf(&make_vtf(4, 4, 0, 1, &[0; 8])).is_err(), "dados menores que o esperado");
        assert!(decode_vtf(&make_vtf(0, 4, 0, 1, &[])).is_err(), "largura zero");
        let mut env = make_vtf(2, 2, 0, 1, &[0; 16]);
        env[20..24].copy_from_slice(&FLAG_ENVMAP.to_le_bytes());
        assert!(decode_vtf(&env).is_err(), "cubemap");
    }

    #[test]
    fn png_output_has_the_png_signature() {
        let img = decode_vtf(&make_vtf(1, 1, 0, 1, &[1, 2, 3, 4])).unwrap();
        let png = encode_png(&img).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    /// Escreve um VPK v2 de teste. `in_dir`: dados dentro do próprio `_dir.vpk` (senão no `_000.vpk`).
    pub(crate) fn make_vpk(dir: &Path, files: &[(&str, &str, &[u8])], in_dir: bool) -> PathBuf {
        let mut tree = Vec::new();
        let mut data = Vec::new();
        // uma extensão, um diretório por arquivo (suficiente para o teste)
        tree.extend_from_slice(b"vtf\0");
        for (path, name, bytes) in files {
            tree.extend_from_slice(path.as_bytes());
            tree.push(0);
            tree.extend_from_slice(name.as_bytes());
            tree.push(0);
            tree.extend_from_slice(&0u32.to_le_bytes()); // crc
            tree.extend_from_slice(&0u16.to_le_bytes()); // preload
            tree.extend_from_slice(&(if in_dir { IN_DIR_FILE } else { 0 }).to_le_bytes());
            tree.extend_from_slice(&(data.len() as u32).to_le_bytes());
            tree.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            tree.extend_from_slice(&0xFFFFu16.to_le_bytes());
            tree.push(0); // fim dos nomes deste diretório
            data.extend_from_slice(bytes);
        }
        tree.push(0); // fim dos diretórios
        tree.push(0); // fim das extensões

        let mut header = Vec::new();
        header.extend_from_slice(&VPK_SIGNATURE.to_le_bytes());
        header.extend_from_slice(&2u32.to_le_bytes());
        header.extend_from_slice(&(tree.len() as u32).to_le_bytes());
        header.extend_from_slice(&(if in_dir { data.len() as u32 } else { 0 }).to_le_bytes());
        header.extend_from_slice(&[0u8; 12]);

        let dir_path = dir.join("tf2_textures_dir.vpk");
        let mut dir_bytes = header;
        dir_bytes.extend_from_slice(&tree);
        if in_dir {
            dir_bytes.extend_from_slice(&data);
        } else {
            fs::write(dir.join("tf2_textures_000.vpk"), &data).unwrap();
        }
        fs::write(&dir_path, dir_bytes).unwrap();
        dir_path
    }

    pub(crate) fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("suny_icons_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn reads_files_from_a_vpk_with_data_in_separate_archive_and_in_the_dir_file() {
        let a = make_vtf(1, 1, 0, 1, &[10, 20, 30, 255]);
        let b = make_vtf(1, 1, 0, 1, &[40, 50, 60, 255]);
        for in_dir in [false, true] {
            let d = tmp(if in_dir { "indir" } else { "arch" });
            let dir_vpk = make_vpk(&d, &[("materials/vgui/class_icons", "filter_scout", &a), ("materials/vgui/other", "x", &b)], in_dir);
            let vpk = Vpk::open(&dir_vpk, |_| true).unwrap();
            assert_eq!(vpk.len(), 2);
            assert!(vpk.contains("Materials/VGUI/class_icons/FILTER_SCOUT.vtf"), "busca ignora maiúsculas");
            assert_eq!(vpk.read("materials/vgui/class_icons/filter_scout.vtf").unwrap(), a);
            assert_eq!(vpk.read("materials/vgui/other/x.vtf").unwrap(), b);
            assert!(vpk.read("materials/vgui/nao/existe.vtf").is_err());

            // o filtro `wanted` limita o que fica na memória
            let only = Vpk::open(&dir_vpk, |p| p.contains("class_icons")).unwrap();
            assert_eq!(only.len(), 1);
            let _ = fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn load_class_icons_converts_what_exists_and_skips_the_rest() {
        let scout = make_vtf(1, 1, 0, 1, &[1, 2, 3, 255]);
        let d = tmp("load");
        make_vpk(&d, &[("materials/vgui/class_icons", "filter_scout", &scout), ("materials/vgui/class_icons", "filter_spy", b"lixo que nao e vtf")], false);
        let icons = load_class_icons(&d);
        assert!(icons.icons.get("scout").unwrap().starts_with("data:image/png;base64,"));
        assert!(!icons.icons.contains_key("spy"), "VTF inválido fica de fora, sem derrubar o resto");
        assert!(!icons.icons.contains_key("medic"), "ausente no VPK");
        // sem o VPK: vazio, sem erro
        assert!(load_class_icons(Path::new("Z:/nao/existe")).icons.is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn list_paths_returns_only_wanted_paths_without_reading_data() {
        let d = tmp("list");
        let dir_vpk = make_vpk(&d, &[("sound/ui", "hitsound", b"x"), ("materials/vgui", "a", b"y"), ("sound/vo", "demoman_x", b"z")], false);
        let mut all = list_paths(&dir_vpk, |_| true).unwrap();
        all.sort();
        assert_eq!(all, vec!["materials/vgui/a.vtf", "sound/ui/hitsound.vtf", "sound/vo/demoman_x.vtf"]);
        let sounds = list_paths(&dir_vpk, |p| p.starts_with("sound/")).unwrap();
        assert_eq!(sounds.len(), 2);
        // os arquivos de dados nem precisam existir para listar
        std::fs::remove_file(d.join("tf2_textures_000.vpk")).unwrap();
        assert_eq!(list_paths(&dir_vpk, |_| true).unwrap().len(), 3);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn downscaling_averages_two_by_two_blocks_and_keeps_alpha() {
        let img = Image { width: 2, height: 2, rgba: vec![0, 0, 0, 255, 100, 100, 100, 255, 100, 100, 100, 255, 200, 200, 200, 255] };
        let half = downscale_half(&img);
        assert_eq!((half.width, half.height), (1, 1));
        assert_eq!(half.rgba, vec![100, 100, 100, 255]);
        let odd = Image { width: 3, height: 1, rgba: vec![9; 12] };
        let h = downscale_half(&odd);
        assert_eq!((h.width, h.height), (1, 1), "tamanho ímpar não estoura");
    }

    #[test]
    fn portraits_cover_every_class_the_icons_cover_and_a_missing_game_gives_nothing() {
        let icon_keys: Vec<&str> = CLASS_ICONS.iter().map(|(k, _)| *k).collect();
        let portrait_keys: Vec<&str> = CLASS_PORTRAITS.iter().map(|(k, _)| *k).collect();
        assert_eq!(icon_keys, portrait_keys);
        assert!(load_class_portraits(std::path::Path::new("Z:/nao/existe")).icons.is_empty());
    }

    #[test]
    fn corrupt_vpk_is_an_error_not_a_panic() {
        let d = tmp("corrupt");
        let p = d.join("tf2_textures_dir.vpk");
        fs::write(&p, b"isto nao e um vpk").unwrap();
        assert!(Vpk::open(&p, |_| true).is_err());
        // assinatura certa, mas árvore truncada
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&VPK_SIGNATURE.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&500u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(b"vtf\0materials");
        fs::write(&p, bytes).unwrap();
        assert!(Vpk::open(&p, |_| true).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    /// Teste REAL com o seu TF2 (só leitura): `$env:SUNY_LIVE_TF2_DIR='<pasta tf>'; cargo test -p suny-core live_game_icons -- --ignored --nocapture`
    #[test]
    #[ignore = "lê a instalação real do TF2 (SUNY_LIVE_TF2_DIR)"]
    fn live_game_icons_decode_from_the_real_game() {
        let tf = std::env::var("SUNY_LIVE_TF2_DIR").expect("defina SUNY_LIVE_TF2_DIR com a pasta tf do jogo");
        let icons = load_class_icons(Path::new(&tf));
        println!("ícones lidos: {:?}", { let mut k: Vec<_> = icons.icons.keys().cloned().collect(); k.sort(); k });
        assert_eq!(icons.icons.len(), CLASS_ICONS.len(), "todos os ícones de classe decodificaram");
    }

    #[test]
    fn glyph_conversion_makes_background_transparent_and_the_drawing_cream() {
        // 3 pixels: fundo escuro, meio-tom (borda suavizada), desenho claro
        let mut img = Image { width: 3, height: 1, rgba: vec![40, 36, 32, 255, 100, 96, 92, 255, 140, 136, 130, 255] };
        to_glyph(&mut img);
        assert_eq!(img.rgba[3], 0, "o fundo some");
        assert_eq!(img.rgba[11], 255, "o desenho fica sólido");
        assert!(img.rgba[7] > 60 && img.rgba[7] < 200, "a borda suavizada vira transparência parcial: {}", img.rgba[7]);
        assert_eq!(&img.rgba[8..11], &[244, 236, 205]);
        // sem contraste, nada muda
        let mut flat = Image { width: 2, height: 1, rgba: vec![50, 50, 50, 255, 52, 52, 52, 255] };
        let before = flat.rgba.clone();
        to_glyph(&mut flat);
        assert_eq!(flat.rgba, before);
    }
}
