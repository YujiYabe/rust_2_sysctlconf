use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use sysctl_conf::{ValidatedLoadError, parse_file_with_schema};

#[derive(Debug)]
struct InputPaths {
    config: PathBuf,
    schema: PathBuf,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let paths = input_paths(env::args_os().skip(1))?;
    let settings = parse_file_with_schema(&paths.config, &paths.schema)
        .map_err(|error| describe_load_error(&paths, error))?;

    println!("{}", serde_json::to_string_pretty(&settings)?);

    Ok(())
}

fn input_paths(mut arguments: impl Iterator<Item = std::ffi::OsString>) -> io::Result<InputPaths> {
    let config = arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("sysctl.conf"));
    let schema = arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("schema.conf"));

    if arguments.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: sysctl-conf [CONFIG_FILE] [SCHEMA_FILE]",
        ));
    }

    Ok(InputPaths { config, schema })
}

fn describe_load_error(paths: &InputPaths, error: ValidatedLoadError) -> io::Error {
    let message = match &error {
        ValidatedLoadError::Config(sysctl_conf::LoadError::Io(source))
            if source.kind() == io::ErrorKind::NotFound =>
        {
            format!("configuration file not found: {}", paths.config.display())
        }
        ValidatedLoadError::SchemaIo(source) if source.kind() == io::ErrorKind::NotFound => {
            format!("schema file not found: {}", paths.schema.display())
        }
        _ => error.to_string(),
    };

    io::Error::other(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn defaults_to_sample_sysctl_conf() {
        let paths = input_paths(std::iter::empty()).unwrap();
        assert_eq!(paths.config, PathBuf::from("sysctl.conf"));
        assert_eq!(paths.schema, PathBuf::from("schema.conf"));
    }

    #[test]
    fn accepts_custom_paths() {
        let paths =
            input_paths([OsString::from("sample.conf"), OsString::from("types.conf")].into_iter())
                .unwrap();
        assert_eq!(paths.config, PathBuf::from("sample.conf"));
        assert_eq!(paths.schema, PathBuf::from("types.conf"));
    }

    #[test]
    fn rejects_extra_arguments() {
        let error = input_paths(
            [
                OsString::from("one.conf"),
                OsString::from("two.conf"),
                OsString::from("three.conf"),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
