use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("empty path")]
    Empty,
    #[error("absolute paths are not allowed in the payload: {0}")]
    Absolute(String),
    #[error("parent references are not allowed: {0}")]
    ParentRef(String),
    #[error("reserved device name: {0}")]
    ReservedName(String),
    #[error("ADS streams are not allowed: {0}")]
    AdsStream(String),
    #[error("invalid character in path: {0}")]
    InvalidChar(char),
    #[error("component must not end with a dot or a space: {0}")]
    TrailingDotSpace(String),
    #[error("component too long: {0}")]
    ComponentTooLong(String),
}

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn is_reserved_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component);
    RESERVED.iter().any(|r| stem.eq_ignore_ascii_case(r))
}

pub fn sanitize_rel_path(input: &str) -> Result<PathBuf, PathError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(PathError::Empty);
    }
    if input.starts_with('/')
        || input.starts_with('\\')
        || (input.len() >= 2 && input.as_bytes()[1] == b':')
    {
        return Err(PathError::Absolute(input.to_string()));
    }
    let mut out = PathBuf::new();
    for part in input.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(PathError::ParentRef(input.to_string()));
        }
        if part.contains(':') {
            return Err(PathError::AdsStream(part.to_string()));
        }
        if let Some(c) = part
            .chars()
            .find(|&c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*') || (c as u32) < 0x20)
        {
            return Err(PathError::InvalidChar(c));
        }
        if part.len() > 255 {
            return Err(PathError::ComponentTooLong(part.to_string()));
        }
        if part.ends_with('.') || part.ends_with(' ') {
            return Err(PathError::TrailingDotSpace(part.to_string()));
        }
        if is_reserved_name(part) {
            return Err(PathError::ReservedName(part.to_string()));
        }
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        return Err(PathError::Empty);
    }
    Ok(out)
}

pub fn to_extended_len(path: &Path) -> PathBuf {
    let s = path.as_os_str().to_string_lossy();
    if s.starts_with(r"\\?\") {
        return path.to_path_buf();
    }
    PathBuf::from(format!(r"\\?\{s}"))
}

#[cfg(test)]
mod tests {
    use super::{PathBuf, PathError, sanitize_rel_path, to_extended_len};
    use std::path::Path;

    #[test]
    fn accepts_plain_relative_paths() {
        assert_eq!(
            sanitize_rel_path("my-app.exe").unwrap(),
            PathBuf::from("my-app.exe")
        );
        assert_eq!(
            sanitize_rel_path("resources/app/icon.png").unwrap(),
            PathBuf::from("resources/app/icon.png")
        );
        assert_eq!(sanitize_rel_path("a/./b").unwrap(), PathBuf::from("a/b"));
        assert_eq!(sanitize_rel_path("a//b").unwrap(), PathBuf::from("a/b"));
    }

    #[test]
    fn rejects_traversal_and_absolute() {
        assert!(matches!(
            sanitize_rel_path("../x"),
            Err(PathError::ParentRef(_))
        ));
        assert!(matches!(
            sanitize_rel_path("a/../b"),
            Err(PathError::ParentRef(_))
        ));
        assert!(matches!(
            sanitize_rel_path("C:/x"),
            Err(PathError::Absolute(_))
        ));
        assert!(matches!(
            sanitize_rel_path("/x"),
            Err(PathError::Absolute(_))
        ));
        assert!(matches!(
            sanitize_rel_path(r"\\server\share"),
            Err(PathError::Absolute(_))
        ));
    }

    #[test]
    fn rejects_ads_and_reserved_names() {
        assert!(matches!(
            sanitize_rel_path("foo:stream"),
            Err(PathError::AdsStream(_))
        ));
        assert!(matches!(
            sanitize_rel_path("a:b"),
            Err(PathError::Absolute(_))
        ));
        assert!(matches!(
            sanitize_rel_path("CON"),
            Err(PathError::ReservedName(_))
        ));
        assert!(matches!(
            sanitize_rel_path("con.txt"),
            Err(PathError::ReservedName(_))
        ));
        assert!(matches!(
            sanitize_rel_path("LPT9"),
            Err(PathError::ReservedName(_))
        ));
        assert!(!super::is_reserved_name("nuclear.dll"));
    }

    #[test]
    fn rejects_invalid_chars_and_trailing() {
        assert!(matches!(
            sanitize_rel_path("a<b"),
            Err(PathError::InvalidChar('<'))
        ));
        assert!(matches!(
            sanitize_rel_path("foo."),
            Err(PathError::TrailingDotSpace(_))
        ));
        assert!(matches!(
            sanitize_rel_path("a/bar /c"),
            Err(PathError::TrailingDotSpace(_))
        ));
        assert!(matches!(sanitize_rel_path(""), Err(PathError::Empty)));
        assert!(matches!(sanitize_rel_path("   "), Err(PathError::Empty)));
    }

    #[test]
    fn extended_len_prefix() {
        assert_eq!(
            to_extended_len(Path::new(r"C:\App\x")),
            PathBuf::from(r"\\?\C:\App\x")
        );
        assert_eq!(
            to_extended_len(Path::new(r"\\?\C:\x")),
            PathBuf::from(r"\\?\C:\x")
        );
    }
}
