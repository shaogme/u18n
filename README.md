# u18n

**Typed, compile-time localization catalogs for Rust.**

Define message keys and argument types in a schema, author message patterns in locale files, and
compile them into static Rust catalogs during your application build. The compiler checks the
sources and resolves message fallback before Rust compilation; the runtime does not parse
translation files or message patterns.

## Features

- Generated per-message macros check argument names, arity, and Rust types.
- Cardinal and ordinal selectors use pinned CLDR 49 rules. Exact-scale decimal counts preserve
  visible fractional digits such as `1`, `1.0`, and `1.00`.
- Locale files declare explicit fallback chains. Each inherited message comes from one complete
  pattern in the nearest fallback locale.
- Keyed message shards support linker removal of unused message data when data-section garbage
  collection is enabled. A dynamic numeric-ID catalog API is available as well.
- The `no_std` runtime uses `alloc` and supports rendering into caller-owned output sinks.
- A caller-provided formatter handles locale-specific decimal, currency, date, time, and instant
  presentation.
- The CLI compiles catalogs, exports translator context, and reports authored, inherited, and
  missing messages.

## Quick start

Add the runtime and build-time compiler to your application:

```toml
[dependencies]
u18n = "0.2.0"

[build-dependencies]
u18n-compiler = "0.2.0"
```

Create `schema.u18n`:

```text
schema 1;

message greeting(name: string);
message cart.items(count: plural_count);
```

Create `locales/en.u18n`:

```text
locale en;

message greeting = "Hello, {name}!";
message cart.items = "{count, plural, =0 {Your cart is empty.} one {# item} other {# items}}";
```

In `build.rs`, compile the sources and write the generated Rust to Cargo's `OUT_DIR`:

```rust
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
    let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is unset")?)
        .join("u18n_messages.rs");
    fs::write(output, generated.source())?;
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "cargo:rerun-if-changed=schema.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/en.u18n")?;
    Ok(())
}
```

Include the generated module and call its message macros:

```rust
use u18n::PluralCount;

include!(concat!(env!("OUT_DIR"), "/u18n_messages.rs"));

fn main() -> Result<(), u18n::RenderError> {
    let locale = &catalogs::locale_en::LOCALE;
    println!("{}", m_greeting!(locale, "Ada")?);
    println!(
        "{}",
        t!("cart.items", locale = locale, { count: PluralCount::new(2) })?
    );
    Ok(())
}
```

The [complete quickstart](docs/quickstart.md) covers multiple locales and explicit fallbacks.

## Command-line tools

Install the CLI with Cargo:

```sh
cargo install u18n-cli
```

Compile generated Rust, extract translator context for one locale, or report catalog coverage:

```sh
u18n-cli compile \
  --schema schema.u18n \
  --locale locales/en.u18n \
  --locale locales/fr-CA.u18n \
  --output catalogs.rs

u18n-cli extract \
  --schema schema.u18n \
  --locale locales/en.u18n \
  --locale locales/fr-CA.u18n \
  --source-locale fr-CA \
  --output messages.json

u18n-cli coverage \
  --schema schema.u18n \
  --locale locales/en.u18n \
  --locale locales/fr-CA.u18n \
  --output coverage.json
```

`compile` requires every schema message to resolve through each locale's explicit fallback chain.
`coverage` allows missing messages and reports the gaps; `extract` includes the author locale for
inherited patterns.

## Catalog and formatting model

Generated key macros render from a per-message shard and take a lightweight locale handle. This
keeps keyed rendering independent from the global dynamic catalog. Applications that discover
message IDs at runtime can use `LocaleTag::as_catalog()` and the `Catalog` API instead. Locale
selection with `LocaleTag::fallback` is separate from message inheritance: it chooses an included
catalog, while message inheritance follows only the fallback edges written in locale headers.

The runtime evaluates plural and select patterns, but it does not implement locale-specific number
or date formatting. For typed `decimal`, `money`, `date`, `time`, or `instant` operations, implement
`u18n::Formatter`; the runtime passes the adapter the selected locale tag, typed value, and format
options. See [advanced usage](docs/advance.md#provide-locale-aware-formatting) for an example.

## Workspace crates

| Crate | Purpose |
| --- | --- |
| [`u18n`](crates/u18n) | `no_std` runtime for generated catalogs and message rendering. |
| [`u18n-compiler`](crates/u18n-compiler) | Parser orchestration, validation, fallback resolution, and Rust code generation. |
| [`u18n-cli`](crates/u18n-cli) | `compile`, `extract`, and `coverage` commands. |
| [`u18n-schema`](crates/u18n-schema), [`u18n-locale`](crates/u18n-locale) | Independent parsers for schema and locale sources. |
| [`u18n-ast`](crates/u18n-ast), [`u18n-locale-tag`](crates/u18n-locale-tag) | Shared AST types and BCP 47 locale-tag parsing. |
| [`u18n-macros`](crates/u18n-macros) | Host-side helpers used by generated message macros. |

## Documentation

- [Quickstart](docs/quickstart.md) and [advanced usage](docs/advance.md)
- [Architecture](docs/architecture.md) and [crate designs](docs/crate-designs.md)
- [Locale and plural rule policy](docs/policy.md)
- [Schema language](docs/specification/schema-ast.md) and [locale language](docs/specification/locale-ast.md)
- [Compilation and rendering algorithms](docs/specification/algorithms.md) and [CLI reference](docs/specification/cli.md)
- [Showcase application](examples/showcase/README.md)

## License

Licensed under either [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT),
at your option.
