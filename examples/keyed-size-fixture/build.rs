use std::{
    env,
    error::Error,
    fs,
    io::{self, Write},
    path::PathBuf,
};

use u18n_compiler::{CompilationInput, Compiler, CompilerOptions, SourceFile};

fn main() -> Result<(), Box<dyn Error>> {
    let input = CompilationInput::new(
        SourceFile::new("schema.u18n", include_str!("schema.u18n")),
        vec![SourceFile::new(
            "locales/en.u18n",
            include_str!("locales/en.u18n"),
        )],
    );
    let generated = Compiler::new(CompilerOptions::default()).compile(input)?;
    let output =
        PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is unset")?).join("u18n_messages.rs");
    fs::write(output, generated.source())?;
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "cargo:rerun-if-changed=schema.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/en.u18n")?;
    Ok(())
}
