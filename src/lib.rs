//! Parser for files using the Linux `sysctl.conf` syntax.
//!
//! Later definitions of the same key overwrite earlier definitions, matching
//! the behavior users normally expect when loading a configuration file.

mod error;
mod parser;
mod schema;

pub use error::{LoadError, ParseError, SchemaParseError, ValidatedLoadError, ValidationError};
pub use parser::{SysctlMap, load_file, parse_file, parse_reader, parse_str};
pub use schema::{ValueType, parse_file_with_schema, parse_str_with_schema};
