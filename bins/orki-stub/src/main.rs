use std::path::PathBuf;
use std::process::exit;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Silent,
    Passive,
    Uninstall,
    Repair,
    Modify,
    List,
    Extract(PathBuf),
    PrintConfig,
    Version,
    Help,
}

#[derive(Debug, Default)]
struct Args {
    mode: Option<Mode>,
    dir: Option<PathBuf>,
    lang: Option<String>,
    log: Option<PathBuf>,
    dry_run: bool,
    no_restart: bool,
    force_close: bool,
    verify: bool,
    components: Vec<String>,
    props: Vec<(String, String)>,
    positional: Vec<String>,
    bad: bool,
}

fn parse_args(argv: &[String]) -> Args {
    let mut a = Args::default();
    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        let arg_norm = arg.trim_start_matches('/');
        match arg_norm {
            "S" | "-silent" | "--silent" => a.mode = Some(Mode::Silent),
            "P" | "-passive" | "--passive" => a.mode = Some(Mode::Passive),
            "UPDATE" => {}
            "R" | "-restart" | "--restart" => {}
            "D" => a.bad = true,
            d if d.starts_with("D=") => a.dir = Some(PathBuf::from(&d[2..])),
            "-dir" | "--dir" => match it.next() {
                Some(v) => a.dir = Some(PathBuf::from(v)),
                None => a.bad = true,
            },
            "-uninstall" | "--uninstall" => a.mode = Some(Mode::Uninstall),
            "-repair" | "--repair" => a.mode = Some(Mode::Repair),
            "-modify" | "--modify" => a.mode = Some(Mode::Modify),
            "-list" | "--list" => a.mode = Some(Mode::List),
            "-extract" | "--extract" => match it.next() {
                Some(v) => a.mode = Some(Mode::Extract(PathBuf::from(v))),
                None => a.bad = true,
            },
            "-print-config" | "--print-config" => a.mode = Some(Mode::PrintConfig),
            "-version" | "--version" | "V" | "-v" => a.mode = Some(Mode::Version),
            "-help" | "--help" | "h" | "?" => a.mode = Some(Mode::Help),
            "-lang" | "--lang" => match it.next() {
                Some(v) => a.lang = Some(v.clone()),
                None => a.bad = true,
            },
            "-log" | "--log" => match it.next() {
                Some(v) => a.log = Some(PathBuf::from(v)),
                None => a.bad = true,
            },
            "-dry-run" | "--dry-run" => a.dry_run = true,
            "-no-restart" | "--no-restart" => a.no_restart = true,
            "-force-close" | "--force-close" => a.force_close = true,
            "-verify" | "--verify" => a.verify = true,
            "-components" | "--components" => match it.next() {
                Some(v) => {
                    a.components = v
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                }
                None => a.bad = true,
            },
            _ => {
                if let Some((k, v)) = arg.split_once('=') {
                    a.props.push((k.to_string(), v.to_string()));
                } else {
                    a.positional.push(arg.clone());
                }
            }
        }
    }
    a
}
fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = parse_args(&argv);
    exit(dispatch(&args));
}

#[cfg(feature = "gui")]
fn run_gui() -> i32 {
    match orki_ui::shader_app::run() {
        Ok(()) => 0,
        Err(_) => {
            eprintln!("gpu/ui unavailable, falling back to silent");
            1603
        }
    }
}

#[cfg(not(feature = "gui"))]
fn run_gui() -> i32 {
    eprintln!("gui not available in this variant");
    1603
}

fn dispatch(args: &Args) -> i32 {
    if args.bad {
        eprintln!("missing value for option");
        return 1603;
    }
    match &args.mode {
        Some(Mode::Version) => {
            println!("orki-stub {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some(Mode::Help) => {
            print_usage();
            0
        }
        Some(Mode::List) => match read_self_manifest() {
            Ok(m) => {
                for f in &m.files {
                    println!("{}", f.path);
                }
                0
            }
            Err(e) => {
                eprintln!("{e}");
                if e.contains("ORKI-1002") { 1621 } else { 1620 }
            }
        },
        Some(Mode::PrintConfig) => match read_self_manifest() {
            Ok(m) => {
                println!("app.id = {}", m.app_id);
                println!("app.name = {}", m.app_name);
                println!("app.version = {}", m.app_version);
                println!("files = {}", m.files.len());
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1620
            }
        },
        Some(Mode::Extract(dir)) => match extract_to(dir) {
            Ok(n) => {
                println!("extracted {n} files to {}", dir.display());
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1620
            }
        },
        Some(_) | None => match read_self_manifest() {
            Ok(m) => {
                if m.files.is_empty() {
                    return 1620;
                }
                println!("app: {} {} ({})", m.app_name, m.app_version, m.app_id);
                run_gui()
            }
            Err(e) => {
                eprintln!("{e}");
                if e.contains("ORKI-1002") { 1621 } else { 1620 }
            }
        },
    }
}

fn print_usage() {
    println!("orki-stub installer runtime");
    println!();
    println!("  /S, --silent      silent install");
    println!("  /P, --passive     progress-only install");
    println!("  /D=<dir>          install directory");
    println!("  --dir <dir>       install directory");
    println!("  --uninstall       remove installation");
    println!("  --repair          repair installation");
    println!("  --modify          modify components");
    println!("  --list            list payload files");
    println!("  --extract <dir>   extract payload");
    println!("  --print-config    print manifest summary");
    println!("  --verify          verify payload integrity");
    println!("  --lang <code>     ui language");
    println!("  --log <file>      log file");
    println!("  --dry-run         plan only, no writes");
    println!("  --version         print version");
}

fn read_self_manifest() -> Result<orki_pack::PackManifest, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate own executable: {e}"))?;
    let data = std::fs::read(&exe).map_err(|e| format!("cannot read {}: {e}", exe.display()))?;
    check_signature(&data)?;
    orki_pack::read_manifest(&data).map_err(|e| format!("bad package: {e}"))
}

fn trusted_keys() -> Vec<orki_pack::TrustedKey> {
    let hex = match option_env!("ORKI_TRUSTED_PUBKEY") {
        Some(h) => h,
        None => return Vec::new(),
    };
    let mut key = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        if chunk.len() != 2 || i >= 32 {
            return Vec::new();
        }
        let hi = (chunk[0] as char).to_digit(16).unwrap_or(16) as u8;
        let lo = (chunk[1] as char).to_digit(16).unwrap_or(16) as u8;
        if hi > 15 || lo > 15 {
            return Vec::new();
        }
        key[i] = (hi << 4) | lo;
    }
    vec![orki_pack::TrustedKey {
        slot: 0,
        pubkey: key,
    }]
}

fn check_signature(data: &[u8]) -> Result<(), String> {
    match orki_pack::verify_payload_signature(data, &trusted_keys()) {
        Ok(()) => Ok(()),
        Err(orki_pack::PackError::UnsignedPayload) => {
            if cfg!(debug_assertions) {
                Ok(())
            } else {
                Err("signature verification failed (ORKI-1002): unsigned payload".to_string())
            }
        }
        Err(orki_pack::PackError::UnknownKeySlot(slot)) => Err(format!(
            "signature verification failed (ORKI-1002): key slot {slot} is not trusted"
        )),
        Err(orki_pack::PackError::SignatureMismatch) => {
            Err("signature verification failed (ORKI-1002)".to_string())
        }
        Err(e) => Err(format!("bad package: {e}")),
    }
}

fn extract_to(dir: &std::path::Path) -> Result<usize, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate own executable: {e}"))?;
    let data = std::fs::read(&exe).map_err(|e| format!("cannot read {}: {e}", exe.display()))?;
    let m = orki_pack::read_manifest(&data).map_err(|e| format!("bad package: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let mut count = 0usize;
    for f in &m.files {
        let bytes =
            orki_pack::extract_file(&data, &m, f).map_err(|e| format!("{}: {e}", f.path))?;
        let rel =
            orki_core::paths::sanitize_rel_path(&f.path).map_err(|e| format!("{}: {e}", f.path))?;
        let target = dir.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&target, &bytes).map_err(|e| format!("{}: {e}", target.display()))?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::{Mode, parse_args};
    use orki_pack::{AppMeta, PackBuilder};

    fn sample_payload() -> Vec<u8> {
        let mut b = PackBuilder::new(0);
        b.add_file("my-app.exe", b"app").unwrap();
        b.finish(&AppMeta::new("com.example.app", "App", "0.1.0"))
    }

    #[test]
    fn parses_nsis_style_flags() {
        let argv: Vec<String> = ["/S", "/D=C:\\Program Files\\App", "/UPDATE"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let a = parse_args(&argv);
        assert_eq!(a.mode, Some(Mode::Silent));
        assert_eq!(
            a.dir.as_deref(),
            Some(std::path::Path::new("C:\\Program Files\\App"))
        );
        assert!(a.props.is_empty());
    }

    #[test]
    fn parses_gnu_style_flags() {
        let argv: Vec<String> = [
            "--passive",
            "--lang",
            "pl",
            "--dry-run",
            "KEY=VALUE",
            "extra.txt",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let a = parse_args(&argv);
        assert_eq!(a.mode, Some(Mode::Passive));
        assert_eq!(a.lang.as_deref(), Some("pl"));
        assert!(a.dry_run);
        assert_eq!(a.props, vec![("KEY".to_string(), "VALUE".to_string())]);
        assert_eq!(a.positional, vec!["extra.txt"]);
    }

    #[test]
    fn missing_value_is_bad() {
        let a = parse_args(&["--dir".to_string()]);
        assert!(a.bad);
        let a = parse_args(&["/D".to_string()]);
        assert!(a.bad);
    }

    #[test]
    fn tauri_updater_flags_accepted() {
        let argv: Vec<String> = ["/P", "/R"].iter().map(|s| s.to_string()).collect();
        let a = parse_args(&argv);
        assert_eq!(a.mode, Some(Mode::Passive));
    }

    #[test]
    fn manifest_from_attached_payload() {
        let payload = sample_payload();
        let m = orki_pack::read_manifest(&payload).expect("valid");
        assert_eq!(m.app_id, "com.example.app");
        assert_eq!(m.files.len(), 1);
    }
}
