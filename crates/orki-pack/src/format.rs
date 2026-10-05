use crate::{MAGIC, PackError};

pub const HEADER_MAGIC: [u8; 8] = *b"ORKIHDR\0";
pub const SIGNATURE_MAGIC: [u8; 8] = *b"ORKISIG\0";
pub const PAYLOAD_FORMAT_VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 64;
pub const SIGNATURE_BLOCK_SIZE: usize = 128;

pub const FLAG_SIGNED: u32 = 1 << 0;
pub const FLAG_HAS_STRING_TABLE: u32 = 1 << 1;
pub const FLAG_HAS_ASSETS: u32 = 1 << 2;
pub const FLAG_HAS_UNINSTALLER: u32 = 1 << 3;

pub const MAX_FILES: usize = 100_000;
pub const MAX_CHUNKS: usize = 400_000;
pub const MAX_ASSETS: usize = 10_000;
pub const MAX_PATH_UTF16: usize = 1024;
pub const MAX_RAW_CHUNK: u64 = 16 * 1024 * 1024;
pub const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_TOTAL_RAW: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_MANIFEST_LEN: u64 = 256 * 1024 * 1024;

const PE_DOS_MAGIC: [u8; 2] = *b"MZ";
const PE_NT_MAGIC_OFFSET: usize = 0x3c;
const PE_NT_MAGIC: [u8; 4] = *b"PE\0\0";
const COFF_HEADER_SIZE: usize = 20;
const SECTION_ENTRY_SIZE: usize = 40;
const SECTION_RAW_SIZE_OFF: usize = 16;
const SECTION_RAW_PTR_OFF: usize = 20;

pub struct PayloadHeader {
    pub format_version: u32,
    pub payload_len: u64,
    pub manifest_offset: u64,
    pub manifest_len: u64,
    pub manifest_crc32: u32,
    pub flags: u32,
    pub string_table_offset: u64,
}

pub fn overlay_start(data: &[u8]) -> usize {
    if data.len() < 0x40 || data[0..2] != PE_DOS_MAGIC {
        return 0;
    }
    let nt = match read_u32(data, PE_NT_MAGIC_OFFSET) {
        Some(v) => v as usize,
        None => return 0,
    };
    if nt == 0 || nt + 4 > data.len() || data[nt..nt + 4] != PE_NT_MAGIC {
        return 0;
    }
    let num_sections = match read_u16(data, nt + 4 + 2) {
        Some(v) => v as usize,
        None => return 0,
    };
    let opt_size = match read_u16(data, nt + 4 + 16) {
        Some(v) => v as usize,
        None => return 0,
    };
    let table = nt + 4 + COFF_HEADER_SIZE + opt_size;
    let mut end = 0usize;
    for i in 0..num_sections {
        let off = table + i * SECTION_ENTRY_SIZE;
        let raw_size = match read_u32(data, off + SECTION_RAW_SIZE_OFF) {
            Some(v) => v as usize,
            None => return 0,
        };
        let raw_ptr = match read_u32(data, off + SECTION_RAW_PTR_OFF) {
            Some(v) => v as usize,
            None => return 0,
        };
        if raw_size > 0 && raw_ptr.checked_add(raw_size).is_some() {
            end = end.max(raw_ptr + raw_size);
        }
    }
    if end == 0 || end > data.len() {
        data.len()
    } else {
        end
    }
}

fn read_u16(data: &[u8], off: usize) -> Option<u16> {
    if off + 2 > data.len() {
        return None;
    }
    Some(u16::from_le_bytes([data[off], data[off + 1]]))
}

fn read_u32(data: &[u8], off: usize) -> Option<u32> {
    if off + 4 > data.len() {
        return None;
    }
    Some(u32::from_le_bytes([
        data[off],
        data[off + 1],
        data[off + 2],
        data[off + 3],
    ]))
}

pub fn write_header(
    payload_len: u64,
    manifest_offset: u64,
    manifest_len: u64,
    manifest_crc32: u32,
    flags: u32,
    string_table_offset: u64,
) -> [u8; HEADER_SIZE] {
    let mut h = [0u8; HEADER_SIZE];
    h[0..8].copy_from_slice(&HEADER_MAGIC);
    h[8..12].copy_from_slice(&PAYLOAD_FORMAT_VERSION.to_le_bytes());
    h[12..16].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
    h[16..24].copy_from_slice(&payload_len.to_le_bytes());
    h[24..32].copy_from_slice(&manifest_offset.to_le_bytes());
    h[32..40].copy_from_slice(&manifest_len.to_le_bytes());
    h[40..44].copy_from_slice(&manifest_crc32.to_le_bytes());
    h[44..48].copy_from_slice(&flags.to_le_bytes());
    h[48..56].copy_from_slice(&string_table_offset.to_le_bytes());
    let crc = crc32fast::hash(&h[0..56]);
    h[56..60].copy_from_slice(&crc.to_le_bytes());
    h
}

pub fn read_payload_header(data: &[u8], overlay: usize) -> Result<PayloadHeader, PackError> {
    let start = overlay;
    let hend = start.checked_add(HEADER_SIZE).ok_or(PackError::Truncated)?;
    if hend > data.len() {
        return Err(PackError::Truncated);
    }
    let h = &data[start..hend];
    if h[0..8] != HEADER_MAGIC {
        return Err(PackError::BadMagic);
    }
    let header_crc = u32::from_le_bytes([h[56], h[57], h[58], h[59]]);
    if crc32fast::hash(&h[0..56]) != header_crc {
        return Err(PackError::Crc);
    }
    let version = u32::from_le_bytes([h[8], h[9], h[10], h[11]]);
    if version != PAYLOAD_FORMAT_VERSION {
        return Err(PackError::UnsupportedVersion(version));
    }
    let u64_at = |off: usize| {
        u64::from_le_bytes([
            h[off],
            h[off + 1],
            h[off + 2],
            h[off + 3],
            h[off + 4],
            h[off + 5],
            h[off + 6],
            h[off + 7],
        ])
    };
    let u32_at = |off: usize| u32::from_le_bytes([h[off], h[off + 1], h[off + 2], h[off + 3]]);
    let header_size = u32_at(12) as usize;
    if header_size != HEADER_SIZE {
        return Err(PackError::Truncated);
    }
    Ok(PayloadHeader {
        format_version: version,
        payload_len: u64_at(16),
        manifest_offset: u64_at(24),
        manifest_len: u64_at(32),
        manifest_crc32: u32_at(40),
        flags: u32_at(44),
        string_table_offset: u64_at(48),
    })
}

pub fn locate(data: &[u8]) -> Result<(usize, PayloadHeader), PackError> {
    let overlay = overlay_start(data);
    let header = read_payload_header(data, overlay)?;
    let payload_end = overlay
        .checked_add(header.payload_len as usize)
        .ok_or(PackError::Truncated)?;
    if payload_end > data.len() {
        return Err(PackError::Truncated);
    }
    Ok((overlay, header))
}

pub fn footer_v0(data: &[u8]) -> Result<crate::Footer, PackError> {
    if data.len() < FOOTER_SIZE_V0 {
        return Err(PackError::Truncated);
    }
    let f = &data[data.len() - FOOTER_SIZE_V0..];
    if f[0..8] != MAGIC {
        return Err(PackError::BadMagic);
    }
    let header_crc = u32::from_le_bytes([f[56], f[57], f[58], f[59]]);
    if crc32fast::hash(&f[0..56]) != header_crc {
        return Err(PackError::Crc);
    }
    Ok(crate::Footer {
        format_version: u32::from_le_bytes([f[8], f[9], f[10], f[11]]),
        manifest_offset: u64::from_le_bytes([
            f[12], f[13], f[14], f[15], f[16], f[17], f[18], f[19],
        ]),
        manifest_len: u64::from_le_bytes([f[20], f[21], f[22], f[23], f[24], f[25], f[26], f[27]]),
        manifest_crc32: u32::from_le_bytes([f[28], f[29], f[30], f[31]]),
    })
}

const FOOTER_SIZE_V0: usize = 64;
