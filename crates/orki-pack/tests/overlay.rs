use orki_pack::{AppMeta, PackBuilder, extract_file, read_footer, read_manifest};

#[test]
fn overlay_on_stub_bytes_roundtrip() {
    let mut stub = vec![0u8; 4096];
    for (i, b) in stub.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    let mut b = PackBuilder::new(stub.len() as u64);
    b.add_file("app/main.exe", b"MZ binary").unwrap();
    b.add_file("app/config.toml", b"schema = 1\n").unwrap();
    let mut data = stub.clone();
    data.extend_from_slice(&b.finish(&AppMeta::new("com.example.app", "App", "0.1.0")));

    let f = read_footer(&data).unwrap();
    assert!(f.manifest_offset >= 4096);
    let m = read_manifest(&data).unwrap();
    assert_eq!(m.files.len(), 2);
    assert_eq!(extract_file(&data, &m, &m.files[0]).unwrap(), b"MZ binary");
    assert_eq!(
        extract_file(&data, &m, &m.files[1]).unwrap(),
        b"schema = 1\n"
    );
    assert_eq!(&data[..8], &stub[..8]);
}
