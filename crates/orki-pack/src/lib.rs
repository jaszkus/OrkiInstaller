pub const MAGIC: [u8; 8] = *b"ORKIPACK";
pub const FORMAT_VERSION: u32 = 1;
pub const FOOTER_SIZE: u64 = 64;

pub const CODEC_STORE: u8 = 0;
pub const CODEC_LZMA2: u8 = 1;
pub const CODEC_BROTLI: u8 = 2;
pub const CODEC_ZSTD: u8 = 3;

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
    base: u64,
    body: Vec<u8>,
    chunks: Vec<Chunk>,
    files: Vec<FileEntry>,
}

impl Default for PackBuilder {
    fn default() -> Self {
        Self::new(0)
    }
}

impl PackBuilder {
    pub fn new(base: u64) -> Self {
        Self {
            base,
            body: Vec::new(),
            chunks: Vec::new(),
            files: Vec::new(),
        }
    }

    pub fn add_file(&mut self, rel_path: &str, data: &[u8]) -> Result<(), PackError> {
        let rel = orki_core::paths::sanitize_rel_path(rel_path)
            .map_err(|e| PackError::Manifest(e.to_string()))?;
        let path = rel.to_string_lossy().replace('\\', "/");
        if self.files.iter().any(|f| f.path == path) {
            return Err(PackError::Manifest(format!("duplicate path: {path}")));
        }
        let chunk_start = self.chunks.len() as u32;
        let offset = self.base + self.body.len() as u64;
        let hash = blake3::hash(data);
        let crc = crc32fast::hash(data);
        self.body.extend_from_slice(data);
        self.chunks.push(Chunk {
            codec: CODEC_STORE,
            comp_len: data.len() as u32,
            raw_len: data.len() as u32,
            blake3: hash.into(),
            crc32: crc,
        });
        self.files.push(FileEntry {
            path,
            offset,
            size: data.len() as u64,
            chunk_start,
            chunk_count: 1,
        });
        Ok(())
    }

    pub fn finish(self, meta: &AppMeta) -> Vec<u8> {
        let mut out = self.body;
        let manifest = PackManifest {
            schema: 1,
            app_id: meta.id.clone(),
            app_name: meta.name.clone(),
            app_version: meta.version.clone(),
            files: self.files,
            chunks: self.chunks,
        };
        let manifest_offset = self.base + out.len() as u64;
        let encoded = postcard::to_allocvec(&manifest).expect("postcard encode");
        let manifest_crc32 = crc32fast::hash(&encoded);
        let manifest_len = encoded.len() as u64;
        out.extend_from_slice(&encoded);

        let mut footer = [0u8; 64];
        footer[0..8].copy_from_slice(&MAGIC);
        footer[8..12].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        footer[12..20].copy_from_slice(&manifest_offset.to_le_bytes());
        footer[20..28].copy_from_slice(&manifest_len.to_le_bytes());
        footer[28..32].copy_from_slice(&manifest_crc32.to_le_bytes());
        let header_crc = crc32fast::hash(&footer[0..56]);
        footer[56..60].copy_from_slice(&header_crc.to_le_bytes());
        out.extend_from_slice(&footer);
        out
    }
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
    let footer = read_footer(data)?;
    let start = usize::try_from(footer.manifest_offset).map_err(|_| PackError::Truncated)?;
    let end =
        start.checked_add(usize::try_from(footer.manifest_len).map_err(|_| PackError::Truncated)?);
    let end = end.ok_or(PackError::Truncated)?;
    if end > data.len() {
        return Err(PackError::Truncated);
    }
    let encoded = &data[start..end];
    if crc32fast::hash(encoded) != footer.manifest_crc32 {
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
    let start = usize::try_from(entry.offset).map_err(|_| PackError::Truncated)?;
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

fn decode_chunk(codec: u8, data: &[u8], raw_len: usize) -> Result<Vec<u8>, PackError> {
    match codec {
        CODEC_STORE => {
            if data.len() != raw_len {
                return Err(PackError::Truncated);
            }
            Ok(data.to_vec())
        }
        other => Err(PackError::Codec(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::{AppMeta, PackBuilder, PackError, extract_file, read_footer, read_manifest};

    fn sample() -> Vec<u8> {
        let mut b = PackBuilder::new(0);
        b.add_file("my-app.exe", b"fake exe bytes").unwrap();
        b.add_file("resources/icon.png", &[1, 2, 3, 4, 5]).unwrap();
        b.finish(&AppMeta::new("com.example.myapp", "My App", "1.4.2"))
    }

    #[test]
    fn footer_roundtrip() {
        let data = sample();
        let f = read_footer(&data).unwrap();
        assert_eq!(f.format_version, 1);
        assert!(f.manifest_offset < data.len() as u64);
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
        let mut data = sample();
        data[2] ^= 0xff;
        let m = read_manifest(&data).unwrap();
        match extract_file(&data, &m, &m.files[0]) {
            Err(e @ (PackError::Hash(_) | PackError::Crc)) => {
                assert!(!e.to_string().is_empty());
            }
            other => panic!("expected integrity error, got {other:?}"),
        }
    }

    #[test]
    fn bad_magic_rejected() {
        let mut data = sample();
        let len = data.len();
        data[len - 64] = b'X';
        assert!(matches!(read_footer(&data), Err(PackError::BadMagic)));
    }

    #[test]
    fn base_offset_shifts_positions() {
        let stub = vec![0u8; 1000];
        let mut b = PackBuilder::new(stub.len() as u64);
        b.add_file("a.txt", b"hello").unwrap();
        let mut data = stub.clone();
        data.extend_from_slice(&b.finish(&AppMeta::new("a", "a", "0.1.0")));
        let f = read_footer(&data).unwrap();
        assert!(f.manifest_offset >= 1000);
        let m = read_manifest(&data).unwrap();
        assert_eq!(extract_file(&data, &m, &m.files[0]).unwrap(), b"hello");
    }
}
