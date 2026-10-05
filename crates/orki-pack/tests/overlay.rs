use orki_pack::{AppMeta, PackBuilder, extract_file, locate, read_manifest, verify_integrity};

fn synthetic_pe(sections_end: usize) -> Vec<u8> {
    let mut pe = vec![0u8; sections_end];
    pe[0..2].copy_from_slice(b"MZ");
    let nt = 0x80usize;
    pe[0x3c..0x40].copy_from_slice(&(nt as u32).to_le_bytes());
    pe[nt..nt + 4].copy_from_slice(b"PE\0\0");
    let num_sections: u16 = 1;
    pe[nt + 6..nt + 8].copy_from_slice(&num_sections.to_le_bytes());
    let opt_size: u16 = 0;
    pe[nt + 22..nt + 24].copy_from_slice(&opt_size.to_le_bytes());
    let table = nt + 24;
    let raw_size = (sections_end / 2) as u32;
    let raw_ptr = (sections_end / 2) as u32;
    pe[table + 16..table + 20].copy_from_slice(&raw_size.to_le_bytes());
    pe[table + 20..table + 24].copy_from_slice(&raw_ptr.to_le_bytes());
    pe
}

#[test]
fn overlay_on_stub_bytes_roundtrip() {
    let stub = synthetic_pe(4096);
    let mut b = PackBuilder::new(0);
    b.add_file("app/main.exe", b"MZ binary").unwrap();
    b.add_file("app/config.toml", b"schema = 1\n").unwrap();
    let mut data = stub;
    data.extend_from_slice(&b.finish(&AppMeta::new("com.example.app", "App", "0.1.0")));

    let (overlay, header) = locate(&data).unwrap();
    assert_eq!(overlay, 4096);
    assert_eq!(header.payload_len as usize, data.len() - 4096);
    let m = read_manifest(&data).unwrap();
    assert_eq!(m.files.len(), 2);
    assert_eq!(extract_file(&data, &m, &m.files[0]).unwrap(), b"MZ binary");
    assert_eq!(
        extract_file(&data, &m, &m.files[1]).unwrap(),
        b"schema = 1\n"
    );
    assert!(verify_integrity(&data, &m).is_ok());
}

#[test]
fn trailing_authenticode_table_tolerated_on_overlay() {
    let stub = synthetic_pe(2048);
    let mut b = PackBuilder::new(0);
    b.add_file("app/app.exe", b"payload").unwrap();
    let mut data = stub;
    data.extend_from_slice(&b.finish(&AppMeta::new("a", "a", "0.1.0")));
    data.extend_from_slice(&[0xAA; 512]);
    let m = read_manifest(&data).unwrap();
    assert_eq!(extract_file(&data, &m, &m.files[0]).unwrap(), b"payload");
    assert!(verify_integrity(&data, &m).is_ok());
}

#[test]
fn payload_only_has_zero_overlay() {
    let mut b = PackBuilder::new(0);
    b.add_file("a.txt", b"hello").unwrap();
    let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
    let (overlay, _) = locate(&data).unwrap();
    assert_eq!(overlay, 0);
    let m = read_manifest(&data).unwrap();
    assert_eq!(extract_file(&data, &m, &m.files[0]).unwrap(), b"hello");
}
