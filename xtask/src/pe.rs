struct Section {
    virt_addr: u32,
    virt_size: u32,
    raw_ptr: u32,
    raw_size: u32,
}

pub fn import_dlls(data: &[u8]) -> Result<Vec<String>, String> {
    let pe = dos_header(data)?;
    check_sig(data, pe)?;
    let coff = pe + 4;
    let num_sections = read_u16(data, coff + 2)?;
    let size_opt = read_u16(data, coff + 16)? as usize;
    let opt = coff + 20;
    let magic = read_u16(data, opt)?;
    let (dirs_base, num_dirs_off, image_base_off) = match magic {
        0x20b => (opt + 112, opt + 108, Some(opt + 24)),
        0x10b => (opt + 96, opt + 92, Some(opt + 28)),
        m => return Err(format!("unsupported optional header magic {m:#06x}")),
    };
    let num_dirs = read_u32(data, num_dirs_off)? as usize;
    let image_base = match image_base_off {
        Some(off) if magic == 0x20b => read_u64(data, off)?,
        Some(off) => u64::from(read_u32(data, off)?),
        None => 0,
    };
    let sections = parse_sections(data, opt + size_opt, num_sections as usize)?;
    let mut names = Vec::new();
    if num_dirs > 1 {
        let (rva, _) = data_dir(data, dirs_base, 1)?;
        read_import_dir(data, rva, &sections, &mut names)?;
    }
    if num_dirs > 13 {
        let (rva, _) = data_dir(data, dirs_base, 13)?;
        read_delay_dir(data, rva, image_base, &sections, &mut names)?;
    }
    Ok(names)
}

fn dos_header(data: &[u8]) -> Result<usize, String> {
    if data.len() < 0x40 || &data[0..2] != b"MZ" {
        return Err("not an MZ image".to_string());
    }
    Ok(read_u32(data, 0x3c)? as usize)
}

fn check_sig(data: &[u8], pe: usize) -> Result<(), String> {
    if data.get(pe..pe + 4) != Some(b"PE\0\0".as_slice()) {
        return Err("missing PE signature".to_string());
    }
    Ok(())
}

fn read_u16(data: &[u8], off: usize) -> Result<u16, String> {
    let b = data
        .get(off..off + 2)
        .ok_or_else(|| format!("truncated at {off:#x}"))?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32, String> {
    let b = data
        .get(off..off + 4)
        .ok_or_else(|| format!("truncated at {off:#x}"))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_u64(data: &[u8], off: usize) -> Result<u64, String> {
    let b = data
        .get(off..off + 8)
        .ok_or_else(|| format!("truncated at {off:#x}"))?;
    let mut a = [0u8; 8];
    a.copy_from_slice(b);
    Ok(u64::from_le_bytes(a))
}

fn data_dir(data: &[u8], base: usize, index: usize) -> Result<(u32, u32), String> {
    let off = base + index * 8;
    Ok((read_u32(data, off)?, read_u32(data, off + 4)?))
}

fn parse_sections(data: &[u8], table: usize, count: usize) -> Result<Vec<Section>, String> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let off = table + i * 40;
        out.push(Section {
            virt_size: read_u32(data, off + 8)?,
            virt_addr: read_u32(data, off + 12)?,
            raw_size: read_u32(data, off + 16)?,
            raw_ptr: read_u32(data, off + 20)?,
        });
    }
    Ok(out)
}

fn rva_to_off(rva: u32, sections: &[Section]) -> Option<usize> {
    for s in sections {
        let span = s.virt_size.max(s.raw_size);
        let end = s.virt_addr.saturating_add(span);
        if s.raw_size > 0 && rva >= s.virt_addr && rva < end {
            return Some((s.raw_ptr + (rva - s.virt_addr)) as usize);
        }
    }
    None
}

fn read_cstr(data: &[u8], off: usize) -> Option<String> {
    let rest = data.get(off..)?;
    let end = rest.iter().position(|&b| b == 0)?;
    Some(String::from_utf8_lossy(&rest[..end]).into_owned())
}

fn read_import_dir(
    data: &[u8],
    rva: u32,
    sections: &[Section],
    out: &mut Vec<String>,
) -> Result<(), String> {
    let Some(mut off) = rva_to_off(rva, sections) else {
        return Ok(());
    };
    for _ in 0..4096 {
        let oft = read_u32(data, off)?;
        let name_rva = read_u32(data, off + 12)?;
        let ft = read_u32(data, off + 16)?;
        if oft == 0 && name_rva == 0 && ft == 0 {
            break;
        }
        if name_rva != 0
            && let Some(noff) = rva_to_off(name_rva, sections)
            && let Some(name) = read_cstr(data, noff)
        {
            out.push(name);
        }
        off += 20;
    }
    Ok(())
}

fn read_delay_dir(
    data: &[u8],
    rva: u32,
    image_base: u64,
    sections: &[Section],
    out: &mut Vec<String>,
) -> Result<(), String> {
    let Some(mut off) = rva_to_off(rva, sections) else {
        return Ok(());
    };
    for _ in 0..4096 {
        let attrs = read_u32(data, off)?;
        let sz_name = read_u32(data, off + 4)?;
        if attrs == 0 && sz_name == 0 {
            break;
        }
        let name_addr = if attrs & 1 != 0 {
            sz_name
        } else {
            (sz_name as u64).wrapping_sub(image_base) as u32
        };
        if let Some(noff) = rva_to_off(name_addr, sections)
            && let Some(name) = read_cstr(data, noff)
        {
            out.push(name);
        }
        off += 32;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::import_dlls;

    fn tiny_pe32plus() -> Vec<u8> {
        let mut b = vec![0u8; 0x1000];
        b[0] = b'M';
        b[1] = b'Z';
        b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        let pe = 0x80usize;
        b[pe..pe + 4].copy_from_slice(b"PE\0\0");
        let coff = pe + 4;
        b[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes());
        b[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
        b[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes());
        let opt = coff + 20;
        b[opt..opt + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        b[opt + 24..opt + 32].copy_from_slice(&0x140000000u64.to_le_bytes());
        b[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes());
        let dir1 = opt + 112 + 8;
        b[dir1..dir1 + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        b[dir1 + 4..dir1 + 8].copy_from_slice(&40u32.to_le_bytes());
        let sec = opt + 240;
        b[sec..sec + 8].copy_from_slice(b".idata\0\0");
        b[sec + 8..sec + 12].copy_from_slice(&0x1000u32.to_le_bytes());
        b[sec + 12..sec + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        b[sec + 16..sec + 20].copy_from_slice(&0x1000u32.to_le_bytes());
        b[sec + 20..sec + 24].copy_from_slice(&0x1000u32.to_le_bytes());
        b.resize(0x1100, 0);
        b[0x1000 + 12..0x1000 + 16].copy_from_slice(&0x1020u32.to_le_bytes());
        b[0x1020..0x1029].copy_from_slice(b"KERNEL32\0");
        b
    }

    #[test]
    fn parses_import_names() {
        let names = import_dlls(&tiny_pe32plus()).expect("parse ok");
        assert_eq!(names, vec!["KERNEL32"]);
    }

    #[test]
    fn rejects_non_pe() {
        assert!(import_dlls(&[0u8; 64]).is_err());
    }
}
