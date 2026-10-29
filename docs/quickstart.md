# Quickstart

This guide takes a schema and two locale files through compilation and rendering. The compiler
turns them into Rust source during the application build; the `u18n` runtime renders the
generated static catalogs.

## 1. Add the crates

Add the runtime to your application and the compiler as a build dependency:

```toml
[dependencies]
u18n = "0.2.0"

[build-dependencies]
u18n-compiler = "0.2.0"
```

Create these files in the package root:

```text
my-app/
├── build.rs
├── schema.u18n
├── locales/
│   ├── en.u18n
│   └── es-419.u18n
└── src/
    └── main.rs
```

## 2. Define messages in a schema

The schema defines message keys, parameter names and types, and positional argument order. It
contains no translated text.

```text
schema 1;

message app.title();
message cart.items(count: plural_count);
message cart.hint();
message cart.multiline();
```

`plural_count` is an unsigned integer with plural meaning. Use `number` for a value that should
be displayed using its Rust `Display` implementation rather than used in plural rules. See
[Schema AST](specification/schema-ast.md) for all supported types.

## 3. Add locale messages

`locales/en.u18n`:

```text
locale en;

message app.title = "Small Cart";
message cart.items = "{count, plural, =0 {Your cart is empty.} one {Your cart has # item.} other {Your cart has # items.}}";
message cart.hint = "Add a product to get started.";
message cart.multiline = "First line
Second line";
```

Physical LF, CRLF, and CR line breaks inside quoted patterns render as LF. Literal line breaks and
indentation are preserved inside outer text and branch patterns; selector-layout whitespace is
discarded.

`locales/es-419.u18n`:

```text
locale es-419 fallback en;

message app.title = "Carrito pequeño";
message cart.items = "{count, plural, =0 {Tu carrito está vacío.} one {Tu carrito tiene # artículo.} many {Tu carrito tiene # artículos.} other {Tu carrito tiene # artículos.}}";
```

The Spanish catalog inherits `cart.hint` from English through its explicit fallback. Fallback
resolution takes a complete message from the nearest locale that defines it. It does not infer a
fallback from the language tag or combine parts of different messages. Plural category branches
must match the full category set for the locale that authored the pattern; Spanish cardinal
rules in the pinned data include `one`, `many`, and `other`.

## 4. Compile catalogs during the build

Create `build.rs` to pass the source files to the compiler and write generated Rust to Cargo's
`OUT_DIR`:

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
        vec![
            SourceFile::new("locales/en.u18n", include_str!("locales/en.u18n")),
            SourceFile::new(
                "locales/es-419.u18n",
                include_str!("locales/es-419.u18n"),
            ),
        ],
    );
    let generated = Compiler::new(CompilerOptions::default()).compile(input)?;
    let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is unset")?)
        .join("u18n_messages.rs");
    fs::write(output, generated.source())?;

    let mut stdout = io::stdout().lock();
    writeln!(stdout, "cargo:rerun-if-changed=schema.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/en.u18n")?;
    writeln!(stdout, "cargo:rerun-if-changed=locales/es-419.u18n")?;
    Ok(())
}
```

Include the generated module at the crate root, for example at the top of `src/main.rs`:

```rust
include!(concat!(env!("OUT_DIR"), "/u18n_messages.rs"));
```

The generated source contains one dynamic catalog and one lightweight locale handle per source
locale, plus a private data shard per schema key and its generated macro. `LocaleTag` and
`LocaleTag::fallback` select a locale handle for message macros; for example,
`LocaleTag::fallback("es-AR").expect("valid request").as_locale()` selects `es-419`. Use
`as_catalog()` when calling the dynamic numeric-ID API. Runtime locale selection is separate from
locale-file fallback, which resolves message inheritance at compile time. The compiler checks
every source file and requires every message to resolve for every locale before it generates Rust.

For tag parsing, fallback order, and generated variant names, see [Advanced usage](advance.md#select-a-locale-for-each-call)
and [Locale identity policy](policy.md#runtime-catalog-selection).

## 5. Render a message

Use the generated per-message macros with a lightweight locale handle and arguments in schema
order:

```rust
use std::error::Error;

use u18n::PluralCount;

include!(concat!(env!("OUT_DIR"), "/u18n_messages.rs"));

fn main() -> Result<(), Box<dyn Error>> {
    let english = &catalogs::locale_en::LOCALE;
    let spanish_tag = "es-419"
        .parse::<LocaleTag>()
        .expect("the build includes es-419");
    let spanish = spanish_tag.as_locale();

    println!("{}", m_app_stitle!(english)?);
    println!("{}", m_cart_sitems!(english, PluralCount::new(2))?);
    println!("{}", m_app_stitle!(spanish)?);
    println!("{}", m_cart_sitems!(spanish, PluralCount::new(0))?);
    println!("{}", m_cart_shint!(spanish)?);
    Ok(())
}
```

The generated macro names encode message keys: `app.title` becomes `m_app_stitle!`, and
`cart.items` becomes `m_cart_sitems!`. You can also use named arguments when a message has
several parameters. The key-based `t!` macro dispatches by a literal key; in the default
non-atomic mode, pass a locale handle with a `locale = expression` header:

```rust
let text = t!(
    "cart.items",
    locale = spanish,
    { count: PluralCount::new(2) }
)?;
```

`LocaleTag::from_str` accepts ASCII case variations and returns the authored spelling from
`as_str()`. Source fallback targets remain case-sensitive. The Spanish `cart.hint` call above
uses the English pattern because that key is absent from the Spanish file. Key macros link only
their own message data; use `as_catalog()` and `Catalog::render(message_id, ...)` when a message
ID is discovered at runtime. The dynamic API retains the shared program store, while keyed calls
allow unused key data to be removed by linkers that garbage-collect data sections.

## CLI tools

Install

```sh
cargo install u18n-cli
```

To install a prebuilt release with [cargo-binstall](https://github.com/cargo-bins/cargo-binstall):

```sh
cargo binstall u18n-cli
```

Compile the showcase sources to generated Rust:

```sh
u18n-cli -- compile \
  --schema examples/showcase/schema.u18n \
  --locale examples/showcase/locales/en.u18n \
  --locale examples/showcase/locales/es-419.u18n \
  --output catalogs.rs
```

Export translator context for one locale, including the locale that authored each inherited
pattern:

```sh
u18n-cli -- extract \
  --schema examples/showcase/schema.u18n \
  --locale examples/showcase/locales/en.u18n \
  --locale examples/showcase/locales/es-419.u18n \
  --source-locale es-419 \
  --output messages.json
```

Report authored, inherited, and missing messages with `coverage`:

```sh
u18n-cli -- coverage \
  --schema examples/showcase/schema.u18n \
  --locale examples/showcase/locales/en.u18n \
  --locale examples/showcase/locales/es-419.u18n \
  --output coverage.json
```

For typed decimal, currency, date, time, or instant operations, provide an implementation of
`u18n::Formatter`; the runtime passes it the selected locale's source tag and the operation's
options. See the [showcase](../examples/showcase/README.md), [CLI
specification](specification/cli.md), and [locale specification](specification/locale-ast.md)
for those features and the complete source rules.
