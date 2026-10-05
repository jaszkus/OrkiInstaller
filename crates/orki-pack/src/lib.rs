pub mod format;

pub use format::{
    FLAG_HAS_ASSETS, FLAG_HAS_STRING_TABLE, FLAG_HAS_UNINSTALLER, FLAG_SIGNED, HEADER_MAGIC,
    HEADER_SIZE, MAX_ASSETS, MAX_CHUNKS, MAX_FILE_SIZE, MAX_FILES, MAX_MANIFEST_LEN,
    MAX_PATH_UTF16, MAX_RAW_CHUNK, MAX_TOTAL_RAW, PAYLOAD_FORMAT_VERSION, PayloadHeader,
    SIGNATURE_BLOCK_SIZE, locate, overlay_start, read_payload_header, write_header,
};

pub const MAGIC: [u8; 8] = *b"ORKIPACK";
pub const FORMAT_VERSION: u32 = 1;
pub const FOOTER_SIZE: u64 = 64;

pub const CODEC_STORE: u8 = 0;
pub const CODEC_LZMA2: u8 = 1;
pub const CODEC_BROTLI: u8 = 2;
pub const CODEC_ZSTD: u8 = 3;

pub const CHUNK_MIN: u32 = 16 * 1024;
pub const CHUNK_AVG: u32 = 64 * 1024;
pub const CHUNK_MAX: u32 = 256 * 1024;

const MIN_AUTO_RAW: usize = 512;

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("bad magic")]
    BadMagic,
    #[error("truncated payload")]
    Truncated,
    #[error("crc mismatch")]
    Crc,
    #[error("hash mismatch for chunk {0}")]
    Hash(usize),
    #[error("unknown codec {0}")]
    Codec(u8),
    #[error("manifest: {0}")]
    Manifest(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported payload format version: {0}")]
    UnsupportedVersion(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PackManifest {
    pub schema: u32,
    pub app_id: String,
    pub app_name: String,
    pub app_version: String,
    pub files: Vec<FileEntry>,
    pub chunks: Vec<Chunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,
    pub chunk_start: u32,
    pub chunk_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Chunk {
    pub codec: u8,
    pub comp_len: u32,
    pub raw_len: u32,
    pub blake3: [u8; 32],
    pub crc32: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footer {
    pub format_version: u32,
    pub manifest_offset: u64,
    pub manifest_len: u64,
    pub manifest_crc32: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecPolicy {
    Auto,
    Store,
    Lzma2,
    Brotli,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMeta {
    pub id: String,
    pub name: String,
    pub version: String,
}

impl AppMeta {
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
        }
    }
}

pub struct PackBuilder {
    body: Vec<u8>,
    chunks: Vec<Chunk>,
    files: Vec<FileEntry>,
    codec: CodecPolicy,
}

impl Default for PackBuilder {
    fn default() -> Self {
        Self::new(0)
    }
}

impl PackBuilder {
    pub fn new(_base: u64) -> Self {
        Self {
            body: Vec::new(),
            chunks: Vec::new(),
            files: Vec::new(),
            codec: CodecPolicy::Auto,
        }
    }

    pub fn codec(mut self, policy: CodecPolicy) -> Self {
        self.codec = policy;
        self
    }

    pub fn add_file(&mut self, rel_path: &str, data: &[u8]) -> Result<(), PackError> {
        let rel = orki_core::paths::sanitize_rel_path(rel_path)
            .map_err(|e| PackError::Manifest(e.to_string()))?;
        let path = rel.to_string_lossy().replace('\\', "/");
        if self.files.iter().any(|f| f.path == path) {
            return Err(PackError::Manifest(format!("duplicate path: {path}")));
        }
        let chunk_start = self.chunks.len() as u32;
        let offset = HEADER_SIZE as u64 + self.body.len() as u64;
        if data.is_empty() {
            self.files.push(FileEntry {
                path,
                offset,
                size: 0,
                chunk_start,
                chunk_count: 0,
            });
            return Ok(());
        }
        let mut count = 0u32;
        for part in fastcdc::v2020::FastCDC::new(
            data,
            CHUNK_MIN as usize,
            CHUNK_AVG as usize,
            CHUNK_MAX as usize,
        ) {
            let raw = &data[part.offset..part.offset + part.length];
            let hash = blake3::hash(raw);
            let crc = crc32fast::hash(raw);
            let (codec, comp) = encode_chunk(raw, self.codec)?;
            self.body.extend_from_slice(&comp);
            self.chunks.push(Chunk {
                codec,
                comp_len: comp.len() as u32,
                raw_len: raw.len() as u32,
                blake3: hash.into(),
                crc32: crc,
            });
            count += 1;
        }
        self.files.push(FileEntry {
            path,
            offset,
            size: data.len() as u64,
            chunk_start,
            chunk_count: count,
        });
        Ok(())
    }

    pub fn finish(self, meta: &AppMeta) -> Vec<u8> {
        let manifest = PackManifest {
            schema: 1,
            app_id: meta.id.clone(),
            app_name: meta.name.clone(),
            app_version: meta.version.clone(),
            files: self.files,
            chunks: self.chunks,
        };
        let encoded = postcard::to_allocvec(&manifest).expect("postcard encode");
        let manifest_crc32 = crc32fast::hash(&encoded);
        let manifest_len = encoded.len() as u64;
        let manifest_offset = HEADER_SIZE as u64 + self.body.len() as u64;
        let payload_len = manifest_offset + manifest_len;
        let header = write_header(
            payload_len,
            manifest_offset,
            manifest_len,
            manifest_crc32,
            0,
            0,
        );
        let mut out = Vec::with_capacity(payload_len as usize);
        out.extend_from_slice(&header);
        out.extend_from_slice(&self.body);
        out.extend_from_slice(&encoded);
        out
    }
}

fn encode_chunk(data: &[u8], policy: CodecPolicy) -> Result<(u8, Vec<u8>), PackError> {
    Ok(match policy {
        CodecPolicy::Store => (CODEC_STORE, data.to_vec()),
        CodecPolicy::Lzma2 => (CODEC_LZMA2, compress_lzma2(data)?),
        CodecPolicy::Brotli => (CODEC_BROTLI, compress_brotli(data)?),
        CodecPolicy::Auto => {
            if data.len() < MIN_AUTO_RAW {
                (CODEC_STORE, data.to_vec())
            } else {
                let lzma2 = compress_lzma2(data)?;
                if lzma2.len() < data.len() {
                    (CODEC_LZMA2, lzma2)
                } else {
                    let brotli_c = compress_brotli(data)?;
                    if brotli_c.len() < data.len() {
                        (CODEC_BROTLI, brotli_c)
                    } else {
                        (CODEC_STORE, data.to_vec())
                    }
                }
            }
        }
    })
}

fn compress_lzma2(data: &[u8]) -> Result<Vec<u8>, PackError> {
    use std::io::Write;
    let mut opts = lzma_rust2::Lzma2Options::with_preset(6);
    opts.lzma_options.dict_size = lzma_rust2::LzmaOptions::DICT_SIZE_DEFAULT;
    let mut out = Vec::with_capacity(data.len() / 2 + 64);
    let mut writer = lzma_rust2::Lzma2Writer::new(&mut out, opts);
    writer.write_all(data)?;
    writer.finish()?;
    Ok(out)
}

fn compress_brotli(data: &[u8]) -> Result<Vec<u8>, PackError> {
    let params = brotli::enc::BrotliEncoderParams {
        quality: 9,
        ..Default::default()
    };
    let mut out = Vec::with_capacity(data.len() / 2 + 64);
    brotli::BrotliCompress(&mut &data[..], &mut out, &params)?;
    Ok(out)
}

pub enum DetectedFormat {
    V1 {
        overlay: usize,
        header: PayloadHeader,
    },
    V0 {
        footer: Footer,
    },
}

pub fn detect(data: &[u8]) -> Result<DetectedFormat, PackError> {
    let overlay = overlay_start(data);
    if let Ok(header) = read_payload_header(data, overlay) {
        let payload_end = overlay
            .checked_add(header.payload_len as usize)
            .ok_or(PackError::Truncated)?;
        if payload_end <= data.len() {
            return Ok(DetectedFormat::V1 { overlay, header });
        }
        return Err(PackError::Truncated);
    }
    let footer = format::footer_v0(data)?;
    Ok(DetectedFormat::V0 { footer })
}

pub fn read_footer(data: &[u8]) -> Result<Footer, PackError> {
    if data.len() < FOOTER_SIZE as usize {
        return Err(PackError::Truncated);
    }
    let f = &data[data.len() - 64..];
    if f[0..8] != MAGIC {
        return Err(PackError::BadMagic);
    }
    let header_crc = u32::from_le_bytes([f[56], f[57], f[58], f[59]]);
    if crc32fast::hash(&f[0..56]) != header_crc {
        return Err(PackError::Crc);
    }
    Ok(Footer {
        format_version: u32::from_le_bytes([f[8], f[9], f[10], f[11]]),
        manifest_offset: u64::from_le_bytes([
            f[12], f[13], f[14], f[15], f[16], f[17], f[18], f[19],
        ]),
        manifest_len: u64::from_le_bytes([f[20], f[21], f[22], f[23], f[24], f[25], f[26], f[27]]),
        manifest_crc32: u32::from_le_bytes([f[28], f[29], f[30], f[31]]),
    })
}

pub fn read_manifest(data: &[u8]) -> Result<PackManifest, PackError> {
    let (start, len, crc32) = match detect(data)? {
        DetectedFormat::V1 { overlay, header } => {
            if header.manifest_len > MAX_MANIFEST_LEN {
                return Err(PackError::Truncated);
            }
            let start = overlay
                .checked_add(header.manifest_offset as usize)
                .ok_or(PackError::Truncated)?;
            (start, header.manifest_len, header.manifest_crc32)
        }
        DetectedFormat::V0 { footer } => {
            let start =
                usize::try_from(footer.manifest_offset).map_err(|_| PackError::Truncated)?;
            (start, footer.manifest_len, footer.manifest_crc32)
        }
    };
    let end = start
        .checked_add(usize::try_from(len).map_err(|_| PackError::Truncated)?)
        .ok_or(PackError::Truncated)?;
    if end > data.len() {
        return Err(PackError::Truncated);
    }
    let encoded = &data[start..end];
    if crc32fast::hash(encoded) != crc32 {
        return Err(PackError::Crc);
    }
    let m: PackManifest =
        postcard::from_bytes(encoded).map_err(|e| PackError::Manifest(e.to_string()))?;
    Ok(m)
}

pub fn extract_file(
    data: &[u8],
    m: &PackManifest,
    entry: &FileEntry,
) -> Result<Vec<u8>, PackError> {
    let base = match detect(data)? {
        DetectedFormat::V1 { overlay, .. } => overlay,
        DetectedFormat::V0 { .. } => 0,
    };
    let start = usize::try_from(entry.offset)
        .map_err(|_| PackError::Truncated)?
        .checked_add(base)
        .ok_or(PackError::Truncated)?;
    let mut out = Vec::with_capacity(entry.size as usize);
    let mut consumed = 0u32;
    for i in 0..entry.chunk_count {
        let idx = (entry.chunk_start + i) as usize;
        let chunk = m.chunks.get(idx).ok_or(PackError::Truncated)?;
        let cstart = start + consumed as usize;
        let cend = cstart + chunk.comp_len as usize;
        if cend > data.len() {
            return Err(PackError::Truncated);
        }
        let raw = decode_chunk(chunk.codec, &data[cstart..cend], chunk.raw_len as usize)?;
        if blake3::hash(&raw).as_bytes() != &chunk.blake3 {
            return Err(PackError::Hash(idx));
        }
        if crc32fast::hash(&raw) != chunk.crc32 {
            return Err(PackError::Crc);
        }
        out.extend_from_slice(&raw);
        consumed += chunk.comp_len;
    }
    if out.len() as u64 != entry.size {
        return Err(PackError::Truncated);
    }
    Ok(out)
}

pub struct IntegrityReport {
    pub chunks_checked: usize,
    pub bytes_checked: u64,
}

pub fn verify_integrity(data: &[u8], m: &PackManifest) -> Result<IntegrityReport, PackError> {
    if m.files.len() > MAX_FILES || m.chunks.len() > MAX_CHUNKS || bytes_total(m) > MAX_TOTAL_RAW {
        return Err(PackError::Manifest("manifest exceeds format limits".into()));
    }
    for c in &m.chunks {
        if c.raw_len as u64 > MAX_RAW_CHUNK {
            return Err(PackError::Manifest("chunk raw_len exceeds limit".into()));
        }
    }
    let detected = detect(data)?;
    let (data_base, data_end) = match &detected {
        DetectedFormat::V1 { overlay, header } => {
            let end = overlay
                .checked_add(header.payload_len as usize)
                .ok_or(PackError::Truncated)?;
            if end > data.len() {
                return Err(PackError::Truncated);
            }
            (*overlay, end)
        }
        DetectedFormat::V0 { .. } => (0usize, data.len()),
    };
    let mut used = vec![false; m.chunks.len()];
    let mut bytes_checked = 0u64;
    let mut body_end = 0u64;
    for (fi, entry) in m.files.iter().enumerate() {
        if entry.size > MAX_FILE_SIZE {
            return Err(PackError::Manifest(format!("file {fi} exceeds size limit")));
        }
        let start = usize::try_from(entry.offset).map_err(|_| PackError::Truncated)? + data_base;
        let mut consumed = 0u32;
        let mut raw_this_file = 0u64;
        for i in 0..entry.chunk_count {
            let idx = (entry.chunk_start + i) as usize;
            let chunk = m.chunks.get(idx).ok_or_else(|| {
                PackError::Manifest(format!("file {fi} references missing chunk {idx}"))
            })?;
            let cstart = start
                .checked_add(consumed as usize)
                .ok_or(PackError::Truncated)?;
            let cend = cstart
                .checked_add(chunk.comp_len as usize)
                .ok_or(PackError::Truncated)?;
            if cend > data_end {
                return Err(PackError::Truncated);
            }
            let raw = decode_chunk(chunk.codec, &data[cstart..cend], chunk.raw_len as usize)?;
            if blake3::hash(&raw).as_bytes() != &chunk.blake3 {
                return Err(PackError::Hash(idx));
            }
            if crc32fast::hash(&raw) != chunk.crc32 {
                return Err(PackError::Crc);
            }
            used[idx] = true;
            consumed += chunk.comp_len;
            raw_this_file += raw.len() as u64;
            bytes_checked += raw.len() as u64;
            body_end = body_end.max(cend as u64);
        }
        if entry.chunk_count == 0 {
            if entry.size != 0 {
                return Err(PackError::Truncated);
            }
        } else if raw_this_file != entry.size {
            return Err(PackError::Truncated);
        }
    }
    for (idx, u) in used.iter().enumerate() {
        if !u {
            return Err(PackError::Manifest(format!(
                "chunk {idx} is not referenced"
            )));
        }
    }
    match &detected {
        DetectedFormat::V1 { overlay, header } => {
            let manifest_end = overlay
                .checked_add((header.manifest_offset + header.manifest_len) as usize)
                .ok_or(PackError::Truncated)?;
            if body_end > (overlay + header.manifest_offset as usize) as u64 {
                return Err(PackError::Truncated);
            }
            let _ = manifest_end;
        }
        DetectedFormat::V0 { footer } => {
            let manifest_end = footer
                .manifest_offset
                .checked_add(footer.manifest_len)
                .ok_or(PackError::Truncated)?;
            if manifest_end + FOOTER_SIZE != data.len() as u64 || body_end > footer.manifest_offset
            {
                return Err(PackError::Truncated);
            }
        }
    }
    Ok(IntegrityReport {
        chunks_checked: m.chunks.len(),
        bytes_checked,
    })
}

fn bytes_total(m: &PackManifest) -> u64 {
    m.files.iter().map(|f| f.size).sum()
}

fn decode_chunk(codec: u8, data: &[u8], raw_len: usize) -> Result<Vec<u8>, PackError> {
    match codec {
        CODEC_STORE => {
            if data.len() != raw_len {
                return Err(PackError::Truncated);
            }
            Ok(data.to_vec())
        }
        CODEC_LZMA2 => {
            use std::io::Read;
            let cap = raw_len.saturating_add(1);
            let mut reader = lzma_rust2::Lzma2Reader::new(
                data,
                lzma_rust2::LzmaOptions::DICT_SIZE_DEFAULT,
                None,
            );
            let mut out = Vec::with_capacity(raw_len);
            let mut limited = (&mut reader).take(cap as u64);
            limited.read_to_end(&mut out)?;
            if out.len() != raw_len {
                return Err(PackError::Truncated);
            }
            Ok(out)
        }
        CODEC_BROTLI => {
            use std::io::Read;
            let cap = raw_len.saturating_add(1);
            let mut reader = brotli::Decompressor::new(data, 4096);
            let mut out = Vec::with_capacity(raw_len);
            let mut limited = (&mut reader).take(cap as u64);
            limited.read_to_end(&mut out)?;
            if out.len() != raw_len {
                return Err(PackError::Truncated);
            }
            Ok(out)
        }
        other => Err(PackError::Codec(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppMeta, CODEC_BROTLI, CODEC_LZMA2, CODEC_STORE, CodecPolicy, PAYLOAD_FORMAT_VERSION,
        PackBuilder, PackError, extract_file, locate, read_manifest, verify_integrity,
    };

    fn sample() -> Vec<u8> {
        let mut b = PackBuilder::new(0);
        b.add_file("my-app.exe", b"fake exe bytes").unwrap();
        b.add_file("resources/icon.png", &[1, 2, 3, 4, 5]).unwrap();
        b.finish(&AppMeta::new("com.example.myapp", "My App", "1.4.2"))
    }

    fn repetitive(len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        let mut x: u64 = 0x0451;
        while out.len() < len {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let byte = (x >> 33) as u8;
            for _ in 0..7 {
                out.push(byte);
            }
        }
        out.truncate(len);
        out
    }

    fn incompressible(len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        let mut x: u64 = 0x9E3779B97F4A7C15;
        while out.len() < len {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            out.push((x >> 33) as u8);
        }
        out
    }

    #[test]
    fn header_roundtrip() {
        let data = sample();
        let (overlay, header) = locate(&data).unwrap();
        assert_eq!(overlay, 0);
        assert_eq!(header.format_version, PAYLOAD_FORMAT_VERSION);
        assert_eq!(header.payload_len as usize, data.len());
        assert!(header.manifest_offset < data.len() as u64);
    }

    #[test]
    fn manifest_roundtrip() {
        let data = sample();
        let m = read_manifest(&data).unwrap();
        assert_eq!(m.app_id, "com.example.myapp");
        assert_eq!(m.app_version, "1.4.2");
        assert_eq!(m.files.len(), 2);
        assert_eq!(m.files[0].path, "my-app.exe");
    }

    #[test]
    fn extract_roundtrip() {
        let data = sample();
        let m = read_manifest(&data).unwrap();
        let exe = extract_file(&data, &m, &m.files[0]).unwrap();
        assert_eq!(exe, b"fake exe bytes");
        let icon = extract_file(&data, &m, &m.files[1]).unwrap();
        assert_eq!(icon, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn corrupted_chunk_fails_hash() {
        let data = sample();
        let m = read_manifest(&data).unwrap();
        let mut corrupt = data.clone();
        let first = m.files[0].offset as usize;
        corrupt[first] ^= 0xff;
        match extract_file(&corrupt, &m, &m.files[0]) {
            Err(e @ (PackError::Hash(_) | PackError::Crc)) => {
                assert!(!e.to_string().is_empty());
            }
            other => panic!("expected integrity error, got {other:?}"),
        }
    }

    #[test]
    fn bad_magic_rejected() {
        let mut data = sample();
        data[0] = b'X';
        assert!(matches!(
            locate(&data),
            Err(PackError::BadMagic) | Err(PackError::Crc)
        ));
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut data = sample();
        let v = PAYLOAD_FORMAT_VERSION + 1;
        data[8..12].copy_from_slice(&v.to_le_bytes());
        let new_crc = crc32fast::hash(&data[0..56]);
        data[56..60].copy_from_slice(&new_crc.to_le_bytes());
        assert!(matches!(
            locate(&data),
            Err(PackError::UnsupportedVersion(_))
        ));
    }

    #[test]
    fn trailing_bytes_after_payload_tolerated() {
        let mut data = sample();
        data.extend_from_slice(b"AUTHENTICODE-CERT-TABLE-SIMULATION");
        let m = read_manifest(&data).unwrap();
        assert_eq!(m.files.len(), 2);
        assert!(verify_integrity(&data, &m).is_ok());
    }

    #[test]
    fn large_file_is_multi_chunk() {
        let big = repetitive(700 * 1024);
        let mut b = PackBuilder::new(0);
        b.add_file("assets/big.bin", &big).unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        assert!(m.files[0].chunk_count > 1);
        let out = extract_file(&data, &m, &m.files[0]).unwrap();
        assert_eq!(out, big);
    }

    #[test]
    fn empty_file_roundtrip() {
        let mut b = PackBuilder::new(0);
        b.add_file("empty.lock", b"").unwrap();
        b.add_file("real.txt", b"data").unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        assert_eq!(m.files[0].chunk_count, 0);
        assert_eq!(extract_file(&data, &m, &m.files[0]).unwrap(), b"");
        assert_eq!(extract_file(&data, &m, &m.files[1]).unwrap(), b"data");
    }

    #[test]
    fn auto_stores_incompressible() {
        let noise = incompressible(128 * 1024);
        let mut b = PackBuilder::new(0).codec(CodecPolicy::Auto);
        b.add_file("noise.bin", &noise).unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        let codecs: Vec<u8> = m.chunks.iter().map(|c| c.codec).collect();
        assert!(
            codecs.iter().all(|c| *c == CODEC_STORE),
            "codecs: {codecs:?}"
        );
    }

    #[test]
    fn auto_compresses_repetitive_with_lzma2() {
        let text = repetitive(300 * 1024);
        let mut b = PackBuilder::new(0).codec(CodecPolicy::Auto);
        b.add_file("text.bin", &text).unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        assert!(
            m.chunks.iter().any(|c| c.codec == CODEC_LZMA2),
            "expected lzma2 chunks"
        );
        assert!(data.len() < text.len());
        let out = extract_file(&data, &m, &m.files[0]).unwrap();
        assert_eq!(out, text);
    }

    #[test]
    fn forced_brotli_roundtrip() {
        let text = repetitive(300 * 1024);
        let mut b = PackBuilder::new(0).codec(CodecPolicy::Brotli);
        b.add_file("text.bin", &text).unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        assert!(
            m.chunks.iter().all(|c| c.codec == CODEC_BROTLI),
            "expected brotli chunks"
        );
        let out = extract_file(&data, &m, &m.files[0]).unwrap();
        assert_eq!(out, text);
    }

    #[test]
    fn forced_lzma2_roundtrip() {
        let text = repetitive(300 * 1024);
        let mut b = PackBuilder::new(0).codec(CodecPolicy::Lzma2);
        b.add_file("text.bin", &text).unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        assert!(
            m.chunks.iter().all(|c| c.codec == CODEC_LZMA2),
            "expected lzma2 chunks"
        );
        let out = extract_file(&data, &m, &m.files[0]).unwrap();
        assert_eq!(out, text);
    }

    #[test]
    fn deterministic_output() {
        let build = || {
            let mut b = PackBuilder::new(0);
            b.add_file("my-app.exe", b"fake exe bytes").unwrap();
            b.add_file("resources/icon.png", &[1, 2, 3, 4, 5]).unwrap();
            b.finish(&AppMeta::new("com.example.myapp", "My App", "1.4.2"))
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn duplicate_path_rejected() {
        let mut b = PackBuilder::new(0);
        b.add_file("a.txt", b"1").unwrap();
        assert!(b.add_file("a.txt", b"2").is_err());
    }

    #[test]
    fn integrity_ok_on_valid_package() {
        let big = repetitive(700 * 1024);
        let mut b = PackBuilder::new(0);
        b.add_file("big.bin", &big).unwrap();
        b.add_file("small.txt", b"hello").unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        let report = verify_integrity(&data, &m).unwrap();
        assert_eq!(report.chunks_checked, m.chunks.len());
        assert_eq!(report.bytes_checked, 700 * 1024 + 5);
    }

    #[test]
    fn integrity_detects_body_corruption() {
        let mut b = PackBuilder::new(0);
        b.add_file("data.bin", &repetitive(300 * 1024)).unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        let mut corrupt = data.clone();
        let mid = m.files[0].offset as usize + 100;
        corrupt[mid] ^= 0xff;
        assert!(verify_integrity(&corrupt, &m).is_err());
        assert!(verify_integrity(&data, &m).is_ok());
    }

    #[test]
    fn integrity_detects_manifest_corruption() {
        let mut b = PackBuilder::new(0);
        b.add_file("a.bin", b"aaaa").unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let (_, header) = locate(&data).unwrap();
        let mut corrupt = data.clone();
        let mi = header.manifest_offset as usize + 20;
        corrupt[mi] ^= 0xff;
        assert!(read_manifest(&corrupt).is_err());
    }

    #[test]
    fn integrity_rejects_wrong_size_layout() {
        let mut b = PackBuilder::new(0);
        b.add_file("a.bin", b"aaaa").unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        let mut truncated = data.clone();
        truncated.pop();
        assert!(verify_integrity(&truncated, &m).is_err());
        assert!(verify_integrity(&data, &m).is_ok());
    }

    #[test]
    fn chunk_layout_is_contiguous() {
        let big = repetitive(700 * 1024);
        let mut b = PackBuilder::new(0).codec(CodecPolicy::Store);
        b.add_file("big.bin", &big).unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        let m = read_manifest(&data).unwrap();
        let f = &m.files[0];
        assert!(f.chunk_count > 1);
        let mut body_pos = f.offset as usize;
        let mut raw_pos = 0usize;
        for i in 0..f.chunk_count {
            let c = &m.chunks[f.chunk_start as usize + i as usize];
            assert_eq!(
                &data[body_pos..body_pos + c.comp_len as usize],
                &big[raw_pos..raw_pos + c.raw_len as usize]
            );
            body_pos += c.comp_len as usize;
            raw_pos += c.raw_len as usize;
        }
        assert_eq!(raw_pos, big.len());
    }
}
