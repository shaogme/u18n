use std::{
    collections::BTreeSet,
    error::Error,
    fmt::{self, Display, Formatter},
};

use u18n_ast::SchemaAst;
use u18n_locale::LocaleParser;
use u18n_schema::SchemaParser;

use crate::{
    cldr::CldrData,
    codegen::{CodeGenerator, CodegenInput, GeneratedSource as CodegenSource, GenerationConfig},
    diagnostic::Diagnostic,
    extract::Extractor,
    fingerprint::{Fingerprint, FingerprintInput},
    semantic::{LinkedCatalog, LocaleInput, LocaleParseStatus, ValidationRequest, Validator},
};

pub struct SourceFile {
    name: String,
    contents: String,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, contents: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            contents: contents.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn contents(&self) -> &str {
        &self.contents
    }
}

pub struct CompilationInput {
    schema: SourceFile,
    locales: Vec<SourceFile>,
}

impl CompilationInput {
    pub fn new(schema: SourceFile, locales: Vec<SourceFile>) -> Self {
        Self { schema, locales }
    }

    pub fn schema(&self) -> &SourceFile {
        &self.schema
    }

    pub fn locales(&self) -> &[SourceFile] {
        &self.locales
    }
}

#[derive(Clone)]
pub struct CompilerOptions {
    pool_limit: usize,
    atomic_locale: bool,
    final_fallback_locale: Option<String>,
}

impl CompilerOptions {
    pub fn with_pool_limit(mut self, pool_limit: usize) -> Self {
        self.pool_limit = pool_limit;
        self
    }

    pub fn pool_limit(&self) -> usize {
        self.pool_limit
    }

    pub fn with_atomic_locale(mut self, atomic_locale: bool) -> Self {
        self.atomic_locale = atomic_locale;
        self
    }

    pub fn atomic_locale(&self) -> bool {
        self.atomic_locale
    }

    pub fn with_final_fallback_locale(mut self, locale: impl Into<String>) -> Self {
        self.final_fallback_locale = Some(locale.into());
        self
    }

    pub fn final_fallback_locale(&self) -> Option<&str> {
        self.final_fallback_locale.as_deref()
    }
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            pool_limit: 256,
            atomic_locale: false,
            final_fallback_locale: None,
        }
    }
}

struct ParsedCompilation {
    schema: SchemaAst,
    schema_source: String,
    locale_inputs: Vec<LocaleInput>,
    catalog: LinkedCatalog,
    cldr: CldrData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolStatistics {
    literal_fragments: usize,
    pool_bytes: usize,
    overlap_optimization_enabled: bool,
    keyed_literal_fragments: usize,
    keyed_pool_bytes: usize,
    keyed_shard_count: usize,
    keyed_optimized_shards: usize,
}

impl PoolStatistics {
    pub fn literal_fragments(&self) -> usize {
        self.literal_fragments
    }

    pub fn pool_bytes(&self) -> usize {
        self.pool_bytes
    }

    pub fn overlap_optimization_enabled(&self) -> bool {
        self.overlap_optimization_enabled
    }

    /// Returns the total fragments across all per-message pools, counting repeated text per key.
    pub fn keyed_literal_fragments(&self) -> usize {
        self.keyed_literal_fragments
    }

    /// Returns the total bytes across all per-message pools, including repeated text per key.
    pub fn keyed_pool_bytes(&self) -> usize {
        self.keyed_pool_bytes
    }

    /// Returns the number of emitted per-message pools.
    pub fn keyed_shard_count(&self) -> usize {
        self.keyed_shard_count
    }

    /// Returns how many per-message pools used overlap optimization.
    pub fn keyed_optimized_shards(&self) -> usize {
        self.keyed_optimized_shards
    }
}

pub struct GeneratedSource {
    source: String,
    pool_statistics: PoolStatistics,
}

impl GeneratedSource {
    fn from_codegen(generated: CodegenSource) -> Self {
        Self {
            source: generated.source,
            pool_statistics: PoolStatistics {
                literal_fragments: generated.literal_fragments,
                pool_bytes: generated.pool_bytes,
                overlap_optimization_enabled: generated.overlap_optimization_enabled,
                keyed_literal_fragments: generated.keyed_literal_fragments,
                keyed_pool_bytes: generated.keyed_pool_bytes,
                keyed_shard_count: generated.keyed_shard_count,
                keyed_optimized_shards: generated.keyed_optimized_shards,
            },
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn pool_statistics(&self) -> PoolStatistics {
        self.pool_statistics
    }
}

#[derive(Debug)]
pub struct CompilerError {
    message: String,
}

impl CompilerError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for CompilerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for CompilerError {}

pub struct Compiler {
    options: CompilerOptions,
}

impl Compiler {
    pub fn new(options: CompilerOptions) -> Self {
        Self { options }
    }

    pub fn compile(&self, input: CompilationInput) -> Result<GeneratedSource, CompilerError> {
        let parsed = Self::parse_and_validate(input, false, true)?;
        self.validate_generation_options(&parsed.catalog)?;
        let generation = GenerationConfig {
            pool_limit: self.options.pool_limit,
            atomic_locale: self.options.atomic_locale,
            final_fallback_locale: self.options.final_fallback_locale.as_deref(),
        };
        let fingerprint = Fingerprint::calculate(FingerprintInput {
            schema: &parsed.schema_source,
            locales: &parsed.locale_inputs,
            registry_digest: parsed.cldr.registry_digest(),
            cardinal_digest: parsed.cldr.cardinal_digest(),
            ordinal_digest: parsed.cldr.ordinal_digest(),
            aliases_digest: parsed.cldr.aliases_digest(),
            supplemental_digest: parsed.cldr.supplemental_digest(),
            generation,
        });
        let generated = CodeGenerator::generate(CodegenInput {
            catalog: &parsed.catalog,
            fingerprint: &fingerprint,
            registry_digest: parsed.cldr.registry_digest(),
            cardinal_digest: parsed.cldr.cardinal_digest(),
            ordinal_digest: parsed.cldr.ordinal_digest(),
            aliases_digest: parsed.cldr.aliases_digest(),
            supplemental_digest: parsed.cldr.supplemental_digest(),
            main_parents: parsed.cldr.main_parents(),
            generation,
        })
        .map_err(|error| CompilerError::new(format!("code generation: {error}")))?;
        Ok(GeneratedSource::from_codegen(generated))
    }

    fn validate_generation_options(&self, catalog: &LinkedCatalog) -> Result<(), CompilerError> {
        let final_fallback = self.options.final_fallback_locale.as_deref();
        match (self.options.atomic_locale, final_fallback) {
            (true, None) => Err(CompilerError::new(
                "atomic locale selection requires a final fallback locale; configure one with `with_final_fallback_locale`",
            )),
            (false, Some(_)) => Err(CompilerError::new(
                "a final fallback locale can only be configured when atomic locale selection is enabled",
            )),
            (true, Some(tag))
                if !catalog
                    .locales
                    .iter()
                    .any(|locale| locale.ast.locale.source_tag == tag) =>
            {
                Err(CompilerError::new(format!(
                    "final fallback locale `{tag}` does not exactly match an input catalog"
                )))
            }
            _ => Ok(()),
        }
    }

    pub fn extract(
        &self,
        input: CompilationInput,
        source_locale: &str,
    ) -> Result<String, CompilerError> {
        let parsed = Self::parse_and_validate(input, false, false)?;
        Extractor::extract(
            &parsed.schema,
            &parsed.catalog,
            &parsed.locale_inputs,
            source_locale,
        )
        .map_err(CompilerError::new)
    }

    pub fn coverage(&self, input: CompilationInput) -> Result<String, CompilerError> {
        let parsed = Self::parse_and_validate(input, true, false)?;
        Extractor::coverage(&parsed.catalog).map_err(CompilerError::new)
    }

    fn parse_and_validate(
        input: CompilationInput,
        allow_missing: bool,
        combine_parse_diagnostics: bool,
    ) -> Result<ParsedCompilation, CompilerError> {
        let CompilationInput { schema, locales } = input;
        let schema_parse = SchemaParser::parse_collect(&schema.contents, 0);
        let mut parse_diagnostics = schema_parse
            .errors
            .into_iter()
            .map(|error| {
                Diagnostic::at(
                    &schema.name,
                    &schema.contents,
                    error.start,
                    error.end,
                    error.message,
                )
            })
            .collect::<Vec<_>>();
        let schema_ast = schema_parse.ast;
        let mut locale_inputs = Vec::new();
        let mut locale_parse_status = LocaleParseStatus {
            had_errors: false,
            unavailable_tags: BTreeSet::new(),
        };

        for (index, locale) in locales.into_iter().enumerate() {
            let file_id = u32::try_from(index + 1)
                .map_err(|_| CompilerError::new("too many locale files"))?;
            let parsed = LocaleParser::parse_collect(&locale.contents, file_id);
            let locale_tag = parsed.locale_tag().map(ToOwned::to_owned);
            let has_errors = parsed.has_errors();
            locale_parse_status.had_errors |= has_errors;
            if has_errors && let Some(locale_tag) = locale_tag {
                locale_parse_status.unavailable_tags.insert(locale_tag);
            }
            parse_diagnostics.extend(parsed.errors().iter().map(|error| {
                Diagnostic::at(
                    &locale.name,
                    &locale.contents,
                    error.start(),
                    error.end(),
                    error.message().to_string(),
                )
            }));
            if let Some(ast) = parsed.into_ast() {
                locale_inputs.push(LocaleInput {
                    path: locale.name,
                    source: locale.contents,
                    ast,
                });
            }
        }

        let Some(schema_ast) = schema_ast else {
            if parse_diagnostics.is_empty() {
                return Err(CompilerError::new("schema parser did not produce an AST"));
            }
            return Err(Self::render_diagnostics(parse_diagnostics));
        };
        if !combine_parse_diagnostics && !parse_diagnostics.is_empty() {
            return Err(Self::render_diagnostics(parse_diagnostics));
        }

        let cldr = CldrData::load()
            .map_err(|error| CompilerError::new(format!("pinned locale data: {error}")))?;
        let catalog = match Validator::validate(ValidationRequest {
            schema: &schema_ast,
            locale_inputs: &locale_inputs,
            cldr: &cldr,
            schema_source: &schema.contents,
            parse_status: &locale_parse_status,
            allow_missing,
        }) {
            Ok(catalog) => catalog,
            Err(diagnostics) => {
                if combine_parse_diagnostics {
                    parse_diagnostics.extend(diagnostics);
                    return Err(Self::render_diagnostics(parse_diagnostics));
                }
                return Err(Self::render_diagnostics(diagnostics));
            }
        };
        if !parse_diagnostics.is_empty() {
            return Err(Self::render_diagnostics(parse_diagnostics));
        }

        Ok(ParsedCompilation {
            schema: schema_ast,
            schema_source: schema.contents,
            locale_inputs,
            catalog,
            cldr,
        })
    }

    fn render_diagnostics(diagnostics: Vec<Diagnostic>) -> CompilerError {
        CompilerError::new(
            diagnostics
                .into_iter()
                .map(Diagnostic::render_owned)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}
