# Crate designs

This document describes how the workspace crates divide ownership and work. It complements
[Architecture](architecture.md), which describes the end-to-end build flow, and the [Schema
AST](specification/schema-ast.md) and [Locale AST](specification/locale-ast.md) specifications,
which define source behavior.

## Dependency and runtime boundaries

```text
u18n-schema ──────> u18n-ast
u18n-locale ──────> u18n-ast, u18n-locale-tag
u18n-locale-tag ──> u18n-compiler, u18n
u18n-compiler ────> u18n-schema, u18n-locale, u18n-ast, u18n-locale-tag
u18n-cli ────────> u18n-compiler

application ────> generated catalog source ────> u18n ────> u18n-macros
```

`u18n-ast`, `u18n-schema`, `u18n-locale`, `u18n-locale-tag`, and `u18n` use `no_std`; the AST
and locale parser also use `alloc`. The compiler and CLI use `std`; the proc-macro crate runs on
the host. The source parsers depend on the AST crate, but not on the compiler. The shared
locale-tag crate is allocation-free and owns syntax parsing, the general 16-byte ASCII
`ParsedTag::packed_key` helper, and the bounded semantic locale-code encoder used by generated
CLDR parent lookup. The
compiler emits source that refers to `u18n`; `u18n` re-exports the parser and semantic-code
helpers for generated code, along with the hidden proc-macro entry points needed by generated
message macros and `t!` dispatch.

## Crate responsibilities

### `u18n-ast`

[`crates/u18n-ast/src/ast.rs`](../crates/u18n-ast/src/ast.rs) owns the shared public data model.
It defines schema and locale nodes, closed argument and formatting enums, identifiers, message
keys, and file-relative spans. Fields are public because these are data-transfer types; this
crate does not parse source, bind names, validate semantics, or lower patterns.

The source parsers construct these nodes directly. Keeping one definition of `Identifier`,
`MessageKey`, and `Span` gives cross-AST diagnostics a shared vocabulary and prevents the locale
parser from depending on schema parser internals.

### `u18n-schema`

[`crates/u18n-schema/src/parser.rs`](../crates/u18n-schema/src/parser.rs) is a small
cursor-based parser for schema headers, declarations, parameters, and documentation comments.
`SchemaParser::parse` returns the first syntax error; `parse_collect` recovers at later
`message` boundaries and returns all collected errors with no AST when any error occurred. Spans
are recorded as UTF-8 byte offsets using the caller-supplied file ID.

This crate checks syntax only. Duplicate keys and parameter names are checked later by the
compiler, where they can be reported with related declaration spans.

### `u18n-locale-tag`

[`crates/u18n-locale-tag/src/lib.rs`](../crates/u18n-locale-tag/src/lib.rs) implements the
shared borrowed BCP 47 parser, grandfathered-tag set, private-use-aware truncation, and the
general-purpose 16-byte `ParsedTag::packed_key` API. The separate semantic encoder maps only a 2-
or 3-letter language with optional 4-letter script and optional 2-letter or 3-digit region into a
`u64`. It checks syntax and the version 1 extension
policy without IANA registration or heap allocation. The locale parser and checked runtime
catalog constructor use the same validation behavior. Generated source-locale lookup remains a
case-insensitive string index; each generated CLDR main-parent row stores a single sorted `u64`
key combining the semantic child code and fixed parent-enum discriminant.

### `u18n-locale`

[`crates/u18n-locale/src/parser.rs`](../crates/u18n-locale/src/parser.rs) parses the locale
header and top-level message declarations. It records the authored locale and explicit fallback
spelling without canonicalizing or inferring either one. It uses `u18n-locale-tag` to reject
malformed BCP 47 structure and extension sequences. Full IANA registration and
recommended-casing checks belong to the compiler, where the pinned registry is available.

[`crates/u18n-locale/src/parser/pattern.rs`](../crates/u18n-locale/src/parser/pattern.rs) parses
each quoted pattern into recursive typed nodes. It decodes escapes while retaining source spans,
parses interpolation, plural, ordinal, select, and format expressions, and enforces a maximum of
32 nested selectors. Literal message text is therefore ready for runtime rendering without a
runtime parser.

Like the schema parser, `LocaleParser::parse_collect` can recover at later message declarations.
Its output also retains a candidate locale tag when the header tag was lexically complete,
allowing the compiler to explain fallback references to files that failed to parse.

### `u18n-compiler`

[`crates/u18n-compiler/src/api.rs`](../crates/u18n-compiler/src/api.rs) is the public facade.
`CompilationInput` owns the schema and locale source files; `Compiler` exposes `compile`,
`extract`, and `coverage`; `CompilerOptions` carries string-pool and optional atomic-locale
settings. Parsing, validation, and code generation share this API, so the CLI and build scripts
use the same compilation behavior.

The implementation is split by phase:

| Module | Design responsibility |
| --- | --- |
| `semantic/schema.rs` | Check schema uniqueness and build per-message parameter bindings in schema order. |
| `semantic/pattern.rs` | Bind locale references to parameter slots; check value types, selector branches, and `#` scope; retain source locations for diagnostics. |
| `semantic/fallback.rs` | Check explicit fallback targets and cycles, then resolve each schema message from the nearest declaration in the chain. |
| `semantic/model.rs` | Hold linked messages and patterns between semantic validation and lowering. A linked message keeps its whole pattern and author locale. |
| `cldr.rs` | Load and digest-check embedded locale data, then coordinate tag and rule lookup, retaining independent main-component and plural-component parent maps. |
| `cldr/tag.rs` | Parse the vendored IANA registry and CLDR aliases, then validate source locale tags and resolve lookup spelling. |
| `cldr/rules.rs` | Parse CLDR plural and ordinal rule expressions into the runtime's predicate representation. |
| `codegen/pool.rs` | Build deterministic shared and per-message UTF-8 pools from literal fragments and select labels, subject to configured work limits. |
| `codegen/lower.rs` | Lower a globally interned dynamic store and independently intern each key's resolved programs and tables. |
| `codegen/emit.rs`, `codegen/emit/messages.rs`, and `codegen/emit/locale_tag.rs` | Emit the dynamic store, independent locale handles, one shard per key, lookup APIs, message macros, `t!`, and optional atomic locale selection. |
| `diagnostic.rs` | Render source spans and related locations as compiler diagnostics. |
| `extract.rs` | Produce deterministic translator extraction and coverage JSON from the validated linked catalog. |
| `fingerprint.rs` | Hash source files, generation settings, and all pinned locale-data digests. |

The validator sorts schema messages by key and uses ordered maps for locale identity and
bindings. Linking changes source names into schema-order numeric slots. Fallback resolution
selects a complete message pattern from one locale and records its author; it does not combine
fragments from different locales. Lowering interns equivalent complete patterns using canonical
selector ordering and plural-rule contents, then removes source-only spans and keeps only the
shared data needed for rendering.

Pinned IANA and CLDR inputs are embedded in the compiler. Loading verifies their expected
digests before parsing them. The IANA snapshot validates registered language, script, region,
variant, extlang, and grandfathered subtags. CLDR aliases and plural-component parent data
affect plural-rule lookup; the separate main-component parent map drives generated runtime
catalog selection. The source locale spelling remains the catalog identity.

The fingerprint orders locale inputs by source tag. It includes source contents, generation
settings, output-format version (`1`), CLDR version, IANA registry digest, CLDR cardinal-rule,
ordinal-rule, alias, and supplemental-data digests, and the locale extension policy. The
cardinal-rule and alias digests identify inputs used for plural resolution; the supplemental-data
digest identifies the main-component parent mappings embedded in generated
`LocaleTag::fallback`. The string pool uses ordered fragments; overlap optimization is skipped
when its configured limits would make the work excessive. Generated metadata records the format
version, locale-data identifiers, pool limit, and locale-selection settings.

### `u18n-macros`

[`crates/u18n-macros/src/lib.rs`](../crates/u18n-macros/src/lib.rs) is a host-side proc-macro
helper, not the owner of message definitions. The compiler emits `macro_rules!` definitions for
each schema message; those definitions call the hidden `invoke_message` helper with the message
schema, formatter requirement, lightweight locale expression, that key's shard, and caller
arguments.

`invoke_message` parses positional or named arguments, diagnoses invalid arity and named fields,
and emits typed `ArgumentValue` constructors. For named arguments, it evaluates expressions once
in written order and only then arranges values into schema order. `dispatch_translation` expands
`t!("key", ...)`: it validates the control-header form, selects an explicit locale or the
generated atomic current locale, and forwards the call to the key-specific generated macro.

The message-key encoding is implemented in both the compiler and this crate. The compiler emits
the macro name, while the dispatcher reconstructs it from a literal key; both use the same
injective encoding for `_`, `-`, and `.`.

### `u18n`

[`crates/u18n/src/catalog.rs`](../crates/u18n/src/catalog.rs) defines dynamic shared-store
records, locale-local message bindings, typed argument values, value wrappers, `Catalog`, the
lightweight `Locale`, keyed `MessageData` and shard tables, and `RenderError`. Both paths offer
allocating and sink-writing methods, with formatter variants.

[`crates/u18n/src/format.rs`](../crates/u18n/src/format.rs) keeps dynamic message-ID lookup
through sorted locale bindings and explicit fallback links separate from keyed locale-index
lookup. Both provide borrowed table views to one evaluator that appends text and arguments,
selects branches, tracks the active plural value for `#`, and calls the caller's `Formatter`. A
formatter receives the selected locale's authored tag. The runtime supplies no locale-specific
number, currency, date, or time formatting policy.

[`crates/u18n/src/plural.rs`](../crates/u18n/src/plural.rs) evaluates compiled cardinal and
ordinal rules against exact decimal operands.
[`crates/u18n/src/validation.rs`](../crates/u18n/src/validation.rs) checks caller-built
locale-tag syntax throughout the fallback chain, shared table ranges, references, binding order
and completeness, fallback cycles, rule sets, branch completeness, and selector depth.
Compiler-generated dynamic tables and keyed shards use trusted constructors after compiler
validation. Application-built dynamic tables can use `Catalog::try_new` to validate rule sets and
branch completeness once before rendering; plural selection then trusts those immutable tables.
Callers using the trusted keyed constructor must supply valid rule sets and category branches.
Keyed rendering checks locale index and tag agreement, the selected program reference, argument
count, table ranges as they are used, and selector depth.

The runtime never reads locale source or parses message patterns. Compiler-generated catalogs
are immutable static data; applications do not register catalogs dynamically. Selector nesting
remains capped at 32 in runtime validation and rendering as a boundary for manually constructed
tables too.

### `u18n-cli`

[`crates/u18n-cli/src/args.rs`](../crates/u18n-cli/src/args.rs) parses the `compile`, `extract`,
and `coverage` commands and their options.
[`crates/u18n-cli/src/main.rs`](../crates/u18n-cli/src/main.rs) performs file I/O, constructs
the compiler input, invokes the public compiler API, and writes output to a file or standard
output. Locale paths are sorted before reading; source semantics and diagnostics remain in
`u18n-compiler`.

## Data ownership across phases

| Phase | Representation | Information retained |
| --- | --- | --- |
| Parse | `SchemaAst`, `LocaleAst` | Typed source structure, exact decoded text, and UTF-8 source spans. |
| Link | `LinkedCatalog` | Schema-order argument slots, validated selector/format operations, resolved fallback messages, rule-set references, and pattern author locale. |
| Lower | Dynamic store plus message-local shards | A globally interned dynamic store and, for each key, locale program choices, local tables, rule sets, and a local pool. |
| Emit | Rust source | One `SharedProgramStore`, sparse dynamic `CatalogData` views, independent `Locale` handles, one shard per key, generated macros, locale parsing, and optional atomic selection. |
| Render | `Locale` or `Catalog` plus typed arguments | Keyed lookup reads one shard; dynamic-ID lookup traverses catalog bindings. Both share table evaluation and optional formatter calls. |

This boundary keeps source concerns out of the runtime while preserving enough information
during compilation to produce precise diagnostics and translator tooling. In particular, the
fallback author locale survives linking because formatters need the catalog's selected locale,
and extraction needs to identify the origin of inherited patterns.
