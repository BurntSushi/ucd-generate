use std::error;
use std::fmt;
use std::io;
use std::result;

use clap;
use fst;
use ucd_parse;
use ucd_trie;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Clap(clap::Error),
    Other(String),
}

impl Error {
    pub fn is_broken_pipe(&self) -> bool {
        match *self {
            Error::Io(ref e) if e.kind() == io::ErrorKind::BrokenPipe => true,
            _ => false,
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::Io(ref err) => Some(err),
            Error::Clap(ref err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::Io(ref err) => err.fmt(f),
            Error::Clap(ref err) => err.fmt(f),
            Error::Other(ref msg) => write!(f, "{}", msg),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<clap::Error> for Error {
    fn from(err: clap::Error) -> Error {
        Error::Clap(err)
    }
}

impl From<fst::Error> for Error {
    fn from(err: fst::Error) -> Error {
        Error::Other(err.to_string())
    }
}

impl From<ucd_parse::Error> for Error {
    fn from(err: ucd_parse::Error) -> Error {
        Error::Other(err.to_string())
    }
}

impl From<ucd_trie::Error> for Error {
    fn from(err: ucd_trie::Error) -> Error {
        Error::Other(err.to_string())
    }
}
