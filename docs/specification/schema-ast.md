# Schema language and Schema AST

This document defines the version 1 schema source language and its strongly typed AST. The public
AST types are owned by the `u18n-ast` crate; the independent, `no_std` parser in `u18n-schema` maps
schema source directly into those nodes. `u18n-ast` defines data types only and does not parse,
validate, or lower source. The schema is the authority for message keys, parameter names, parameter
types, generated call signatures, and parameter order. It contains no localized text.

## Source file format

A schema file is UTF-8 and begins with exactly one version header:

```text
schema 1;

message app.title();
message cart.items(count: plural_count, item_name: string);
message cart.total(amount: number, currency: string);

/// The current purchase total.
message checkout.total(
    /// Exact decimal amount; the caller supplies the currency separately.
    amount: decimal,
    currency: money
);
```

Whitespace and `//` line comments may appear between tokens, including before the header and between
declarations. A comment starts at `//` and ends before the line break. A `///` line immediately
preceding a message declaration or parameter is retained as translator documentation; consecutive
documentation comments are kept in source order. Other comments remain trivia. Strings are not part
of schema version 1. Declarations end with semicolons. Parameter order is significant for generated
positional macro calls; message declaration order is not semantically significant.

## Lexical rules

The source must be valid UTF-8. Keywords and type names are ASCII and case-sensitive. Whitespace is
space, horizontal tab, carriage return, or line feed. CR, LF, and CRLF delimit line comments. No
implicit Unicode normalization is performed.

```ebnf
letter       = "A" … "Z" | "a" … "z" ;
digit        = "0" … "9" ;
name         = (letter | "_") , { letter | digit | "_" } ;
key-segment  = (letter | "_") , { letter | digit | "_" | "-" } ;
message-key  = key-segment , { "." , key-segment } ;
whitespace   = " " | "\t" | "\r" | "\n" ;
```

Names are case-sensitive. Message key segments may contain hyphens; argument names may not, because
generated APIs use them as Rust identifiers. A key cannot begin or end with a dot and cannot contain
empty segments.

In the AST, `Identifier` is a source-spanned wrapper for token text. Message-key segments use the
`key-segment` spelling rule above; parameter names use the stricter `name` rule.

## Grammar

The grammar below describes syntax after comments and whitespace have been recognized. `*` means
zero or more, `+` one or more, and `?` optional.

```ebnf
file              = whitespace* , schema-header , whitespace* , message-decl* , whitespace* , EOF ;
schema-header     = "schema" , whitespace+ , "1" , whitespace* , ";" ;
message-decl      = "message" , whitespace+ , message-key , whitespace* , "(" ,
                    whitespace* , parameter-list? , whitespace* , ")" , whitespace* , ";" ;
parameter-list    = parameter , { whitespace* , "," , whitespace* , parameter } ;
parameter         = name , whitespace* , ":" , whitespace* , argument-type ;
argument-type     = "plural_count" | "plural_decimal" | "ordinal_count" | "string" | "number"
                  | "decimal" | "money" | "date" | "time" | "instant" ;
```

There is no trailing comma in a parameter list. An empty parameter list is written `()`. The parser
rejects unknown keywords, unknown types, missing separators, and any non-comment trailing input.

## Schema AST

The parser maps tokens directly into these domain-specific node shapes in `u18n-ast`. The notation
is descriptive rather than a required Rust API; `Vec<T>` denotes an ordered owned sequence and
`String` an owned decoded identifier. `Identifier`, `MessageKey`, and `Span` are shared by the
schema and locale ASTs and are defined once in `u18n-ast`.

```text
SchemaAst {
    version: SchemaVersion,        // only V1 in this grammar
    messages: Vec<MessageDef>,
    span: Span,
}

MessageDef {
    key: MessageKey,
    parameters: Vec<ParameterDef>, // source order is generated-call order
    documentation: Vec<Documentation>,
    span: Span,
}

MessageKey {
    segments: Vec<Identifier>,
    span: Span,
}

ParameterDef {
    name: Identifier,
    argument_type: ArgumentType,
    type_span: Span,
    documentation: Vec<Documentation>,
    span: Span,
}

Documentation {
    text: String,
    span: Span,
}

Identifier {
    text: String,
    span: Span,
}

ArgumentType = PluralCount | PluralDecimal | OrdinalCount | String | Number
             | Decimal | Money | Date | Time | Instant

SchemaVersion = V1

Span {
    file_id: FileId,
    start: u32,
    end: u32,
}
```

All spans are half-open byte ranges `[start, end)` into the original UTF-8 file. Source files larger
than `u32::MAX` bytes are rejected because their positions cannot be represented by `Span`. The
message and parameter spans include their syntactic declaration; `type_span` covers only the type
keyword; identifier spans cover only their token. An empty file-end location is represented by a
zero-width span. A parser must not synthesize a successfully usable `SchemaAst` for malformed
declarations; it may recover at a declaration boundary to report multiple diagnostics, but the file
is rejected if any syntax error remains.

The AST is intentionally not a generic object/list/value tree. It has no arbitrary map, scalar
union, or untyped node. The `ArgumentType` enum is closed and matches the supported runtime value
classes.

## Semantic rules

Parsing establishes syntax; schema validation establishes these semantic properties:

1. Exactly one `schema 1;` header is present and it precedes all declarations.
2. A `MessageKey` occurs at most once in the schema. Duplicate diagnostics point to the second key
   and label the first key span.
3. A parameter name occurs at most once within a message. Duplicate diagnostics point to the
   repeated identifier and label the original declaration.
4. The parameter order in the AST is retained. Locale files bind by name, so locale pattern order
   never changes generated positional macro argument order.
5. A schema with no message declarations is valid as syntax but is rejected by the compiler as an
   empty catalog input.
6. Each schema message generates one exported macro named by prefixing `m_` to the deterministic,
   injective key encoding. Copy each ASCII letter or digit unchanged, and encode `_` as `_u`, `-` as
   `_h`, and `.` as `_s` across the complete key. Thus `cart.items` becomes `m_cart_sitems!`,
   `cart-items` becomes `m_cart_hitems!`, and `menu.file` becomes `m_menu_sfile!`. The `m_` prefix
   makes the complete macro name a legal Rust identifier even when the encoded key alone is a Rust
   keyword. Distinct source keys cannot collide under this encoding; a generated-symbol collision is
   a code-generation error.
7. A generated macro accepts either positional expressions in schema declaration order or a
   struct-like named form in braces. Named fields use the source parameter spelling and allow
   shorthand `{name}` or explicit `{name: expression}`; fields can appear in any order and may have
   a trailing comma. The shorthand requires a parameter name that can also be used as a Rust
   expression; `_`, `self`, `Self`, `super`, and `crate` require an explicit expression. Positional
   calls accept Rust expressions, including brace-delimited block expressions, require exactly the
   schema arity, and may have a trailing comma. Named calls require every
   declared parameter and reject unknown or duplicate fields. The two forms cannot be mixed. A
   zero-parameter message accepts the locale handle alone or an empty named form `{}`. Every value is
   statically checked against its schema type, including `number` values implementing
   `core::fmt::Display`; the result is `Result<String, RenderError>`.
8. If any compiled locale's resolved pattern uses a formatting operation for a message, its macro
   takes a locale handle, then a caller-provided formatter, then either argument form. Other
   messages accept only the locale handle and arguments. Each generated macro refers directly to
   that key's message shard and does not pass a global numeric message ID to the runtime. The
   expansion evaluates the locale once, then the formatter when required, then argument expressions
   once in their written order. Named values are reordered only after evaluation to construct the
   schema-ordered runtime argument slice. Wrong
   positional arity, mixed forms, and missing, unknown, or duplicate named fields are diagnosed
   during macro expansion at the relevant call-site token where possible. Rust reports type errors
   for values that do not match the generated typed argument variants.

The schema does not require every parameter to be referenced by every locale. This permits a
translation to omit a value when its wording does not need it. Referenced parameters are always
checked against this AST by the cross-AST validator.

The CLI can export translator context without adding it to generated runtime tables:

```sh
u18n-cli extract --schema schema.u18n --locale en.u18n --locale fr-CA.u18n \
  --source-locale fr-CA --output messages.json
```

Extraction sorts messages by key and includes the message description, parameter names/types/notes,
and the quoted source pattern selected from the requested locale's explicit fallback chain. It
records the pattern's author locale in `pattern_locale`, whether the pattern is authored in the
requested locale or inherited from a fallback. The output is deterministic JSON with
`schema_version: 1`; it does not change either source language or generated catalog format.

## Type meaning

| Schema type | Runtime value | Locale use |
| --- | --- | --- |
| `plural_count` | Non-negative integer count, represented by the runtime `PluralCount` wrapper over `u64` | May be interpolated or used as the selector of a cardinal plural expression, including its optional exact integer cases. |
| `plural_decimal` | Non-negative exact decimal coefficient and scale, represented by runtime `PluralDecimal`; scales from 0 through 38 preserve trailing zeroes | May be interpolated, used by a cardinal plural expression, or passed to a decimal format operation. |
| `ordinal_count` | Non-negative rank, represented by `OrdinalCount` over `u64` | May be interpolated or used by an ordinal expression. |
| `string` | UTF-8 string slice | May be interpolated as text or used as the selector of a named `select` expression. |
| `number` | Caller-formatted value implementing `core::fmt::Display` | May be interpolated. It is not eligible for plural selection. |
| `decimal` | `Decimal` coefficient and scale | Requires a decimal format operation with grouping, precision, and rounding options. |
| `money` | `Money` decimal amount and currency code | Requires a currency format operation with precision and rounding options. |
| `date` | `CivilDate` | Requires a date format operation with an explicit style. |
| `time` | `CivilTime` | Requires a time format operation with an explicit style. |
| `instant` | `Instant` | Requires an instant format operation with an explicit style and time zone. |

`plural_count`, `plural_decimal`, and `ordinal_count` are deliberately distinct from `number` and
from each other. Numeric display values do not imply grammatical count or rank semantics. `number`
remains caller-formatted through `Display`. `plural_decimal` accepts a checked non-negative
coefficient and a scale no greater than 38; it rejects negative coefficients and preserves trailing
zeroes. `decimal`, `money`, `date`, `time`, and `instant` require a matching typed locale format
operation; they cannot be interpolated as untyped display values. A `plural_decimal` may use a
decimal format operation, which passes its validated coefficient and scale to the formatter as a
runtime `Decimal` value.

`Decimal` stores a signed integer coefficient and an explicit decimal scale, preserving trailing
zeroes without binary floating point. `Money` stores a `Decimal` and a three-letter uppercase ISO
4217 code. Its constructor validates the code's shape; the caller must supply a code actually
assigned by ISO 4217. `CivilDate` is a validated Gregorian civil date; `CivilTime` is a wall-clock
time without a date or zone; `Instant` is an absolute Unix-epoch instant. Formatting options are
closed and appear in the locale pattern, so callers can supply different formatter adapters without
adding locale data to the runtime.

## Errors and source binding

Syntax errors identify the smallest unexpected token or missing-token position. Semantic diagnostics
carry a primary span and optional related spans. During cross-AST validation, the schema's
`MessageDef` and `ParameterDef` spans are preserved so an error such as “plural selector `price` has
type `number` ” can underline the locale selector and point to the schema type declaration. See
[Locale AST](locale-ast.md) for the exact binding and plural-completeness rules.
