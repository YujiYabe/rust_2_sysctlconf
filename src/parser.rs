use std::fs::File;
use std::io::{BufRead, BufReader, Cursor};
use std::path::Path;

use serde_json::{Map, Value};

use crate::{LoadError, ParseError};

/// Parsed sysctl settings represented as a nested map.
pub type SysctlMap = Map<String, Value>;

/// Parses a UTF-8 string containing sysctl configuration.
pub fn parse_str(input: &str) -> Result<SysctlMap, ParseError> {
    match parse_reader(Cursor::new(input.as_bytes())) {
        Ok(settings) => Ok(settings),
        Err(LoadError::Parse(error)) => Err(error),
        Err(LoadError::Io(error)) => unreachable!("reading from a string failed: {error}"),
    }
}

/// Parses sysctl configuration from a buffered reader.
pub fn parse_reader<R: BufRead>(reader: R) -> Result<SysctlMap, LoadError> {
    let mut settings = Map::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        if let Some((key, value)) = parse_line(&line, index + 1)? {
            insert_nested(&mut settings, key, Value::String(value.to_owned()));
        }
    }
    Ok(settings)
}

pub(crate) fn insert_nested(settings: &mut SysctlMap, key: &str, value: Value) {
    let parts: Vec<_> = key.split('.').collect();
    insert_path(settings, &parts, value);
}

fn insert_path(settings: &mut SysctlMap, parts: &[&str], value: Value) {
    if let [key] = parts {
        settings.insert((*key).to_owned(), value);
        return;
    }
    let object = settings
        .entry(parts[0].to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if !object.is_object() {
        *object = Value::Object(Map::new());
    }
    insert_path(object.as_object_mut().unwrap(), &parts[1..], value);
}

/// Opens and parses a sysctl configuration file at an arbitrary path.
pub fn parse_file(path: impl AsRef<Path>) -> Result<SysctlMap, LoadError> {
    parse_reader(BufReader::new(File::open(path)?))
}

/// Alias for [`parse_file`], for callers that prefer loading terminology.
pub fn load_file(path: impl AsRef<Path>) -> Result<SysctlMap, LoadError> {
    parse_file(path)
}

pub(crate) fn parse_line(
    line: &str,
    line_number: usize,
) -> Result<Option<(&str, &str)>, ParseError> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
        return Ok(None);
    }
    let line = line.strip_prefix('-').unwrap_or(line).trim_start();
    let pair = if let Some((key, value)) = line.split_once('=') {
        Some((key.trim(), value.trim()))
    } else {
        line.find(char::is_whitespace)
            .map(|separator| (&line[..separator], line[separator..].trim()))
    };
    match pair {
        Some((key, value)) if !key.is_empty() && !value.is_empty() => Ok(Some((key, value))),
        _ => Err(ParseError {
            line: line_number,
            content: line.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn parses_normal_sysctl_syntax() {
        let parsed = parse_str("# settings\nnet.ipv4.ip_forward = 1\nvm.swappiness 10\nkernel.domainname = example.local\n").unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed["net"]["ipv4"]["ip_forward"], "1");
        assert_eq!(parsed["vm"]["swappiness"], "10");
        assert_eq!(parsed["kernel"]["domainname"], "example.local");
    }

    #[test]
    fn later_values_overwrite_earlier_values() {
        let parsed = parse_str("vm.swappiness = 60\nvm.swappiness = 5\n").unwrap();
        assert_eq!(parsed["vm"]["swappiness"], "5");
    }

    #[test]
    fn accepts_error_suppression_prefix() {
        let parsed = parse_str("-net.ipv6.conf.all.disable_ipv6 = 1\n").unwrap();
        assert_eq!(parsed["net"]["ipv6"]["conf"]["all"]["disable_ipv6"], "1");
    }

    #[test]
    fn preserves_spaces_and_comment_characters_in_values() {
        let parsed = parse_str("kernel.domainname = example # value\n").unwrap();
        assert_eq!(parsed["kernel"]["domainname"], "example # value");
    }

    #[test]
    fn dotted_keys_create_nested_maps() {
        let parsed = parse_str(
            "endpoint = localhost:3000\nlog.file = /var/log/console.log\nlog.name = default.log\n",
        )
        .unwrap();
        assert_eq!(parsed["endpoint"], "localhost:3000");
        assert_eq!(parsed["log"]["file"], "/var/log/console.log");
        assert_eq!(parsed["log"]["name"], "default.log");
        assert!(parsed.get("log.file").is_none());
    }

    #[test]
    fn reports_invalid_line_number_and_content() {
        let error = parse_str("ok.key = value\ninvalid\n").unwrap_err();
        assert_eq!(error.line(), 2);
        assert_eq!(error.content(), "invalid");
    }

    #[test]
    fn propagates_reader_errors() {
        assert!(matches!(
            parse_reader(io::BufReader::new(FailingReader)),
            Err(LoadError::Io(_))
        ));
    }

    struct FailingReader;
    impl io::Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("test error"))
        }
    }
}
