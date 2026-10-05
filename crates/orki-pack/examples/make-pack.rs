use std::path::PathBuf;

use orki_pack::{AppMeta, PackBuilder};
fn main() {
    let mut args = std::env::args().skip(1);
    let stub = args.next().map(PathBuf::from).expect("stub path required");
    let out = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| stub.clone());

    let mut b = PackBuilder::new(0);
    b.add_file("bin/hello.txt", b"orki smoke payload\n")
        .unwrap();
    let payload = b.finish(&AppMeta::new("com.orki.smoke", "Orki Smoke", "0.0.1"));

    let mut bin = std::fs::read(&stub).expect("stub bytes");
    bin.extend_from_slice(&payload);
    std::fs::write(&out, bin).expect("write installer");
    println!("installer: {}", out.display());
}
