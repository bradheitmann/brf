// A failure the command line reports as a plain message (exit 2) instead of a crash (exit 1).
use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// An expected refusal or usage error: printed as "brf: <message> (<CODE>)", exit 2.
    Brf { code: String, message: String },
    /// Anything else: printed as is, exit 1.
    Other(String),
}

impl Error {
    pub fn brf(code: &str, message: impl Into<String>) -> Self {
        Error::Brf { code: code.to_string(), message: message.into() }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Brf { code, .. } => Some(code),
            Error::Other(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Brf { message, .. } => f.write_str(message),
            Error::Other(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Other(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// An io error with the path it concerns, in the 1.x wording ("ENOENT: no such file or directory, open '<path>'").
pub fn io_at(e: std::io::Error, op: &str, path: &str) -> Error {
    let what = match e.kind() {
        std::io::ErrorKind::NotFound => "ENOENT: no such file or directory".to_string(),
        std::io::ErrorKind::PermissionDenied => "EACCES: permission denied".to_string(),
        std::io::ErrorKind::AlreadyExists => "EEXIST: file already exists".to_string(),
        _ => e.to_string(),
    };
    Error::Other(format!("{what}, {op} '{path}'"))
}
