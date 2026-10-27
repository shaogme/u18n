mod args;

use std::{
    fs,
    io::{self, Write},
    path::Path,
    process,
};

use args::{ArgumentsParser, Command};
use u18n_compiler::{CompilationInput, Compiler, CompilerOptions, SourceFile};

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr(), "{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = match ArgumentsParser::parse()? {
        Some(args) => args,
        None => return Ok(()),
    };
    let schema_source = fs::read_to_string(args.schema())
        .map_err(|error| format!("{}: {error}", args.schema().display()))?;
    let schema = SourceFile::new(args.schema().display().to_string(), schema_source);
    let mut locale_paths = args.locales().to_vec();
    locale_paths.sort_by_key(|path| fs::canonicalize(path).unwrap_or_else(|_| path.clone()));
    let mut locales = Vec::new();
    for path in locale_paths {
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        locales.push(SourceFile::new(path.display().to_string(), source));
    }
    let input = CompilationInput::new(schema, locales);
    let output = match args.command() {
        Command::Compile => {
            let mut options = CompilerOptions::default()
                .with_pool_limit(args.pool_limit())
                .with_atomic_locale(args.atomic_locale());
            if let Some(final_fallback) = args.final_fallback_locale() {
                options = options.with_final_fallback_locale(final_fallback);
            }
            let compiler = Compiler::new(options);
            let generated = compiler.compile(input).map_err(|error| error.to_string())?;
            let statistics = generated.pool_statistics();
            write_output(args.output().map(Path::new), generated.source())?;
            let optimization = if statistics.overlap_optimization_enabled() {
                "enabled"
            } else {
                "disabled by the optimization limits"
            };
            writeln!(
                io::stderr(),
                "u18n-cli: dynamic shared pool: {} literal fragments, {} bytes; overlap optimization {optimization}",
                statistics.literal_fragments(),
                statistics.pool_bytes()
            )
            .map_err(|error| format!("stderr: {error}"))?;
            writeln!(
                io::stderr(),
                "u18n-cli: keyed shard pools: {} fragments, {} bytes across {} keys; overlap optimization ran for {}/{} shards",
                statistics.keyed_literal_fragments(),
                statistics.keyed_pool_bytes(),
                statistics.keyed_shard_count(),
                statistics.keyed_optimized_shards(),
                statistics.keyed_shard_count()
            )
            .map_err(|error| format!("stderr: {error}"))?;
            return Ok(());
        }
        Command::Extract => Compiler::new(CompilerOptions::default())
            .extract(
                input,
                args.source_locale()
                    .ok_or_else(|| "extract requires --source-locale TAG".to_string())?,
            )
            .map_err(|error| error.to_string())?,
        Command::Coverage => Compiler::new(CompilerOptions::default())
            .coverage(input)
            .map_err(|error| error.to_string())?,
    };
    write_output(args.output().map(Path::new), &output)
}

fn write_output(path: Option<&Path>, output: &str) -> Result<(), String> {
    match path {
        Some(path) => {
            fs::write(path, output).map_err(|error| format!("{}: {error}", path.display()))
        }
        None => io::stdout()
            .write_all(output.as_bytes())
            .map_err(|error| format!("stdout: {error}")),
    }
}
