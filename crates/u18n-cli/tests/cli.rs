use std::{
    env,
    error::Error,
    fmt::Debug,
    fs,
    path::PathBuf,
    process::{self, Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let identifier = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("u18n-cli-test-{}-{identifier}", process::id()));
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn write(&self, name: &str, contents: &str) -> Result<PathBuf, Box<dyn Error>> {
        let path = self.root.join(name);
        fs::write(&path, contents)?;
        Ok(path)
    }

    fn compile(&self, schema_source: &str, locale_source: &str) -> Result<Output, Box<dyn Error>> {
        self.compile_with_limit(schema_source, locale_source, None)
    }

    fn compile_with_limit(
        &self,
        schema_source: &str,
        locale_source: &str,
        pool_limit: Option<usize>,
    ) -> Result<Output, Box<dyn Error>> {
        self.compile_locales_with_limit(
            schema_source,
            &[("locale.u18n", locale_source)],
            pool_limit,
        )
    }

    fn compile_locales_with_limit(
        &self,
        schema_source: &str,
        locale_sources: &[(&str, &str)],
        pool_limit: Option<usize>,
    ) -> Result<Output, Box<dyn Error>> {
        let schema = self.write("schema.u18n", schema_source)?;
        let output = self.root.join("generated.rs");
        let mut command = Command::new(env!("CARGO_BIN_EXE_u18n-cli"));
        command
            .arg("compile")
            .arg("--schema")
            .arg(schema.to_str().ok_or("schema path is not UTF-8")?);
        for (name, source) in locale_sources {
            let locale = self.write(name, source)?;
            command
                .arg("--locale")
                .arg(locale.to_str().ok_or("locale path is not UTF-8")?);
        }
        command
            .arg("--output")
            .arg(output.to_str().ok_or("output path is not UTF-8")?);
        if let Some(pool_limit) = pool_limit {
            command.arg("--pool-limit").arg(pool_limit.to_string());
        }
        Ok(command.output()?)
    }

    fn extract(
        &self,
        schema_source: &str,
        locale_sources: &[(&str, &str)],
        source_locale: &str,
    ) -> Result<Output, Box<dyn Error>> {
        let schema = self.write("extract-schema.u18n", schema_source)?;
        let mut command = Command::new(env!("CARGO_BIN_EXE_u18n-cli"));
        command
            .arg("extract")
            .arg("--schema")
            .arg(schema.to_str().ok_or("schema path is not UTF-8")?)
            .arg("--source-locale")
            .arg(source_locale);
        for (name, source) in locale_sources {
            let locale = self.write(name, source)?;
            command
                .arg("--locale")
                .arg(locale.to_str().ok_or("locale path is not UTF-8")?);
        }
        Ok(command.output()?)
    }

    fn coverage(
        &self,
        schema_source: &str,
        locale_sources: &[(&str, &str)],
    ) -> Result<Output, Box<dyn Error>> {
        let schema = self.write("coverage-schema.u18n", schema_source)?;
        let mut command = Command::new(env!("CARGO_BIN_EXE_u18n-cli"));
        command
            .arg("coverage")
            .arg("--schema")
            .arg(schema.to_str().ok_or("schema path is not UTF-8")?);
        for (name, source) in locale_sources {
            let locale = self.write(name, source)?;
            command
                .arg("--locale")
                .arg(locale.to_str().ok_or("locale path is not UTF-8")?);
        }
        Ok(command.output()?)
    }

    fn run_with_options(
        &self,
        command_name: &str,
        schema_source: &str,
        locale_sources: &[(&str, &str)],
        options: &[&str],
    ) -> Result<Output, Box<dyn Error>> {
        let schema = self.write("option-schema.u18n", schema_source)?;
        let mut command = Command::new(env!("CARGO_BIN_EXE_u18n-cli"));
        command
            .arg(command_name)
            .arg("--schema")
            .arg(schema.to_str().ok_or("schema path is not UTF-8")?);
        for (name, source) in locale_sources {
            let locale = self.write(name, source)?;
            command
                .arg("--locale")
                .arg(locale.to_str().ok_or("locale path is not UTF-8")?);
        }
        command.args(options);
        Ok(command.output()?)
    }

    fn generated(&self) -> Result<String, Box<dyn Error>> {
        Ok(fs::read_to_string(self.root.join("generated.rs"))?)
    }
}

fn check(condition: bool) {
    assert!(condition);
}

fn check_equal<T: Debug + PartialEq>(actual: T, expected: T) {
    assert_eq!(actual, expected);
}

#[test]
fn atomic_compile_options_are_validated_and_reported() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let schema = "schema 1; message app.title();";
    let locales = [("en.u18n", "locale en; message app.title = \"Title\";")];

    let valid = fixture.run_with_options(
        "compile",
        schema,
        &locales,
        &["--atomic", "--final-fallback", "en"],
    )?;
    check(valid.status.success());
    let generated = String::from_utf8(valid.stdout)?;
    check(generated.contains("atomic-locale-selection=true"));
    check(generated.contains("final-fallback-locale=en"));
    check(generated.contains("pub const FINAL_FALLBACK_LOCALE"));
    check(generated.contains("macro_rules! t"));

    let missing_fallback = fixture.run_with_options("compile", schema, &locales, &["--atomic"])?;
    check(!missing_fallback.status.success());
    check(
        String::from_utf8(missing_fallback.stderr)?
            .contains("--atomic requires --final-fallback TAG"),
    );

    let missing_tag = fixture.run_with_options(
        "compile",
        schema,
        &locales,
        &["--atomic", "--final-fallback"],
    )?;
    check(!missing_tag.status.success());
    check(
        String::from_utf8(missing_tag.stderr)?.contains("--final-fallback requires a locale tag"),
    );

    let non_input = fixture.run_with_options(
        "compile",
        schema,
        &locales,
        &["--atomic", "--final-fallback", "EN"],
    )?;
    check(!non_input.status.success());
    check(String::from_utf8(non_input.stderr)?.contains("exactly match an input catalog"));

    let unpaired =
        fixture.run_with_options("compile", schema, &locales, &["--final-fallback", "en"])?;
    check(!unpaired.status.success());
    check(String::from_utf8(unpaired.stderr)?.contains("--final-fallback requires --atomic"));
    Ok(())
}

#[test]
fn atomic_options_reject_duplicates_and_non_compile_commands() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let schema = "schema 1; message app.title();";
    let locales = [("en.u18n", "locale en; message app.title = \"Title\";")];
    let duplicate_atomic = fixture.run_with_options(
        "compile",
        schema,
        &locales,
        &["--atomic", "--atomic", "--final-fallback", "en"],
    )?;
    check(!duplicate_atomic.status.success());
    check(String::from_utf8(duplicate_atomic.stderr)?.contains("duplicate --atomic"));

    let duplicate_fallback = fixture.run_with_options(
        "compile",
        schema,
        &locales,
        &[
            "--atomic",
            "--final-fallback",
            "en",
            "--final-fallback",
            "en",
        ],
    )?;
    check(!duplicate_fallback.status.success());
    check(String::from_utf8(duplicate_fallback.stderr)?.contains("duplicate --final-fallback"));

    let extract = fixture.run_with_options(
        "extract",
        schema,
        &locales,
        &["--atomic", "--source-locale", "en"],
    )?;
    check(!extract.status.success());
    check(String::from_utf8(extract.stderr)?.contains("--atomic is only valid for compile"));

    let coverage =
        fixture.run_with_options("coverage", schema, &locales, &["--final-fallback", "en"])?;
    check(!coverage.status.success());
    check(
        String::from_utf8(coverage.stderr)?.contains("--final-fallback is only valid for compile"),
    );
    Ok(())
}

#[test]
fn help_documents_atomic_selection_requirements() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_u18n-cli"))
        .arg("--help")
        .output()?;
    check(output.status.success());
    let help = String::from_utf8(output.stdout)?;
    check(help.contains("--atomic --final-fallback TAG"));
    check(help.contains("requires --atomic"));
    check(help.contains("input locale catalog"));
    Ok(())
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn compiles_typed_messages_and_reports_reproducible_metadata() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1;\nmessage app.title();\nmessage cart.items(count: plural_count, item_name: string);\n",
        "locale en;\nmessage app.title = \"Cart\";\nmessage cart.items = \"{count, plural, one {# item} other {# items}} for {item_name}\";\n",
    )?;
    check(output.status.success());
    let generated = fixture.generated()?;
    check(generated.contains("output-format-version=2"));
    check(generated.contains("cldr-cardinal-rules-sha256=69a30c57a621de9c52ec6ca95fe7bf9f06df6f8c13ca71a72e447b532c92cff7"));
    check(generated.contains("cldr-ordinal-rules-sha256=6c62fa2250f4f0150a3c7e24fc93c20ae6722f65bd26b49aaad799b40f8014b3"));
    check(generated.contains(
        "cldr-aliases-sha256=b06614fc2c9aab3b9a9f7e8bf9328f4f7a95d89b2d2dbb300d0caff0f885c164",
    ));
    check(generated.contains("locale-extension-policy=reject-all"));
    check(generated.contains("iana-language-subtag-registry-sha256=755fad43283be7b41ebe3c89ad054b6eaf928f404f9c0edb74799e0eab74beb1"));
    check(generated.contains("macro_rules! m_app_stitle"));
    check(generated.contains("macro_rules! m_cart_sitems"));
    check(!generated.contains("Messages"));
    check(!generated.contains("Args"));
    check(!generated.contains("pub mod messages"));
    check(generated.contains("static SHARED_STORE: SharedProgramStore"));
    check(generated.contains("shared_store: &super::SHARED_STORE"));
    check(!generated.contains("super::super::SHARED_STORE"));
    check(generated.contains("PluralBranchRecord"));
    let statistics = String::from_utf8(output.stderr)?;
    check(statistics.contains("dynamic shared pool: 4 literal fragments"));
    check(statistics.contains("keyed shard pools:"));
    Ok(())
}

#[test]
fn compiles_a_crlf_locale_fixture_with_a_rendered_line_break() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let source_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/crlf");
    let schema = source_directory.join("schema.u18n");
    let locale = source_directory.join("en.u18n");
    let output_path = fixture.root.join("generated.rs");
    let output = Command::new(env!("CARGO_BIN_EXE_u18n-cli"))
        .arg("compile")
        .arg("--schema")
        .arg(&schema)
        .arg("--locale")
        .arg(&locale)
        .arg("--output")
        .arg(&output_path)
        .output()?;

    check(output.status.success());
    let generated = fs::read_to_string(output_path)?;
    check(generated.contains("output-format-version=2"));
    check(generated.contains("Instruction::LineBreak"));
    check(generated.contains("text_pool: \"firstsecond\""));
    Ok(())
}

#[test]
fn prefixes_message_macros_and_avoids_keyword_conflicts() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message menu(); message type(a: string, b: string, c: string, d: string, e: string);",
        "locale en; message menu = \"Menu\"; message type = \"Type\";",
    )?;
    check(output.status.success());

    let generated = fixture.generated()?;
    check(generated.contains("macro_rules! m_menu"));
    check(generated.contains("macro_rules! m_type"));
    check(!generated.contains("Messages"));
    check(!generated.contains("Args"));
    Ok(())
}

#[test]
fn compiles_typed_formatters_and_ordinal_plural_rules() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message report(amount: decimal, rank: ordinal_count);",
        "locale en; message report = \"{amount, format, decimal, grouping: auto, precision: 2, rounding: half_even}; rank {rank, ordinal, one {#st} two {#nd} few {#rd} other {#th}}\";",
    )?;

    check(output.status.success());
    let generated = fixture.generated()?;
    check(generated.contains("FormatOptions::Decimal"));
    check(generated.contains("PluralKind::Ordinal"));
    check(generated.contains("RuleSet { kind: PluralKind::Ordinal"));
    check(generated.contains("$formatter:expr"));
    check(generated.contains("rank, ordinal_count"));

    let invalid = fixture.compile(
        "schema 1; message report(amount: number);",
        "locale en; message report = \"{amount, format, decimal, grouping: auto, precision: 2, rounding: half_even}\";",
    )?;
    check(!invalid.status.success());
    check(String::from_utf8(invalid.stderr)?.contains("expected `decimal`"));
    Ok(())
}

#[test]
fn extracts_translator_context_in_sorted_order_through_explicit_fallbacks()
-> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.extract(
        "schema 1;\n/// Last key docs.\nmessage z.last(\n    /// Parameter guidance.\n    name: string\n);\n/// First key docs.\nmessage a.first();",
        &[
            ("es.u18n", "locale es fallback en; message a.first = \"Primero\";"),
            (
                "en.u18n",
                "locale en; message z.last = \"Hello {name}\"; message a.first = \"First\";",
            ),
        ],
        "es",
    )?;

    check(output.status.success());
    let extracted = String::from_utf8(output.stdout)?;
    let first_position = extracted.find("a.first").ok_or("first key absent")?;
    let last_position = extracted.find("z.last").ok_or("last key absent")?;
    check(first_position < last_position);
    check(extracted.contains("First key docs."));
    check(extracted.contains("Parameter guidance."));
    check(extracted.contains("\"pattern_locale\": \"en\""));
    check(extracted.contains("\\\"Hello {name}\\\""));
    Ok(())
}

#[test]
fn coverage_reports_authored_inherited_and_missing_messages() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let schema = "schema 1; message a.first(); message z.last();";
    let locales = [
        (
            "fr.u18n",
            "locale fr fallback en; message z.last = \"Dernier\";",
        ),
        ("en.u18n", "locale en; message a.first = \"First\";"),
    ];
    let coverage = fixture.coverage(schema, &locales)?;

    check(coverage.status.success());
    let report = String::from_utf8(coverage.stdout)?;
    check(report.contains("\"authored\": 1, \"inherited\": 0, \"missing\": 1"));
    check(report.contains("\"status\": \"missing\""));
    check(report.contains("\"status\": \"inherited\", \"author_locale\": \"en\""));
    check(report.contains("\"status\": \"authored\", \"author_locale\": \"fr\""));

    let incomplete_compile = fixture.compile_locales_with_limit(schema, &locales, None)?;
    check(!incomplete_compile.status.success());
    check(String::from_utf8(incomplete_compile.stderr)?.contains("is missing schema key"));

    let invalid = fixture.coverage(
        "schema 1; message title();",
        &[("invalid.u18n", "locale en; message ghost = \"Ghost\";")],
    )?;
    check(!invalid.status.success());
    check(String::from_utf8(invalid.stderr)?.contains("is not declared in the schema"));
    Ok(())
}

#[test]
fn compiles_decimal_cardinal_arguments_and_exact_catalog_lookup_helpers()
-> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message cart.items(count: plural_decimal);",
        "locale en; message cart.items = \"{count, plural, =2 {exact #} one {# item} other {# items}}\";",
    )?;
    check(output.status.success());
    let generated = fixture.generated()?;
    check(generated.contains("macro_rules! m_cart_sitems"));
    check(generated.contains("count, plural_decimal"));
    check(generated.contains("PluralOperand::V"));
    check(generated.contains("pub enum LocaleTag {"));
    check(generated.contains("/// The `En` locale.\n        En,"));
    check(generated.contains("static LOCALE_INDEX: &[LocaleIndexEntry]"));
    check(generated.contains(
        "LOCALE_INDEX.binary_search_by(|entry| tag.cmp_ignore_ascii_case(entry.tag).reverse())",
    ));
    check(generated.contains("pub const fn as_str(&self) -> &'static str"));
    check(generated.contains("Self::En => \"en\","));
    check(generated.contains("impl FromStr for LocaleTag"));
    check(generated.contains("fn from_str(locale_tag: &str) -> Result<Self, Self::Err>"));
    check(generated.contains("Self::find_source_locale(&tag).ok_or(\"unsupported locale tag\")"));
    check(generated.contains("\n        pub const fn as_catalog(&self) -> &'static Catalog"));
    check(generated.contains("Self::En => &catalogs::locale_en::CATALOG"));
    check(!generated.contains("find_catalog"));
    check(!generated.contains("pub const fn select_catalog("));
    check(!generated.contains("index: usize"));

    let invalid_ordinal = fixture.compile(
        "schema 1; message rank(value: plural_decimal);",
        "locale en; message rank = \"{value, ordinal, one {#st} two {#nd} few {#rd} other {#th}}\";",
    )?;
    check(!invalid_ordinal.status.success());
    check(String::from_utf8(invalid_ordinal.stderr)?.contains("expected `ordinal_count`"));
    Ok(())
}

#[test]
fn inherited_decimal_plural_patterns_keep_their_author_locale_rules() -> Result<(), Box<dyn Error>>
{
    let fixture = Fixture::new()?;
    let output = fixture.compile_locales_with_limit(
        "schema 1; message count.items(value: plural_decimal);",
        &[
            (
                "en.u18n",
                "locale en fallback fr;",
            ),
            (
                "fr.u18n",
                "locale fr; message count.items = \"{value, plural, one {# article} many {# articles} other {# articles}}\";",
            ),
        ],
        None,
    )?;

    check(output.status.success());
    let generated = fixture.generated()?;
    let english = generated
        .find("pub mod locale_en {")
        .ok_or("English catalog absent")?;
    let french = generated
        .find("pub mod locale_fr {")
        .ok_or("French catalog absent")?;
    let english_catalog = generated
        .get(english..french)
        .ok_or("catalog range invalid")?;
    check(generated.contains("PluralOperand::I"));
    check(english_catalog.contains("fallback: Some(&super::locale_fr::CATALOG)"));
    check(english_catalog.contains("bindings: &[]"));
    check(generated.contains("PluralCategory::Many"));
    check(generated.contains(
        "PluralRange { start: PluralNumber { coefficient: 0, scale: 0 }, end: PluralNumber { coefficient: 1, scale: 0 } }",
    ));
    Ok(())
}

#[test]
fn extraction_rejects_schema_and_locale_semantic_errors() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.extract(
        "schema 1; message title(name: string, name: string); message title();",
        &[(
            "en.u18n",
            "locale en fallback en; message ghost = \"Ghost\"; message title = \"{missing}\"; message title = \"Duplicate\";",
        )],
        "en",
    )?;

    check(!output.status.success());
    check(output.stdout.is_empty());
    let diagnostics = String::from_utf8(output.stderr)?;
    check(diagnostics.contains("duplicate schema message key `title`"));
    check(diagnostics.contains("duplicate parameter `name`"));
    check(diagnostics.contains("locale key `ghost` is not declared in the schema"));
    check(diagnostics.contains("duplicate locale message key `title`"));
    check(diagnostics.contains("unknown schema parameter `missing`"));
    check(diagnostics.contains("fallback cycle"));

    let empty_schema = fixture.extract("schema 1;", &[("en.u18n", "locale en;")], "en")?;

    check(!empty_schema.status.success());
    check(empty_schema.stdout.is_empty());
    check(
        String::from_utf8(empty_schema.stderr)?
            .contains("schema must declare at least one message"),
    );
    Ok(())
}

#[test]
fn catalogs_without_plural_rules_skip_the_ruleset_import() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message app.title();",
        "locale en; message app.title = \"Title\";",
    )?;

    check(output.status.success());
    let generated = fixture.generated()?;
    check(generated.contains(
        "use u18n::{Instruction, LocaleProgram, MessageData, MessageProgram, MessageShardData, MessageSignature, SharedProgramStore};",
    ));
    check(generated.contains("use u18n::{Catalog, CatalogData, Locale, MessageBinding};"));
    check(!generated.contains("RuleSet"));
    check(!generated.contains("pub mod args"));
    Ok(())
}

#[test]
fn rustfmt_does_not_edit_generated_source() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message cart.items(count: plural_count, item_name: string);",
        "locale en; message cart.items = \"{count, plural, one {# item} other {# items}} for {item_name}\";",
    )?;
    check(output.status.success());

    let generated_before_formatting = fixture.generated()?;
    let formatting = Command::new("rustfmt")
        .arg(fixture.root.join("generated.rs"))
        .output()?;
    check(formatting.status.success());
    check_equal(fixture.generated()?, generated_before_formatting);
    Ok(())
}

#[test]
fn compiles_nested_plural_patterns_and_escaped_hashes() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message cart.items(count: plural_count);",
        "locale en; message cart.items = \"{count, plural, one {\\# #} other {{count, plural, one {#} other {#}}}}\";",
    )?;
    check(output.status.success());
    let generated = fixture.generated()?;
    check(generated.contains("plural_index: 1"));
    check_equal(generated.matches("Instruction::Pound").count(), 6);
    Ok(())
}

#[test]
fn enforces_the_plural_nesting_limit() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let schema = "schema 1; message cart.items(count: plural_count);";
    let locale_at_depth = |depth: usize| {
        let mut pattern = String::from("end");
        for _ in 0..depth {
            pattern = ["{count, plural, one {", &pattern, "} other {}}"].concat();
        }
        format!("locale en; message cart.items = \"{pattern}\";")
    };

    let accepted = fixture.compile(schema, &locale_at_depth(32))?;
    check(accepted.status.success());

    let rejected = fixture.compile(schema, &locale_at_depth(33))?;
    check(!rejected.status.success());
    check(
        String::from_utf8(rejected.stderr)?
            .contains("plural nesting exceeds the maximum depth of 32"),
    );
    Ok(())
}

#[test]
fn rejects_missing_schema_messages_with_a_source_location() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message app.title(); message cart.items(count: plural_count);",
        "locale en; message app.title = \"Cart\";",
    )?;
    check(!output.status.success());
    let diagnostic = String::from_utf8(output.stderr)?;
    check(diagnostic.contains("is missing schema key `cart.items`"));
    check(diagnostic.contains("locale.u18n:1:"));
    check(diagnostic.contains("related: <schema>:1:"));
    Ok(())
}

#[test]
fn validates_plural_types_and_exact_cardinal_branch_sets() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let type_error = fixture.compile(
        "schema 1; message cart.items(amount: number);",
        "locale en; message cart.items = \"{amount, plural, one {one} other {other}}\";",
    )?;
    check(!type_error.status.success());
    let type_diagnostic = String::from_utf8(type_error.stderr)?;
    check(type_diagnostic.contains("expected `plural_count`"));
    check(type_diagnostic.contains("parameter type declared here"));

    let missing_branch = fixture.compile(
        "schema 1; message cart.items(count: plural_count);",
        "locale en; message cart.items = \"{count, plural, one {one}}\";",
    )?;
    check(!missing_branch.status.success());
    check(String::from_utf8(missing_branch.stderr)?.contains("missing category `other`"));
    Ok(())
}

#[test]
fn validates_locale_registration_casing_and_rejects_extensions() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let schema = "schema 1; message app.title();";
    let lowercase_region =
        fixture.compile(schema, "locale en-us; message app.title = \"Title\";")?;
    check(!lowercase_region.status.success());
    check(String::from_utf8(lowercase_region.stderr)?.contains("must be upper case"));

    let uppercase_private_use_marker =
        fixture.compile(schema, "locale en-X-foo; message app.title = \"Title\";")?;
    check(!uppercase_private_use_marker.status.success());
    check(String::from_utf8(uppercase_private_use_marker.stderr)?.contains("must use lower case"));

    let extension = fixture.compile(
        schema,
        "locale en-u-ca-gregory; message app.title = \"Title\";",
    )?;
    check(!extension.status.success());
    check(String::from_utf8(extension.stderr)?.contains("extension sequence"));

    let unregistered = fixture.compile(schema, "locale zzz; message app.title = \"Title\";")?;
    check(!unregistered.status.success());
    check(String::from_utf8(unregistered.stderr)?.contains("unregistered language subtag"));
    Ok(())
}

#[test]
fn resolves_numeric_regions_and_uses_their_cardinal_categories() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message cart.items(count: plural_count);",
        "locale es-419; message cart.items = \"{count, plural, one {# uno} many {# millón} other {# cosas}}\";",
    )?;
    check(output.status.success());
    check(fixture.generated()?.contains("pub mod locale_es_419"));
    Ok(())
}

#[test]
fn validates_registered_variant_prefixes() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let schema = "schema 1; message app.title();";

    let matching_prefix =
        fixture.compile(schema, "locale de-CH-1901; message app.title = \"Title\";")?;
    check(matching_prefix.status.success());

    let mismatched_prefix =
        fixture.compile(schema, "locale en-1901; message app.title = \"Title\";")?;
    check(!mismatched_prefix.status.success());
    check(
        String::from_utf8(mismatched_prefix.stderr)?
            .contains("does not match any registered IANA Prefix"),
    );

    let one_of_multiple_prefixes = fixture.compile(
        schema,
        "locale sl-rozaj-1994; message app.title = \"Title\";",
    )?;
    check(one_of_multiple_prefixes.status.success());
    Ok(())
}

#[test]
fn emits_typed_message_macros_without_dto_apis() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; \
         message test.inline(a: string, b: string, c: string, d: string); \
         message test.formatted_inline(a: decimal, b: decimal, c: decimal); \
         message test.many(a: string, b: number, c: plural_count, d: string, e: number); \
         message test.other(a: number, b: number, c: number, d: number, e: number); \
         message test.formatted(a: decimal, b: money, c: string, d: number);",
        "locale en; \
         message test.inline = \"{a} {b} {c} {d}\"; \
         message test.formatted_inline = \"{a, format, decimal, grouping: none, precision: input, rounding: half_even} {b, format, decimal, grouping: none, precision: input, rounding: half_even} {c, format, decimal, grouping: none, precision: input, rounding: half_even}\"; \
         message test.many = \"{a} {b} {c} {d} {e}\"; \
         message test.other = \"{a} {b} {c} {d} {e}\"; \
         message test.formatted = \"{a, format, decimal, grouping: none, precision: input, rounding: half_even} {b, format, currency, precision: currency, rounding: half_even} {c} {d}\";",
    )?;
    check(output.status.success());
    let generated = fixture.generated()?;
    check(generated.contains("macro_rules! m_test_sinline"));
    check(generated.contains("macro_rules! m_test_sformatted_uinline"));
    check(generated.contains("macro_rules! m_test_smany"));
    check(generated.contains("macro_rules! m_test_sother"));
    check(generated.contains("macro_rules! m_test_sformatted"));
    check(generated.contains("(a, string), (b, number), (c, plural_count)"));
    check(generated.contains("(a, decimal), (b, money), (c, string), (d, number)"));
    check(generated.contains("$formatter:expr"));
    check(!generated.contains("pub mod args"));
    check(!generated.contains("pub struct Args"));
    check(!generated.contains("Messages"));
    Ok(())
}

#[test]
fn macro_names_do_not_depend_on_sorted_message_positions() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let first = fixture.compile(
        "schema 1; message z.many(a: string, b: string, c: string, d: string, e: string);",
        "locale en; message z.many = \"{a} {b} {c} {d} {e}\";",
    )?;
    check(first.status.success());
    check(fixture.generated()?.contains("macro_rules! m_z_smany"));

    let inserted_earlier = fixture.compile(
        "schema 1; message a.before(); message z.many(a: string, b: string, c: string, d: string, e: string);",
        "locale en; message a.before = \"Before\"; message z.many = \"{a} {b} {c} {d} {e}\";",
    )?;
    check(inserted_earlier.status.success());
    let generated = fixture.generated()?;
    check(generated.contains("macro_rules! m_z_smany"));
    check(!generated.contains("Args"));
    Ok(())
}

#[test]
fn records_multiple_number_parameters_in_macro_metadata() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message receipt.total(amount: number, fee: number);",
        "locale en; message receipt.total = \"{amount} + {fee}\";",
    )?;
    check(output.status.success());

    let generated = fixture.generated()?;
    check(generated.contains("macro_rules! m_receipt_stotal"));
    check(generated.contains("(amount, number), (fee, number)"));
    Ok(())
}

#[test]
fn disables_overlap_merging_above_the_configured_pool_limit() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile_with_limit(
        "schema 1; message greet(name: string);",
        "locale en; message greet = \"Hello {name}, goodbye\";",
        Some(0),
    )?;
    check(output.status.success());
    check(
        String::from_utf8(output.stderr)?
            .contains("overlap optimization disabled by the optimization limits"),
    );
    Ok(())
}

#[test]
fn compiles_string_selectors_and_exact_plural_cases() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile(
        "schema 1; message cart.summary(gender: string, count: plural_count);",
        "locale en; message cart.summary = \"{gender, select, female {She} other {They}}: {count, plural, =0 {none} one {# item} other {# items}}\";",
    )?;

    check(output.status.success());
    let generated = fixture.generated()?;
    check(generated.contains("Instruction::Select { select_index: 0 }"));
    check(generated.contains("PluralSelector::Exact(0)"));
    check(generated.contains("SelectBranchRecord"));
    Ok(())
}

#[test]
fn resolves_partial_locales_and_keeps_inherited_plural_rules() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile_locales_with_limit(
        "schema 1; message cart.items(count: plural_count);",
        &[
            ("en.u18n", "locale en fallback fr;"),
            (
                "fr.u18n",
                "locale fr; message cart.items = \"{count, plural, =0 {aucun} one {# article} many {# articles} other {# articles}}\";",
            ),
        ],
        None,
    )?;

    check(output.status.success());
    let generated = fixture.generated()?;
    check(generated.contains("pub mod locale_en"));
    check(
        generated.contains(
            "PluralNode { argument_index: 0, kind: PluralKind::Cardinal, rule_set_index: 0",
        ),
    );
    check(generated.contains("aucun"));
    Ok(())
}

#[test]
fn emits_only_rule_sets_used_by_each_resolved_catalog() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile_locales_with_limit(
        "schema 1; message cart.items(count: plural_count); message cart.other(count: plural_count); message app.title();",
        &[
            ("en.u18n", "locale en fallback fr;"),
            (
                "fr.u18n",
                "locale fr fallback ja; message cart.items = \"{count, plural, one {# article} many {# articles} other {# articles}}\"; message app.title = \"Accueil\";",
            ),
            (
                "it.u18n",
                "locale it; message cart.items = \"Articles\"; message cart.other = \"Autres\"; message app.title = \"Titolo\";",
            ),
            (
                "ja.u18n",
                "locale ja; message cart.items = \"{count, plural, other {# 個}}\"; message cart.other = \"{count, plural, other {# 個別}}\"; message app.title = \"タイトル\";",
            ),
        ],
        None,
    )?;

    check(output.status.success());
    let generated = fixture.generated()?;
    let shared_store_start = generated
        .find("static SHARED_STORE:")
        .ok_or("shared store missing")?;
    let shared_store_end = generated[shared_store_start..]
        .find("pub mod locale_")
        .map(|offset| shared_store_start + offset)
        .ok_or("locale modules missing")?;
    check_equal(
        generated[shared_store_start..shared_store_end]
            .matches("RuleSet { kind: PluralKind::")
            .count(),
        2,
    );
    check_equal(
        generated[shared_store_start..shared_store_end]
            .matches("PluralNode { argument_index: 0, kind: PluralKind::Cardinal,")
            .count(),
        3,
    );
    check_equal(
        generated
            .matches("PluralNode { argument_index: 0, kind: PluralKind::Cardinal,")
            .count(),
        6,
    );
    Ok(())
}

#[test]
fn validates_messages_in_duplicate_tag_locale_files() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let output = fixture.compile_locales_with_limit(
        "schema 1; message title(count: number);",
        &[
            ("en-first.u18n", "locale en; message title = \"Title\";"),
            (
                "en-second.u18n",
                "locale en fallback zz; message ghost = \"{missing}\"; message title = \"{missing} {count, plural, one {one} other {other}}\"; message title = \"Repeated\";",
            ),
        ],
        None,
    )?;

    check(!output.status.success());
    let diagnostics = String::from_utf8(output.stderr)?;
    check(diagnostics.contains("duplicate source locale tag `en`"));
    check(diagnostics.contains("locale key `ghost` is not declared in the schema"));
    check(diagnostics.contains("unknown schema parameter `missing`"));
    check(diagnostics.contains("selector `count` has type `number`; expected `plural_count`"));
    check(diagnostics.contains("duplicate locale message key `title`"));
    check(diagnostics.contains("fallback locale `zz` is not present in the input set"));
    Ok(())
}

#[test]
fn rejects_unknown_fallbacks_and_cycles() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let unknown = fixture.compile(
        "schema 1; message title();",
        "locale en fallback fr; message title = \"Title\";",
    )?;
    check(!unknown.status.success());
    check(String::from_utf8(unknown.stderr)?.contains("fallback locale `fr` is not present"));

    let cycle = fixture.compile_locales_with_limit(
        "schema 1; message title();",
        &[
            (
                "en.u18n",
                "locale en fallback fr; message title = \"Title\";",
            ),
            (
                "fr.u18n",
                "locale fr fallback en; message title = \"Titre\";",
            ),
        ],
        None,
    )?;
    check(!cycle.status.success());
    let cycle_stderr = String::from_utf8(cycle.stderr)?;
    check(cycle_stderr.contains("fallback cycle"));
    check_equal(
        cycle_stderr
            .matches("fallback cycle reaches locale")
            .count(),
        1,
    );
    Ok(())
}

#[test]
fn reports_multiple_syntax_and_semantic_errors() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let syntax = fixture.compile(
        "schema 1; message first(); message second();",
        r##"locale en; message first = "bad\q"; message second = "bad\z";"##,
    )?;
    check(!syntax.status.success());
    check_equal(
        String::from_utf8(syntax.stderr)?
            .matches("unknown escape sequence")
            .count(),
        2,
    );

    let semantic = fixture.compile(
        "schema 1; message first(); message second();",
        "locale en; message first = \"{missing_a}\"; message second = \"{missing_b}\";",
    )?;
    check(!semantic.status.success());
    check_equal(
        String::from_utf8(semantic.stderr)?
            .matches("unknown schema parameter")
            .count(),
        2,
    );

    let combined = fixture.compile_locales_with_limit(
        "schema 1; message title(name: string);",
        &[
            ("en.u18n", r##"locale en; message title = "broken\q";"##),
            ("fr-CA.u18n", "locale fr-CA fallback en;"),
            ("es.u18n", "locale es; message title = \"{missing}\";"),
        ],
        None,
    )?;
    check(!combined.status.success());
    let combined_stderr = String::from_utf8(combined.stderr)?;
    check(combined_stderr.contains("unknown escape sequence"));
    check(combined_stderr.contains("unknown schema parameter `missing`"));
    check(!combined_stderr.contains("fallback locale `en` is not present"));
    check(!combined_stderr.contains("is missing schema key `title`"));

    let unrelated_fallback = fixture.compile_locales_with_limit(
        "schema 1; message title();",
        &[
            ("en.u18n", r##"locale en; message title = "broken\q";"##),
            ("fr.u18n", "locale fr fallback de;"),
        ],
        None,
    )?;
    check(!unrelated_fallback.status.success());
    let unrelated_stderr = String::from_utf8(unrelated_fallback.stderr)?;
    check(unrelated_stderr.contains("unknown escape sequence"));
    check(unrelated_stderr.contains("fallback locale `de` is not present"));
    check(unrelated_stderr.contains("locale `fr` is missing schema key `title`"));
    Ok(())
}
