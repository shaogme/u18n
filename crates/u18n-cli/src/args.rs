use std::{
    env,
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Command {
    Compile,
    Extract,
    Coverage,
}

pub(super) struct Arguments {
    command: Command,
    schema: PathBuf,
    locales: Vec<PathBuf>,
    output: Option<PathBuf>,
    pool_limit: usize,
    source_locale: Option<String>,
    atomic_locale: bool,
    final_fallback_locale: Option<String>,
}

impl Arguments {
    pub(super) fn schema(&self) -> &PathBuf {
        &self.schema
    }

    pub(super) fn command(&self) -> Command {
        self.command
    }

    pub(super) fn source_locale(&self) -> Option<&str> {
        self.source_locale.as_deref()
    }

    pub(super) fn locales(&self) -> &[PathBuf] {
        &self.locales
    }

    pub(super) fn output(&self) -> Option<&PathBuf> {
        self.output.as_ref()
    }

    pub(super) fn pool_limit(&self) -> usize {
        self.pool_limit
    }

    pub(super) fn atomic_locale(&self) -> bool {
        self.atomic_locale
    }

    pub(super) fn final_fallback_locale(&self) -> Option<&str> {
        self.final_fallback_locale.as_deref()
    }
}

pub(super) struct ArgumentsParser;

impl ArgumentsParser {
    pub(super) fn parse() -> Result<Option<Arguments>, String> {
        let mut values = env::args_os().skip(1);
        let Some(first) = values.next() else {
            return Err(Self::usage());
        };
        if first == "--help" || first == "-h" {
            let mut output = io::stdout();
            output
                .write_all(Self::usage().as_bytes())
                .map_err(|error| error.to_string())?;
            output.write_all(b"\n").map_err(|error| error.to_string())?;
            return Ok(None);
        }
        let command = match first.to_str() {
            Some("compile") => Command::Compile,
            Some("extract") => Command::Extract,
            Some("coverage") => Command::Coverage,
            _ => return Err(Self::usage()),
        };
        let mut schema = None;
        let mut locales = Vec::new();
        let mut output = None;
        let mut pool_limit = 256usize;
        let mut source_locale = None;
        let mut atomic_locale = false;
        let mut atomic_seen = false;
        let mut final_fallback_locale = None;
        while let Some(option) = values.next() {
            let Some(option) = option.to_str() else {
                return Err("command-line option is not valid UTF-8".to_string());
            };
            match option {
                "--schema" => schema = Some(Self::next_path(&mut values, option)?),
                "--locale" => locales.push(Self::next_path(&mut values, option)?),
                "--output" | "-o" => output = Some(Self::next_path(&mut values, option)?),
                "--atomic" => {
                    if command != Command::Compile {
                        return Err("--atomic is only valid for compile".to_string());
                    }
                    if atomic_seen {
                        return Err("duplicate --atomic option".to_string());
                    }
                    atomic_seen = true;
                    atomic_locale = true;
                }
                "--final-fallback" => {
                    if command != Command::Compile {
                        return Err("--final-fallback is only valid for compile".to_string());
                    }
                    if final_fallback_locale.is_some() {
                        return Err("duplicate --final-fallback option".to_string());
                    }
                    let value = values
                        .next()
                        .ok_or_else(|| "--final-fallback requires a locale tag".to_string())?;
                    let value = value
                        .to_str()
                        .ok_or_else(|| "final fallback locale is not valid UTF-8".to_string())?;
                    if value.is_empty() || value.starts_with('-') {
                        return Err("--final-fallback requires a locale tag".to_string());
                    }
                    final_fallback_locale = Some(value.to_string());
                }
                "--source-locale" if command == Command::Extract => {
                    let value = values
                        .next()
                        .ok_or_else(|| format!("{option} requires a locale tag"))?;
                    source_locale = Some(
                        value
                            .to_str()
                            .ok_or_else(|| "source locale is not valid UTF-8".to_string())?
                            .to_string(),
                    );
                }
                "--pool-limit" => {
                    let value = values
                        .next()
                        .ok_or_else(|| format!("{option} requires a number"))?;
                    let value = value
                        .to_str()
                        .ok_or_else(|| "pool limit is not valid UTF-8".to_string())?;
                    pool_limit = value
                        .parse::<usize>()
                        .map_err(|_| format!("invalid pool limit `{value}`"))?;
                }
                _ => return Err(format!("unknown option `{option}`\n{}", Self::usage())),
            }
        }
        let schema = schema.ok_or_else(Self::usage)?;
        if locales.is_empty() {
            return Err("at least one --locale input is required".to_string());
        }
        if command == Command::Extract && source_locale.is_none() {
            return Err("extract requires --source-locale TAG".to_string());
        }
        if final_fallback_locale.is_some() && !atomic_locale {
            return Err("--final-fallback requires --atomic".to_string());
        }
        if atomic_locale && final_fallback_locale.is_none() {
            return Err("--atomic requires --final-fallback TAG".to_string());
        }
        Ok(Some(Arguments {
            command,
            schema,
            locales,
            output,
            pool_limit,
            source_locale,
            atomic_locale,
            final_fallback_locale,
        }))
    }

    fn next_path(
        values: &mut impl Iterator<Item = OsString>,
        option: &str,
    ) -> Result<PathBuf, String> {
        values
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| format!("{option} requires a path"))
    }

    fn usage() -> String {
        "usage: u18n-cli compile --schema FILE --locale FILE... [--atomic --final-fallback TAG] [--output FILE] [--pool-limit N]\n       --final-fallback requires --atomic and must name an input locale catalog\n       u18n-cli extract --schema FILE --locale FILE... --source-locale TAG [--output FILE]\n       u18n-cli coverage --schema FILE --locale FILE... [--output FILE]\n       u18n-cli --help".to_string()
    }
}
