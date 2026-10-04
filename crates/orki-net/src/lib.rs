#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Https,
    Http,
    File,
}

impl Scheme {
    pub fn parse(url: &str) -> Option<Self> {
        if url.starts_with("https://") {
            Some(Self::Https)
        } else if url.starts_with("http://") {
            Some(Self::Http)
        } else if url.starts_with("file://") {
            Some(Self::File)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

impl DownloadProgress {
    pub fn fraction(&self) -> f64 {
        match self.total {
            Some(t) if t > 0 => (self.downloaded as f64 / t as f64).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NetError {
    #[error("unsupported url scheme")]
    UnsupportedScheme,
    #[error("download failed after {retries} attempts: {url}")]
    DownloadFailed { retries: u32, url: String },
    #[error("hash mismatch for downloaded file")]
    HashMismatch,
}

pub const MAX_RESUME_ATTEMPTS: u32 = 5;

#[cfg(test)]
mod tests {
    use super::{DownloadProgress, MAX_RESUME_ATTEMPTS, NetError, Scheme};

    #[test]
    fn scheme_parsing() {
        assert_eq!(
            Scheme::parse("https://example.com/a.exe"),
            Some(Scheme::Https)
        );
        assert_eq!(
            Scheme::parse("http://example.com/a.exe"),
            Some(Scheme::Http)
        );
        assert_eq!(Scheme::parse("file:///C:/a.exe"), Some(Scheme::File));
        assert_eq!(Scheme::parse("ftp://x"), None);
    }

    #[test]
    fn progress_fraction() {
        let p = DownloadProgress {
            downloaded: 50,
            total: Some(100),
        };
        assert_eq!(p.fraction(), 0.5);
        let p = DownloadProgress {
            downloaded: 150,
            total: Some(100),
        };
        assert_eq!(p.fraction(), 1.0);
        let p = DownloadProgress {
            downloaded: 10,
            total: None,
        };
        assert_eq!(p.fraction(), 0.0);
    }

    #[test]
    fn error_display() {
        let e = NetError::DownloadFailed {
            retries: 3,
            url: "https://x".into(),
        };
        assert_eq!(e.to_string(), "download failed after 3 attempts: https://x");
        assert_eq!(MAX_RESUME_ATTEMPTS, 5);
    }
}
