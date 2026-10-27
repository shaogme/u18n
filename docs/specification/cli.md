# Command-line interface

`u18n-cli` compiles schema and locale source files into Rust, exports translator context, and
reports locale coverage. It uses the same parsers, semantic validation, and compiler as the Rust
API; see [Schema AST](schema-ast.md), [Locale AST](locale-ast.md), and
[Architecture](../architecture.md) for source and generated-catalog semantics.

## Invocation

```text
u18n-cli compile  --schema FILE --locale FILE... [--output FILE] [--pool-limit N] [--atomic --final-fallback TAG]
u18n-cli extract  --schema FILE --locale FILE... --source-locale TAG [--output FILE]
u18n-cli coverage --schema FILE --locale FILE... [--output FILE]
u18n-cli --help
```

Each command requires a schema path and at least one locale source path. Repeat `--locale` once for
each locale source file. Input files are read as UTF-8. The CLI sorts locale paths by their
canonicalized path before reading them, using the supplied path if canonicalization fails. Locale
identity and explicit message-fallback matching use the complete, case-sensitive source tag declared
in each file. Repeated `--schema`, `--output`, `--pool-limit`, and `--source-locale` options use the
last value; duplicate `--atomic` and `--final-fallback` options are errors.

`--pool-limit N` is parsed by all commands but only affects `compile`; `extract` and `coverage` use
their default compiler options.

`--output FILE` (or `-o FILE`) writes the command result to that path, replacing its contents. If
omitted, the generated Rust or JSON is written to standard output. `--help` and `-h` print the usage
summary to standard output when given as the first argument.

Argument and input errors, compiler diagnostics, and output-write errors are written to standard
error and cause exit status 1. Successful commands exit with status 0. Diagnostics include a source
label and location, with related source locations where available. File and parse errors use the
supplied paths; some schema-wide semantic diagnostics use `<schema>`, and an empty locale input uses
`<locales>`.

## `compile`

`compile` validates the schema and all locale files, resolves each message through the locale’s
explicit fallback chain, and writes generated Rust source. Every schema message must resolve for
every input locale. See [Schema AST](schema-ast.md) for generated message macros and
[Architecture](../architecture.md) for the generated catalog API.

```sh
u18n-cli compile \
  --schema schema.u18n \
  --locale locales/en.u18n \
  --locale locales/fr-CA.u18n \
  --output "$OUT_DIR/catalogs.rs"
```

### String-pool optimization

`--pool-limit N` sets the maximum number of distinct literal fragments for which the compiler
attempts overlap optimization. The default is `256`. Optimization is also bounded by compiler work
and byte limits; when a limit is exceeded, the compiler still emits a valid catalog and stores the
fragments without overlap merging. This option changes generated output metadata and its input
fingerprint, but not message behavior.

On success, the compiler writes separate statistics for the dynamic shared pool and the aggregate
keyed-shard pools to standard error. Keyed totals count the same fragment once for each shard that
owns it and report how many per-key pools used overlap optimization.

### Atomic locale selection

`--atomic` enables generated atomic current-locale selection for keyed message macros. It must be
paired with `--final-fallback TAG`. The tag must exactly match the source locale tag of an input
locale. The generated selection API uses that lightweight locale handle when no current locale has
been selected. `current_catalog()` and `FINAL_FALLBACK_CATALOG` remain available for dynamic-ID
callers. Atomic selection requires pointer-width atomics on the compilation target; otherwise
generated code produces a compile-time error.

```sh
u18n-cli compile \
  --schema schema.u18n \
  --locale locales/en.u18n \
  --locale locales/fr.u18n \
  --atomic \
  --final-fallback en
```

`--final-fallback` without `--atomic`, or `--atomic` without `--final-fallback TAG`, is an argument
error. These options are available only to `compile`.

## `extract`

`extract` writes deterministic JSON for one requested source locale. `--source-locale TAG` must
exactly match an input catalog. For each schema key, the tool chooses the pattern from that locale
or the nearest locale in its explicit fallback chain that defines the key. The pattern is kept whole
and its author is recorded; patterns are not assembled from fragments in multiple locales.

Extraction requires every input locale to resolve every schema key through its fallback chain, as
`compile` does. It also runs the schema and locale semantic checks. A missing source locale,
malformed input, unknown or duplicate message, invalid parameter use, or invalid fallback chain is
an error.

```sh
u18n-cli extract \
  --schema schema.u18n \
  --locale locales/en.u18n \
  --locale locales/fr-CA.u18n \
  --source-locale fr-CA \
  --output messages.json
```

The JSON has `schema_version: 1`, the requested `source_locale`, and a `messages` array sorted by
message key. Each message contains:

| Field | Meaning |
| --- | --- |
| `key` | Complete schema message key. |
| `description` | Message documentation lines joined with LF. |
| `pattern_locale` | Source locale whose file authored the selected pattern. |
| `pattern` | Exact quoted pattern spelling from that locale file, JSON-escaped. |
| `parameters` | Parameters in schema declaration order, each with `name`, `type`, and documentation `notes`. |

The output is translator context only; it does not modify either source language or generated
runtime catalogs.

## `coverage`

`coverage` writes deterministic JSON describing message resolution for every input locale. Unlike
`compile` and `extract`, it permits schema messages that are absent from a locale’s entire fallback
chain. It still validates the schema, locale syntax and semantics, fallback targets, and fallback
cycles; coverage is not a way to ignore invalid source.

```sh
u18n-cli coverage \
  --schema schema.u18n \
  --locale locales/en.u18n \
  --locale locales/fr-CA.u18n \
  --output coverage.json
```

The JSON contains a `locales` array ordered by source locale tag. Each locale has a `summary` with
counts for `authored`, `inherited`, and `missing`, plus a `messages` array ordered by key. Each
message status is:

| Status | Meaning |
| --- | --- |
| `authored` | The message is declared in this locale’s own file. |
| `inherited` | The message is selected from an explicit fallback locale. The record includes `author_locale`. |
| `missing` | The message is absent from this locale and every locale in its fallback chain. |

Missing entries are reported rather than treated as an error by this command.

## Shared validation

The CLI delegates parsing and validation to `u18n-compiler`. Schema and locale syntax follow the
linked specifications. In particular, fallbacks must name input locales and cannot form cycles; the
shared locale-tag parser checks BCP 47 structure and extension policy, while the compiler checks
source casing, registration against pinned IANA data, and plural rules from CLDR. Generated
source-locale lookup uses a case-insensitive lexical index. Each separate CLDR main-parent index row
stores one numeric `u64` key: the semantic language, script, and region code shifted left three bits,
with the fixed private `ParentLocale` discriminant in the low three bits. Rows sort by child code;
the enum selects the canonical tag after lookup. Valid request tags outside that semantic profile
skip parent lookup and continue through structural truncation. Source tags and explicit fallback
targets retain their authored spelling. The source-locale index remains string-based and has no
parent-code width limit.
`compile` and `extract` reject incomplete resolved catalogs, while `coverage` reports those gaps.
