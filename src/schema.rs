use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor};
use std::path::Path;
use std::str::FromStr;

use serde_json::{Map, Value};

use crate::parser::{insert_nested, parse_line};
use crate::{LoadError, SchemaParseError, SysctlMap, ValidatedLoadError, ValidationError};

/// A supported value type in a schema file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    String,
    Bool,
    Integer,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::String => "string",
            Self::Bool => "bool",
            Self::Integer => "integer",
        })
    }
}

impl FromStr for ValueType {
    type Err = ();
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "string" => Ok(Self::String),
            "bool" => Ok(Self::Bool),
            "integer" => Ok(Self::Integer),
            _ => Err(()),
        }
    }
}

/// Parses and validates configuration and schema strings.
pub fn parse_str_with_schema(input: &str, schema: &str) -> Result<SysctlMap, ValidatedLoadError> {
    let schema = parse_schema_reader(Cursor::new(schema.as_bytes()))?;
    parse_reader_with_schema(Cursor::new(input.as_bytes()), &schema)
}

/// Opens a configuration file and a schema file, then validates all settings.
pub fn parse_file_with_schema(
    config_path: impl AsRef<Path>,
    schema_path: impl AsRef<Path>,
) -> Result<SysctlMap, ValidatedLoadError> {
    let schema_file = File::open(schema_path).map_err(ValidatedLoadError::SchemaIo)?;
    let schema = parse_schema_reader(BufReader::new(schema_file))?;
    let config_file = File::open(config_path)
        .map_err(LoadError::Io)
        .map_err(ValidatedLoadError::Config)?;
    parse_reader_with_schema(BufReader::new(config_file), &schema)
}

fn parse_schema_reader<R: BufRead>(
    reader: R,
) -> Result<HashMap<String, ValueType>, ValidatedLoadError> {
    let mut schema = HashMap::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(ValidatedLoadError::SchemaIo)?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, value_type)) = trimmed.split_once("->") else {
            return schema_error(index, trimmed);
        };
        let key = key.trim();
        let Ok(value_type) = value_type.trim().parse() else {
            return schema_error(index, trimmed);
        };
        if key.is_empty() {
            return schema_error(index, trimmed);
        }
        schema.insert(key.to_owned(), value_type);
    }
    Ok(schema)
}

fn schema_error<T>(index: usize, content: &str) -> Result<T, ValidatedLoadError> {
    Err(ValidatedLoadError::SchemaParse(SchemaParseError {
        line: index + 1,
        content: content.to_owned(),
    }))
}

fn parse_reader_with_schema<R: BufRead>(
    reader: R,
    schema: &HashMap<String, ValueType>,
) -> Result<SysctlMap, ValidatedLoadError> {
    let mut settings = Map::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line
            .map_err(LoadError::Io)
            .map_err(ValidatedLoadError::Config)?;
        let Some((key, value)) = parse_line(&line, index + 1)
            .map_err(LoadError::Parse)
            .map_err(ValidatedLoadError::Config)?
        else {
            continue;
        };
        let Some(value_type) = schema.get(key) else {
            return Err(ValidatedLoadError::Validation(
                ValidationError::UnknownKey {
                    line: index + 1,
                    key: key.to_owned(),
                },
            ));
        };
        let value = convert_value(key, value, *value_type, index + 1)?;
        insert_nested(&mut settings, key, value);
    }
    Ok(settings)
}

fn convert_value(
    key: &str,
    value: &str,
    value_type: ValueType,
    line: usize,
) -> Result<Value, ValidatedLoadError> {
    let converted = match value_type {
        ValueType::String => Some(Value::String(value.to_owned())),
        ValueType::Bool => value.parse::<bool>().ok().map(Value::Bool),
        ValueType::Integer => value.parse::<i64>().ok().map(Value::from),
    };
    converted.ok_or_else(|| {
        ValidatedLoadError::Validation(ValidationError::InvalidValue {
            line,
            key: key.to_owned(),
            value: value.to_owned(),
            expected: value_type,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_converts_values_using_a_schema() {
        let parsed = parse_str_with_schema(
            "endpoint = localhost:3000\ndebug = true\nlog.file = app.log\nretry = 3\n",
            "endpoint -> string\ndebug -> bool\nlog.file -> string\nretry -> integer\n",
        )
        .unwrap();
        assert_eq!(parsed["endpoint"], "localhost:3000");
        assert_eq!(parsed["debug"], true);
        assert_eq!(parsed["log"]["file"], "app.log");
        assert_eq!(parsed["retry"], 3);
    }

    #[test]
    fn rejects_values_with_the_wrong_type() {
        let error = parse_str_with_schema("retry = many\n", "retry -> integer\n").unwrap_err();
        assert!(matches!(
            error,
            ValidatedLoadError::Validation(ValidationError::InvalidValue {
                line: 1,
                expected: ValueType::Integer,
                ..
            })
        ));
    }

    #[test]
    fn rejects_keys_missing_from_the_schema() {
        let error = parse_str_with_schema("unknown = value\n", "known -> string\n").unwrap_err();
        assert!(matches!(
            error,
            ValidatedLoadError::Validation(ValidationError::UnknownKey { line: 1, .. })
        ));
    }

    #[test]
    fn rejects_invalid_schema_lines() {
        let error = parse_str_with_schema("retry = 3\n", "retry -> number\n").unwrap_err();
        assert!(matches!(
            error,
            ValidatedLoadError::SchemaParse(SchemaParseError { line: 1, .. })
        ));
    }

    #[test]
    fn allows_schema_keys_to_be_absent_from_the_configuration() {
        let parsed = parse_str_with_schema(
            "endpoint = localhost\n",
            "endpoint -> string\ndebug -> bool\n",
        )
        .unwrap();
        assert_eq!(parsed["endpoint"], "localhost");
        assert!(!parsed.contains_key("debug"));
    }
}
