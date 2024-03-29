use std::path::{Path, PathBuf};

/// Represents any kind of error that can occur while parsing the UCD.
#[derive(Debug)]
pub struct Error {
    pub(crate) kind: ErrorKind,
    pub(crate) line: Option<u64>,
    pub(crate) path: Option<PathBuf>,
}

/// The kind of error that occurred while parsing the UCD.
#[derive(Debug)]
pub enum ErrorKind {
    /// An I/O error.
    Io(std::io::Error),
    /// A generic parse error.
    Parse(String),
}

impl Error {
    /// Create a new parse error from the given message.
    pub(crate) fn parse(msg: String) -> Error {
        Error { kind: ErrorKind::Parse(msg), line: None, path: None }
    }

    /// Return the specific kind of this error.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Return the line number at which this error occurred, if available.
    pub fn line(&self) -> Option<u64> {
        self.line
    }

    /// Return the file path associated with this error, if one exists.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_ref().map(|p| &**p)
    }

    /// Unwrap this error into its underlying kind.
    pub fn into_kind(self) -> ErrorKind {
        self.kind
    }

    /// Returns true if and only if this is an I/O error.
    ///
    /// If this returns true, the underlying `ErrorKind` is guaranteed to be
    /// `ErrorKind::Io`.
    pub fn is_io_error(&self) -> bool {
        match self.kind {
            ErrorKind::Io(_) => true,
            _ => false,
        }
    }
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref path) = self.path {
            if let Some(line) = self.line {
                write!(f, "{}:{}: ", path.display(), line)?;
            } else {
                write!(f, "{}: ", path.display())?;
            }
        } else if let Some(line) = self.line {
            write!(f, "error on line {}: ", line)?;
        }
        match self.kind {
            ErrorKind::Io(ref err) => write!(f, "{}", err),
            ErrorKind::Parse(ref msg) => write!(f, "{}", msg),
        }
    }
}
