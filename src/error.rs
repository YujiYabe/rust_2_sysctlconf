use std::error::Error;
use std::fmt;
use std::io;

use crate::ValueType;

/// An error found in a sysctl configuration line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub(crate) line: usize,
    pub(crate) content: String,
}

impl ParseError {
    /// One-based line number on which parsing failed.
    pub fn line(&self) -> usize {
        self.line
    }

    /// The invalid line, without its line terminator.
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid sysctl configuration at line {}: {:?}",
            self.line, self.content
        )
    }
}

impl Error for ParseError {}

/// An error returned while reading and parsing a configuration.
#[derive(Debug)]
pub enum LoadError {
    Io(io::Error),
    Parse(ParseError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read sysctl configuration: {error}"),
            Self::Parse(error) => error.fmt(f),
        }
    }
}

impl Error for LoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
        }
    }
}

impl From<io::Error> for LoadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ParseError> for LoadError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

/// An invalid line in a schema file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaParseError {
    pub(crate) line: usize,
    pub(crate) content: String,
}

impl fmt::Display for SchemaParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid schema at line {}: {:?}",
            self.line, self.content
        )
    }
}

impl Error for SchemaParseError {}

/// A setting that does not conform to the loaded schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    UnknownKey {
        line: usize,
        key: String,
    },
    InvalidValue {
        line: usize,
        key: String,
        value: String,
        expected: ValueType,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey { line, key } => write!(
                f,
                "setting {key:?} at line {line} is not defined in the schema"
            ),
            Self::InvalidValue {
                line,
                key,
                value,
                expected,
            } => write!(
                f,
                "invalid value {value:?} for {key:?} at line {line}: expected {expected}"
            ),
        }
    }
}

impl Error for ValidationError {}

/// An error returned while loading and validating configuration files.
#[derive(Debug)]
pub enum ValidatedLoadError {
    Config(LoadError),
    SchemaIo(io::Error),
    SchemaParse(SchemaParseError),
    Validation(ValidationError),
}

impl fmt::Display for ValidatedLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => error.fmt(f),
            Self::SchemaIo(error) => write!(f, "failed to read schema: {error}"),
            Self::SchemaParse(error) => error.fmt(f),
            Self::Validation(error) => error.fmt(f),
        }
    }
}

impl Error for ValidatedLoadError {}
