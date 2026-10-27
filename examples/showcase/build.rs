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
        vec![
            SourceFile::new("locales/ar.u18n", include_str!("locales/ar.u18n")),
            SourceFile::new("locales/az.u18n", include_str!("locales/az.u18n")),
            SourceFile::new("locales/en.u18n", include_str!("locales/en.u18n")),
            SourceFile::new("locales/en-IN.u18n", include_str!("locales/en-IN.u18n")),
            SourceFile::new(
                "locales/en-x-abcdefgh-abc.u18n",
                include_str!("locales/en-x-abcdefgh-abc.u18n"),
            ),
            SourceFile::new(
                "locales/en-x-private.u18n",
                include_str!("locales/en-x-private.u18n"),
            ),
            SourceFile::new("locales/es-419.u18n", include_str!("locales/es-419.u18n")),
            SourceFile::new("locales/fr.u18n", include_str!("locales/fr.u18n")),
            SourceFile::new(
                "locales/zh-Hant-HK.u18n",
                include_str!("locales/zh-Hant-HK.u18n"),
            ),
        ],
    );
    let generated = Compiler::new(
        CompilerOptions::default()
            .with_atomic_locale(true)
            .with_final_fallback_locale("en"),
    )
    .compile(input)?;
    let output =
        PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is unset")?).join("u18n_messages.rs");
    fs::write(output, generated.source())?;
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "cargo:rerun-if-changed=schema.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/ar.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/az.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/en.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/en-IN.u18n")?;
    writeln!(
        stdout,
        "cargo:rerun-if-changed=locales/en-x-abcdefgh-abc.u18n"
    )?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/en-x-private.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/es-419.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/fr.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/zh-Hant-HK.u18n")?;
    Ok(())
}
