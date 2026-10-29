# Advanced usage

This guide covers locale selection, explicit fallback chains, typed message patterns, custom
formatting, and lower-level rendering. The [Quickstart](quickstart.md) introduces schema files
and generated message macros. For the complete source grammar and validation rules, see the
[Schema AST specification](specification/schema-ast.md) and [Locale AST
specification](specification/locale-ast.md).

## Select a locale for each call

Each generated locale has an independent lightweight handle for keyed calls and a full catalog for
dynamic message IDs. Parse a tag with `LocaleTag`, then select the handle for generated message
macros:

```rust
use core::str::FromStr;

use u18n::PluralCount;
use u18n_showcase::{LocaleTag, t};

let tag = LocaleTag::from_str("es-419").expect("locale is included");
let locale = tag.as_locale();
let text = t!(
    "cart.items",
    locale = locale,
    { count: PluralCount::new(3), owner: "Ada", gender: "other" }
)?;
```

`LocaleTag` is a public enum with one variant for each locale supplied to the compiler. Variant
identifiers concatenate the normalized subtags in PascalCase; for example, `en` is `En`,
`es-419` is `Es419`, and `zh-Hant-HK` is `ZhHantHk`. The enum can be matched exhaustively, and
adding or removing a source locale can change that match. Source locales are looked up in a
case-insensitive, string-sorted index and are not limited by the semantic-code profile. A separate
CLDR parent index stores one numeric `u64` key per child. Bits 3–55 hold the bounded semantic
language, script, and region code; bits 0–2 hold the matching fixed private `ParentLocale`
discriminant. Rows sort by child code, and the enum supplies the canonical parent tag after lookup.
`LocaleTag::from_str` accepts any
ASCII casing of an included source tag. It validates BCP 47 structure and version 2's extension
policy, but does not carry the IANA registry for runtime registration checks. A well-formed tag
without an exact included catalog returns an unsupported-tag error. To map a requested tag to an
included catalog, use `LocaleTag::fallback`:

```rust
let requested = LocaleTag::fallback("es-AR").expect("valid locale request");
let locale = requested.as_locale();
assert_eq!(requested.as_str(), "es-419");
```

This helper first checks for an exact source-locale match, then checks CLDR 49 main-component
parent mappings for the current candidate, and then removes a valid trailing component. It
removes a private-use tail as a unit and treats grandfathered tags as atomic candidates.
Malformed or extension-rejected input returns an error; a valid request with no included
candidate selects the lexicographically first generated source locale. Tags outside the bounded
semantic profile can match exactly and otherwise continue through structural truncation. The
helper does not apply CLDR aliases or add likely subtags. You can also refer to a catalog directly
as `catalogs::locale_es_419::CATALOG` for the dynamic-ID API. `LocaleTag::as_catalog()` selects
that full catalog; `LocaleTag::as_locale()` selects the small handle used by generated key macros.

## Call generated message macros

In the default build configuration, `t!` requires a `locale = expression` header. A per-message
macro takes a `&Locale` handle as its first argument:

```rust
let text = m_cart_sitems!(
    locale,
    PluralCount::new(3),
    "Ada",
    "other"
)?;
```

Schema parameter order controls positional calls. For long signatures, use named fields in any
order; the names must match the schema, and every declared parameter must be supplied:

```rust
let text = m_checkout_sorder_usummary!(
    locale,
    {
        currency: "USD",
        total: 24.50,
        quantity: PluralCount::new(2),
        product: "mugs",
        customer: "Ada",
    }
)?;
```

Named expressions are evaluated once, in the order written, then arranged in schema order for
rendering. A locale may omit a declared parameter from its wording, but call sites still provide
the full schema signature. Use `t!` when dispatching by a literal message key is more convenient
than naming the generated macro. See [Schema semantic
rules](specification/schema-ast.md#semantic-rules) for macro argument forms and name encoding.

## Optional process-wide locale selection

For applications that want `t!` calls to use a current locale without a locale header, enable
atomic selection when compiling the catalogs and choose the locale used before any locale is
set. Configure this in the build script:

```rust
use u18n_compiler::{Compiler, CompilerOptions};

let options = CompilerOptions::default()
    .with_atomic_locale(true)
    .with_final_fallback_locale("en");
let generated = Compiler::new(options).compile(input)?;
```

The generated API provides `set_current_locale`, `clear_current_locale`, `current_locale`, and
`FINAL_FALLBACK_LOCALE` for keyed calls. With atomic selection enabled, `t!("app.title")` uses the
current locale handle, or the configured final fallback after `clear_current_locale()`. An
explicit `locale = expression` header overrides the current selection. `current_catalog()` and
`FINAL_FALLBACK_CATALOG` remain available for dynamic-ID callers. The feature requires
pointer-width atomics on the target.

The selected locale is process-wide. A change affects all `t!` calls that omit an explicit
locale, so use explicit locale selection when different requests or users need different
locales at the same time. The build option is available from both `CompilerOptions` and the CLI;
the [CLI specification](specification/cli.md#atomic-locale-selection) describes both forms.

## Build explicit fallback chains

Fallback is authored in each locale header. For example:

```text
locale en;
locale fr fallback en;
locale fr-CA fallback fr;
```

For every schema key, the compiler chooses the nearest declaration starting from the selected
locale and following these explicit edges. If `fr-CA` does not define a key, it uses `fr` when
available, then `en`. A resolved message is always taken as one complete pattern from one file;
text and branches are never merged across locales. Every schema key must resolve for every
catalog at compile time.

Fallback tags must exactly match an input catalog and the graph must be acyclic. The compiler
does not infer message fallback from a tag such as `fr-CA`; `LocaleTag::fallback` only chooses
an included runtime catalog and does not provide inherited message patterns. Plural-rule lookup
has its own CLDR locale resolution and does not change locale identity or message fallback
behavior. If a message is inherited, its pattern keeps the plural rules of the locale that
authored it, while a formatter receives the selected locale's source tag. See [Locale and
plural rule policy](policy.md) for this distinction.

Dynamic `Catalog` views store only the message bindings authored in that locale. At dynamic-ID
render time, a missing binding follows the direct fallback link until the program is found. Keyed
message shards instead contain a pre-resolved program choice for every generated locale, so the
keyed renderer does not traverse fallback links. Both paths use the complete whole pattern chosen
at compile time and preserve its author-locale plural rules. The selected locale tag still goes
to the formatter.

## Use selectors and typed values

Choose schema types according to the meaning of a value, not just its Rust representation:

| Schema type | Use it for | Pattern operations |
| --- | --- | --- |
| `plural_count` | A non-negative integer quantity | Interpolation and cardinal `plural` |
| `plural_decimal` | An exact, non-negative decimal quantity with meaningful visible scale | Interpolation, cardinal `plural`, or decimal formatting |
| `ordinal_count` | An integer rank or position | Interpolation and `ordinal` |
| `string` | Text or a named choice | Interpolation and `select` |
| `number` | A value displayed by its Rust `Display` implementation | Interpolation only |
| `decimal`, `money`, `date`, `time`, `instant` | Values requiring locale-aware presentation | Their corresponding typed `format` operation |

For example, a cardinal selector can provide exact cases and CLDR category branches, while a
`select` chooses wording for a string value:

```text
message cart.items(count: plural_count, owner: string, gender: string);
```

```text
message cart.items = "{owner} has {count, plural, =0 {no items} one {# item} other {# items}}. {gender, select, female {She has it covered.} male {He has it covered.} other {They have it covered.}}";
```

Exact plural cases such as `=0` are checked before categories. Category branches must contain
exactly the full set assigned to the locale and rule family, each once; `other` alone is not
always a valid plural. Category names are grammatical rules, not fixed number labels. A cardinal
`plural_decimal` preserves trailing zeroes, so `1`, `1.0`, and `1.00` can have different CLDR
operands and remain distinct when rendered. Ordinal selectors accept `ordinal_count` only.
Select values match case-sensitively and require an `other` branch.

Patterns can span source lines. Line breaks and indentation in literal text and branch patterns are
preserved (with CRLF and CR normalized to LF); newlines used to lay out selector syntax are
discarded.

In the schema:

```text
message cart.summary(count: plural_count);
```

In a locale catalog:

```text
message cart.summary = "Cart:
{count, plural,
    one {# item
    in the cart}
    other {# items
    in the cart}
}";
```

Inside a plural or ordinal branch, `#` renders that selector's value without grouping. A nested
plural or ordinal makes its own value active; nested `select` branches keep the enclosing value
active. Write `\#` for a literal hash inside a branch. Selector nesting is limited to 32 levels.
Full branch, escape, and type rules are in the [Locale AST
specification](specification/locale-ast.md#cross-ast-binding-and-type-checks).

## Provide locale-aware formatting

The runtime deliberately does not decide how to format decimals, money, dates, times, or
instants. Implement `u18n::Formatter` and pass it to a generated macro for any message whose
resolved pattern uses a typed `format` operation:

```text
message invoice.total(amount: decimal, charge: money);
```

```text
message invoice.total = "Total: {amount, format, decimal, grouping: auto, precision: 2, rounding: half_even}; charged {charge, format, currency, precision: currency, rounding: half_even}";
```

```rust
let amount = Decimal::new(1995, 2);
let charge = Money::new(Decimal::new(1234, 2), "USD")
    .expect("three-letter uppercase currency code");
let formatter = AppFormatter; // Implements u18n::Formatter.

let text = m_invoice_stotal!(locale, &formatter, amount, charge)?;
```

The formatter receives a `FormatRequest` containing the typed value, closed format options, and
the selected locale's exact source tag. It writes to the same output sink as the message
renderer and can return `FormatterError::Unsupported` for requests it cannot handle; rendering
reports formatter errors as `RenderError::FormatterFailed`. `Money::new` checks that a currency
code has three uppercase ASCII letters. The caller must also supply a code assigned by ISO 4217.

Any message that uses a format operation in at least one compiled locale requires a formatter
argument, even if the selected locale's resolved pattern does not use formatting. The [showcase
formatter](../examples/showcase/src/main.rs) demonstrates the trait, and the [Locale AST
specification](specification/locale-ast.md#grammar) lists all format operations and options.

## Stream output through the runtime

Generated message macros return `Result<String, RenderError>`. A `MessageData` handle also offers
`render_to` and `render_with_to` for keyed rendering into a caller-owned `core::fmt::Write` sink.
Code that manages numeric message IDs can use the dynamic catalog methods:

```rust
let mut output = String::new();
catalog.render_to(message_id, &arguments, &mut output)?;
```

Use `render_with_to` when the pattern contains typed format operations:

```rust
let mut output = String::new();
catalog.render_with_to(&formatter, message_id, &arguments, &mut output)?;
```

The argument slice is in schema order. A formatter or output failure can leave a partial prefix
in the sink. `render_to` returns `FormatterRequired` for a message with a format instruction;
the formatted variant accepts a `Formatter`. Applications that construct `CatalogData` and
`SharedProgramStore` themselves should validate the root view once with `Catalog::try_new`
before rendering. That validation checks each rule set and each plural node's complete category
branches; subsequent rendering relies on those immutable invariants. Callers that construct keyed
shards with `MessageData::new_trusted` must provide valid rule sets and branches as well. Generated
keyed calls check the selected locale index and tag, program range, argument count, table references
used during rendering, and selector depth. See the [`Catalog` API](../crates/u18n/src/catalog.rs)
and [runtime rendering algorithm](specification/algorithms.md#runtime-rendering) for the table
contracts.

## Use extraction and coverage in translation workflows

`extract` exports deterministic JSON for a requested locale, including schema descriptions,
parameter notes, the selected whole pattern, and `pattern_locale` for inherited messages:

```sh
u18n-cli extract \
  --schema schema.u18n \
  --locale locales/en.u18n \
  --locale locales/fr-CA.u18n \
  --source-locale fr-CA \
  --output messages.json
```

`coverage` reports authored, inherited, and missing keys for every locale. Unlike `compile` and
`extract`, it allows missing keys so translators can see incomplete fallback chains; it still
checks syntax, types, locale tags, and fallback validity:

```sh
u18n-cli coverage \
  --schema schema.u18n \
  --locale locales/en.u18n \
  --locale locales/fr-CA.u18n \
  --output coverage.json
```

Use `compile` as the final completeness check: every catalog must resolve every schema key. The
[CLI specification](specification/cli.md) documents the JSON fields and command behavior.

## Tune generated catalogs

The compiler builds a deterministic shared UTF-8 pool for dynamic-ID rendering and one pool
inside each message shard. `--pool-limit N` controls overlap optimization for each pool; exceeding
its work or byte limits disables that pool's optimization while keeping the data valid. Pool
tuning changes generated output metadata and the input fingerprint, but not rendered messages.

Build scripts can set the same option with `CompilerOptions::with_pool_limit` and inspect both
the shared-pool and aggregate keyed-pool values in `GeneratedSource::pool_statistics()`. The
[algorithms
specification](specification/algorithms.md#shared-and-keyed-string-pools) describes the optimization limits
and fingerprint inputs. Most applications can use the default limit.

## Further reading

- [Architecture](architecture.md): compile-time and runtime boundaries.
- [Schema AST](specification/schema-ast.md): parameter types, generated signatures, and argument
  evaluation.
- [Locale AST](specification/locale-ast.md): grammar, locale validation, pattern AST, and
  cross-AST checks.
- [Compilation and rendering algorithms](specification/algorithms.md): fallback resolution,
  lowering, pooling, and runtime evaluation.
- [CLI specification](specification/cli.md): compile, extract, coverage, and atomic selection
  options.
