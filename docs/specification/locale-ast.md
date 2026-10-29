# Locale language and Locale AST

This document defines version 2 locale source syntax, its domain-specific AST, and the rules used to
bind it to the schema. The public AST types are owned by `u18n-ast`; the independent, `no_std`
parser in `u18n-locale` maps locale source directly into them and uses `u18n-locale-tag` for shared
BCP 47 syntax checks. `u18n-ast` defines data types only and does not parse, validate, or lower
source. A locale file contains a catalog for one valid BCP 47 locale tag using the recommended
casing and may name one explicit fallback locale for message inheritance. Generated code also
provides `LocaleTag::fallback` as a separate runtime catalog-selection helper based on CLDR 49
main-component parent data. Patterns are parsed into tokens; localized text is never retained as an
opaque message string for the runtime to parse. Locale files have no language-version field; the
compiler release selects this version 2 grammar, with no version 1 compatibility mode.

## Source file format

A locale file is UTF-8 and begins with one locale header. Each schema key is resolved from the
nearest declaration in that locale's fallback chain:

```text
locale en;
locale fr-CA fallback fr;

message app.title = "Cart";
message cart.items = "{count, plural, one {# item} other {# items}} for {item_name}";
message cart.total = "Total: {amount} {currency}";
```

Whitespace and `//` line comments are allowed outside quoted message patterns. A line comment ends
at CR, LF, or CRLF. Message patterns are double-quoted and may contain physical LF, CRLF, or CR
line breaks. Each logical line break renders as LF; selector-layout line breaks are discarded,
while line breaks and indentation in literal pattern regions are preserved. Declarations end with
semicolons. A fallback target is a complete source locale tag,
matched case-sensitively against the compilation input set; it is never inferred from BCP 47
parents.

## Lexical rules and escapes

The shared `u18n-locale-tag` parser checks BCP 47 structure for source headers, explicit fallback
targets, generated API requests, and checked caller-built catalogs. It accepts ASCII case
variations, but source locale tags must use recommended casing: language, extlang, variant, and
private-use subtags are lower-case; script subtags are title-case; alphabetic region subtags are
upper-case; numeric region subtags are unchanged. Underscores are forbidden. The grammar follows
[RFC 5646 §2.1](https://www.rfc-editor.org/rfc/rfc5646.html#section-2.1), including ordered
language, extlang, script, region, variant, and private-use components. This accepts numeric region
and variant subtags such as `es-419` and `de-CH-1901`. Version 2 rejects all extension sequences,
including Unicode (`u`), transformed (`t`), and other singleton extensions. Registered grandfathered
tags and the RFC private-use sequence introduced by `x` remain accepted. The compiler additionally
requires the complete tag to be registered as grandfathered or its language, extlang, script,
region, and variant subtags to be registered in the vendored
[IANA Language Subtag Registry snapshot](../../crates/u18n-compiler/data/iana-language-subtag-registry.txt)
(`File-Date: 2026-09-17`, SHA-256 `755fad43283be7b41ebe3c89ad054b6eaf928f404f9c0edb74799e0eab74beb1`).
Variant subtags cannot repeat. If a registered variant has IANA `Prefix` fields, at least one must
match the beginning of the preceding tag subtags; variants without a registered prefix have no
additional prefix check. A well-formed tag with an unregistered ordinary subtag fails compilation;
private-use subtags are exempt from registration checks. Runtime syntax validation does not imply
IANA registration.

The authored source spelling remains the locale identity and is retained by `LocaleTag::as_str()`,
the lightweight `Locale` handle, and `CatalogData.locale_tag`. `LocaleTag::as_locale()` selects the
handle used by generated key macros; `LocaleTag::as_catalog()` selects the dynamic-ID catalog.
Keyed macros use their own message shard and do not retain the global dynamic store. The generated
source-locale index is sorted by lowercase ASCII tag
and searched with an ASCII case-insensitive comparison; it has no parent-code width limit.
Source tags that differ only by ASCII case are rejected as duplicate normalized lookup tags. The
separate CLDR main-parent index stores one numeric `u64` key per row. Bits 3–55 encode the bounded
language, optional script, and optional region code; bits 0–2 encode the matching fixed private
`ParentLocale` discriminant, and bits 56–63 remain zero. The child layout supports 2- or 3-letter
languages, 4-letter scripts, and 2-letter or 3-digit regions. Rows sort by child code and contain no
child-tag string records. The enum uses stable `repr(u8)` discriminants and provides canonical
parent spellings after lookup.
The compiler requires the pinned 198-child, eight-parent CLDR 49 main-parent map to fit the layout
and rejects duplicate child codes. `fallback` checks for an exact source locale before consulting
this index; a candidate outside the semantic profile skips parent lookup and continues with
structural truncation. CLDR aliases and parent data do not rewrite catalog identity.
Private-use subtags remain part of the exact identity but are dropped as a whole when runtime
fallback truncates a request; plural-rule lookup removes private use before resolving CLDR rules.

Message keys use the same spelling rules as schema keys. Argument names use the schema's ASCII name
rules. Identifiers are case-sensitive. The source is not Unicode-normalized; decoded text preserves
the exact authored scalar sequence.

Generated `LocaleTag` is a public closed enum with one unit variant per source locale in the
generated module. Variant names concatenate normalized subtags, uppercasing each subtag's first
ASCII letter and lowercasing its remaining letters. For example, `en` becomes `En`, `es-419` becomes
`Es419`, `en-x-private` becomes `EnXPrivate`, and `zh-Hant-HK` becomes `ZhHantHk`. Variant-name
collisions are rejected before source emission. The variant set depends on the locale inputs, and
callers can match exhaustively on it. `from_str` accepts any ASCII casing of an included source tag
and distinguishes malformed or extension-rejected requests from valid tags without an exact catalog.
`fallback` returns a parse error for malformed input; it first checks the case-insensitive
source-locale index, then the numeric semantic CLDR main-parent index when the candidate is
representable, then tries structural truncation. It returns the lexicographically first source
locale if no candidate matches. Each parent-index `u64` combines a child code and parent
discriminant; the enum provides canonical BCP 47 strings for the fallback loop to parse. Private-use
tails are truncated as a unit, and grandfathered tags are atomic candidates. This enum is separate
from `ParsedTag`, the borrowed syntax parser, and from locale-tag strings in runtime catalogs and
formatter requests. The runtime parser checks syntax but does not repeat IANA registration checks.

Within a quoted pattern or plural branch, the following escapes are recognized:

| Escape | Decoded text |
| --- | --- |
| `\\` | Backslash |
| `\"` | Double quote |
| `\n`, `\r` | One logical line break rendered as LF |
| `\t` | Tab |
| `\{`, `\}` | Literal opening or closing brace |
| `\#` | Literal `#` (needed inside plural branches) |
| `\u{HEX}` | One Unicode scalar value, with 1–6 hexadecimal digits; U+000A and U+000D are logical line breaks |

Invalid Unicode scalar values, malformed escapes, and unescaped structural braces in literal text
are errors. A raw `#` is ordinary text except inside a plural
branch, where it inserts that plural selector's integer value. Escaped hashes are always literal
text, whether written as `\#` or `\u{23}`.

## Grammar

The following grammar operates on tokens after whitespace and line comments have been discarded
outside quoted patterns, while preserving original source spans. `pattern` is the outer quoted
pattern; a `branch-pattern` is delimited by braces and may recursively contain interpolation or
selectors. Each `bcp47-subtag` token is limited to 1–8 characters. The `locale-tag` production
recognizes only the lexical candidate shape; the complete candidate must also pass RFC 5646 §2.1
`Language-Tag` validation and the version 2 extension-sequence restriction above.

```ebnf
letter            = "A" … "Z" | "a" … "z" ;
digit             = "0" … "9" ;
hex-digit         = digit | "A" … "F" | "a" … "f" ;
name              = (letter | "_") , { letter | digit | "_" } ;
key-segment       = (letter | "_") , { letter | digit | "_" | "-" } ;
message-key       = key-segment , { "." , key-segment } ;
line-break        = "\n" | "\r\n" | "\r" ;
whitespace        = " " | "\t" | line-break ;
pattern-space     = " " | "\t" | line-break ;
backslash         = U+005C ;
file              = whitespace* , locale-header , whitespace* , message-decl* , whitespace* , EOF ;
locale-header     = "locale" , whitespace+ , locale-tag , [ whitespace+ , "fallback" , whitespace+ , locale-tag ] , whitespace* , ";" ;
locale-tag        = bcp47-subtag , { "-" , bcp47-subtag } ;
bcp47-subtag      = alphanum , { alphanum } ;
alphanum          = letter | digit ;
message-decl      = "message" , whitespace+ , message-key , whitespace* , "=" , whitespace* ,
                    pattern , whitespace* , ";" ;

pattern           = "\"" , top-level-part* , "\"" ;
branch-pattern    = "{" , branch-part* , "}" ;
top-level-part    = top-level-text | line-break | escape | interpolation | plural-expression | ordinal-expression | select-expression | format-expression ;
branch-part       = branch-text | line-break | escape | interpolation | plural-expression | ordinal-expression | select-expression | format-expression | pound ;
interpolation     = "{" , pattern-space* , name , pattern-space* , "}" ;
plural-expression = "{" , pattern-space* , name , pattern-space* , "," , pattern-space* , "plural" ,
                    pattern-space* , "," , pattern-space* , plural-branch ,
                    { pattern-space* , plural-branch } , pattern-space* , "}" ;
ordinal-expression = "{" , pattern-space* , name , pattern-space* , "," , pattern-space* , "ordinal" ,
                     pattern-space* , "," , pattern-space* , plural-branch ,
                     { pattern-space* , plural-branch } , pattern-space* , "}" ;
plural-branch     = exact-case , pattern-space* , branch-pattern
                    | plural-category , pattern-space* , branch-pattern ;
exact-case        = "=" , digit , { digit } ;
select-expression = "{" , pattern-space* , name , pattern-space* , "," , pattern-space* , "select" ,
                    pattern-space* , "," , pattern-space* , select-branch ,
                    { pattern-space* , select-branch } , pattern-space* , "}" ;
select-branch     = name , pattern-space* , branch-pattern ;
format-expression = "{" , pattern-space* , name , pattern-space* , "," , pattern-space* , "format" ,
                    pattern-space* , "," , pattern-space* , format-options , pattern-space* , "}" ;
format-options    = decimal-options | currency-options | date-options | time-options | instant-options ;
decimal-options   = "decimal" , pattern-space* , "," , pattern-space* , "grouping" , pattern-space* , ":" , pattern-space* ,
                    ("none" | "auto") , pattern-space* , "," , pattern-space* , "precision" , pattern-space* , ":" ,
                    pattern-space* , ("input" | digit , [digit]) , pattern-space* , "," , pattern-space* , "rounding" ,
                    pattern-space* , ":" , pattern-space* , rounding-mode ;
currency-options  = "currency" , pattern-space* , "," , pattern-space* , "precision" , pattern-space* , ":" ,
                    pattern-space* , ("currency" | digit , [digit]) , pattern-space* , "," , pattern-space* , "rounding" ,
                    pattern-space* , ":" , pattern-space* , rounding-mode ;
date-options      = "date" , pattern-space* , "," , pattern-space* , "style" , pattern-space* , ":" , pattern-space* , format-style ;
time-options      = "time" , pattern-space* , "," , pattern-space* , "style" , pattern-space* , ":" , pattern-space* , format-style ;
instant-options   = "instant" , pattern-space* , "," , pattern-space* , "style" , pattern-space* , ":" , pattern-space* ,
                    format-style , pattern-space* , "," , pattern-space* , "time_zone" , pattern-space* , ":" ,
                    pattern-space* , escaped-zone-string ;
format-style      = "short" | "medium" | "long" | "full" ;
rounding-mode     = "half_even" | "half_up" | "toward_zero" | "away_from_zero" ;
escaped-zone-string = backslash , "\"" , zone-character+ , backslash , "\"" ;
zone-character    = letter | digit | "_" | "-" | "+" | "/" | "." ;
plural-category   = "zero" | "one" | "two" | "few" | "many" | "other" ;
pound             = "#" ;
escape            = backslash , ( backslash | "\"" | "n" | "r" | "t" | "{" | "}" | "#"
                    | "u{" , hex-digit , [hex-digit] , [hex-digit] , [hex-digit] ,
                      [hex-digit] , [hex-digit] , "}" ) ;
```

`top-level-text` is any Unicode scalar other than a quote, backslash, unescaped structural brace,
CR, or LF. `branch-text` has the same rule and also excludes `#`, which is parsed as `pound`.
Escapes are interpreted before the AST token is finalized, so escaped structural characters always
remain literal text. `\n`, `\r`, `\u{A}`, and `\u{D}` each produce one `LineBreak` node. Other
Unicode escapes produce their scalar value; U+0085, U+2028, and U+2029 are ordinary text. CRLF is
one line break whose span covers both source bytes. The parser distinguishes interpolation and
operations after reading the opening brace and identifier. `pattern-space` is discarded around
selector punctuation, option syntax, branch labels, and selector closure. Once a branch pattern
opens, its line breaks, leading and trailing spaces, and indentation are preserved exactly. At top
level `#` is ordinary text. The `u{...}` escape must contain 1–6 hexadecimal digits and encode a
valid Unicode scalar; surrogate values and values above U+10FFFF are rejected.
Decimal and currency precision digits are limited to 0–9; `input` preserves the decimal's scale and
`currency` delegates default precision to the formatter. The formatter receives rounding explicitly.
Time zones use an escaped quoted ASCII identifier such as `\"America/New_York\"`.

Plural and ordinal branches are adjacent exact-case-or-category and pattern clauses with optional
`pattern-space` between them; commas and bars are not branch separators. Exact cases are unsigned
base-10 `u64` values with no sign. Leading zeroes are accepted, but spellings that parse to the same
integer are duplicate cases. Exact cases are optional and checked before their rule family's
category rules. Select branches are adjacent identifier-and-pattern clauses; case names are
case-sensitive and `other` is mandatory. Nested selectors are permitted. A `#` in nested branches
refers to the innermost active cardinal or ordinal selector; a selector inside a `select` branch
does not change that active value. An outer selector can still be interpolated by name. The parser
limits patterns to 32 plural, ordinal, or select expressions along any branch path; a 33rd nested
selector is a syntax error reported at that selector. Runtime catalog validation and rendering
enforce the same maximum and return `InvalidCatalog` for an over-nested catalog.

## Locale AST

The parser maps the locale source directly into these typed nodes in `u18n-ast`. The notation is
descriptive, not a required Rust API. `Identifier`, `MessageKey`, and `Span` are shared with the
schema AST and defined once in `u18n-ast`.

```text
LocaleAst {
    locale: LocaleId,
    fallback: Option<LocaleId>,
    locale_span: Span,
    messages: Vec<LocalizedMessage>,
    span: Span,
}

LocaleId {
    source_tag: String,
    span: Span,
}

LocalizedMessage {
    key: MessageKey,
    pattern: Pattern,
    span: Span,
}

Pattern {
    parts: Vec<PatternPart>,
    span: Span,
}

PatternPart =
    | Text { value: String, span: Span }
    | LineBreak { span: Span }
    | Argument { name: Identifier, span: Span }
    | Plural { name: Identifier, kind: PluralKind, branches: Vec<PluralBranch>, span: Span }
    | Select { name: Identifier, branches: Vec<SelectBranch>, span: Span }
    | Format { name: Identifier, options: FormatOptions, span: Span }
    | Pound { span: Span }

PluralBranch {
    case: PluralCase,
    case_span: Span,
    pattern: Pattern,
    span: Span,
}

PluralCase = Exact(u64) | Category(PluralCategory)

PluralKind = Cardinal | Ordinal

FormatOptions = Decimal { grouping: Grouping, precision: Precision, rounding: Rounding }
              | Currency { precision: CurrencyPrecision, rounding: Rounding }
              | Date { style: FormatStyle }
              | Time { style: FormatStyle }
              | Instant { style: FormatStyle, time_zone: String }

Grouping = None | Auto
Precision = Input | Fixed(u8)
CurrencyPrecision = CurrencyDefault | Fixed(u8)
Rounding = HalfEven | HalfUp | TowardZero | AwayFromZero
FormatStyle = Short | Medium | Long | Full

SelectBranch {
    name: Identifier,
    pattern: Pattern,
    span: Span,
}

PluralCategory = Zero | One | Two | Few | Many | Other

Identifier { text: String, span: Span }
MessageKey { segments: Vec<Identifier>, span: Span }
Span { file_id: FileId, start: u32, end: u32 }
```

`source_tag` stores the spelling accepted by the locale-tag syntax parser; the compiler additionally
checks source casing and IANA registration. It is not replaced with the locale chosen for CLDR rule
lookup or runtime catalog selection. `fallback` stores the explicitly authored target tag and its
source span; it is not canonicalized or inferred. `Text` nodes contain decoded literal text and
never contain CR or LF; `LineBreak` represents a hard break and always renders as LF. Escaped
line-break spans cover the complete escape spelling, and raw CRLF spans cover both bytes. Adjacent
literal text nodes may be coalesced only if their combined span remains a truthful source range. `Argument` identifies plain interpolation. `Plural` contains exact cases and typed category
branches; its `kind` selects cardinal or ordinal rules. `Format` contains a typed formatting
operation and its closed options. `Select` contains named string cases. `Pound` is retained as a
distinct node rather than rewritten to an argument so its binding to the innermost plural or ordinal
selector remains explicit. Every node span is a half-open UTF-8 byte range into the original source;
escaped characters' spans cover their full source escape. Source files larger than `u32::MAX` bytes
are rejected because their positions cannot be represented by `Span`.

Version 2 adds `LineBreak` to the public `PatternPart` enum and `Instruction` enum. Downstream code
that exhaustively matches either enum must handle the new variant, and generated catalogs must be
rebuilt with the matching compiler and runtime. The `\r` escape now renders as LF along with `\n`
and the Unicode line-break escapes.

The AST is purpose-built for messages. It does not contain generic map/list/value nodes or infer
structure later from strings. A malformed file is rejected; parser recovery may continue at the next
top-level `message` boundary after a closed declaration to report more than one syntax error. If a
quoted pattern reaches EOF without closing, recovery stops at EOF rather than treating message-like
content inside the quote as declarations.

## Cross-AST binding and type checks

The locale AST is validated against the `SchemaAst` described in [Schema AST](schema-ast.md):

1. The locale tag must be a valid BCP 47 tag using the recommended casing and contain no extension
   sequence, as required by version 2. Cardinal rules must resolve for every locale in the pinned
   CLDR 49 data. The compiler also looks up ordinal rules for every locale; an unresolved ordinal
   rule set is an error if a pattern uses an ordinal selector. Validate the source spelling against
   the pinned IANA Language Subtag Registry snapshot, then preserve the complete source tag as the
   catalog identity. For plural-rule lookup, first remove the private-use sequence, if present, then
   apply the pinned CLDR language, script, territory, and variant aliases. Starting from that CLDR
   locale ID, follow an explicit parent from the `plurals` component's parent-locale map when
   present; otherwise truncate the rightmost locale subtag and continue. Component-specific parent
   mappings are standalone: do not merge or apply the main component's parent-locale mappings,
   including `localeRules="nonlikelyScript"`. CLDR 49's `plurals` parent-locale block is empty, so
   `az-Arab` checks `az-Arab` and then `az`, resolving to `az`'s cardinal categories rather than the
   main component's `root` parent. A locale with an assigned `other`-only rule set resolves
   successfully. Exhausting a tag's subtags does not make an unknown locale inherit `root` rules.
   Cardinal and ordinal lookups use this resolution algorithm independently.
2. A compilation input set must contain at least one locale file. An empty locale input set is a
   compile error because it cannot provide a selectable generated catalog.
3. A compilation input set cannot contain duplicate source tags or distinct source spellings with
   the same normalized locale key.
4. Fallback targets must name a locale in the input set, and the directed fallback graph must be
   acyclic. For each schema key, the compiler selects the nearest declaration while following that
   explicit chain. A key absent from the entire chain is an error. Unknown keys and duplicates are
   errors in every source locale, including declarations shadowed by a more specific locale.
5. Every `Argument.name`, `Plural.name`, `Select.name`, and `Format.name` must match a parameter in
   that message's schema signature. A typo is reported at the locale identifier span; the linked
   program stores schema-order parameter slots, not names.
6. A plain `{name}` interpolation accepts `string`, `number`, `plural_count`, `plural_decimal`, or
   `ordinal_count`. Counts and ranks render as unsigned base-10 digits without grouping or
   locale-specific formatting; a `plural_decimal` renders as an ungrouped decimal preserving its
   visible scale; a `number` uses the caller's `Display` formatting. Typed values `decimal`, `money`,
   `date`, `time`, and `instant` require their matching format operation. A `plural_decimal` may
   also use the decimal format operation.
7. A cardinal plural selector accepts `plural_count` or `plural_decimal`; an ordinal selector
   remains restricted to `ordinal_count`. Exact cases are optional and unique, and category branches
   must match the authored locale's complete CLDR category set for that selector kind. Exact matches
   are checked before category rules. A decimal with zero fractional value matches an equivalent
   exact integer case.
8. A `select` selector requires `string`. Named cases are unique and `other` is mandatory; unmatched
   strings select `other`.
9. `Pound` can occur only inside a plural or ordinal branch. It renders the active innermost
   selector, including through nested `select` branches. To include a literal hash inside a branch,
   use `\#`; at top level, an unescaped `#` is ordinary text.
10. A declared parameter need not appear in the locale pattern. Repeated interpolation and repeated
    selectors of a parameter are allowed.
11. A resolved message pattern is validated and retained as a whole from the locale that authored
    it. Inherited patterns keep that origin locale's plural rule-set reference; patterns are never
    spliced across locales.

## Plural branch semantics and completeness

The compiler validates each plural expression against the authored locale's category set from the
matching CLDR 49 rule family: `plural` uses cardinal rules and `ordinal` uses ordinal rules.
Category branches must contain every category in that set exactly once and no category outside it.
`other` is mandatory. Optional exact integer branches are unique and checked before categories. The
category names are grammatical labels, not exact-number cases: for example, `one` is selected by the
locale's rule and does not necessarily mean only the integer 1, while `zero` is not an automatic
exact-zero branch. This strict exact-set rule prevents silent dead branches and missing branches
that would otherwise fall through.

CLDR's six category names are `zero`, `one`, `two`, `few`, `many`, and `other`; the set is
locale-specific. `Two` is included even though a five-category list is common in simplified
descriptions. `other` handles every value not matched by another category, and remains required even
in locales with only one category. See the
[CLDR 49 cardinal plural rules chart](https://unicode.org/cldr/charts/49/supplemental/language_plural_rules.html).

At runtime, cardinal and ordinal programs are stored in separate rule sets. Each is evaluated in
CLDR precedence order: `zero`, `one`, `two`, `few`, `many`; if none matches, the category is `other`.
Rule evaluation uses the typed selector's supported numeric domain: integers for counts and ranks,
and exact decimal operands for `plural_decimal`. Branch source order does not affect selection; the
AST keeps source order for diagnostics, and lowering normalizes branch tables to canonical category
order. `#` renders the active selector without grouping or locale-specific formatting: counts and
ranks render as base-10 integers, while `plural_decimal` preserves its visible fractional scale.

`plural_count` supplies the integer domain, with `v`, `w`, `f`, and `t` equal to zero.
`plural_decimal` preserves a non-negative coefficient and visible scale from 0 through 38, so CLDR
rules can distinguish values such as `1`, `1.0`, and `1.00`. Cardinal rules evaluate exact `n`, `i`,
`v`, `w`, `f`, and `t` operands; `c` and `e` are zero because compact values are not supported.
Ordinal rules remain integer-only. Branch completeness still follows every category assigned by the
locale's CLDR rule set, including categories unreachable for an integer input.

The pinned lookup inputs are [`plurals.xml`](../../crates/u18n-compiler/data/cldr-49/plurals.xml)
for cardinal rules, [`ordinals.xml`](../../crates/u18n-compiler/data/cldr-49/ordinals.xml) for
ordinal rules,
[`supplementalMetadata.xml`](../../crates/u18n-compiler/data/cldr-49/supplementalMetadata.xml) for
aliases, and [`supplementalData.xml`](../../crates/u18n-compiler/data/cldr-49/supplementalData.xml)
for parent-locale data. The `plurals` component's map is used only for plural-rule lookup; the
unqualified main-component map is embedded in generated `LocaleTag::fallback` code for runtime
catalog selection. The compiler verifies the cardinal-rule, ordinal-rule, alias, and supplemental
files' SHA-256 digests, records them in generated metadata, and includes them in the catalog
fingerprint. The output-format version is `2`. The
[CLDR 49 cardinal plural rules chart](https://unicode.org/cldr/charts/49/supplemental/language_plural_rules.html)
provides human-readable cardinal examples. The vendored
[IANA Language Subtag Registry snapshot](../../crates/u18n-compiler/data/iana-language-subtag-registry.txt)
used for BCP 47 validation is identified in generated metadata by its SHA-256 digest and is included
in the catalog fingerprint; its `File-Date`, digest, and source are recorded in
[`iana-language-subtag-registry.md`](../references/iana-language-subtag-registry.md). Version 1's
extension policy is recorded as `locale-extension-policy=reject-all` in generated metadata and the
fingerprint, so no separately changing extension registry or specification is an input.

## Typed formatting

Format operations are typed by the schema and carry closed options in the AST. The `decimal`
operation requires `grouping`, `precision`, and `rounding`; precision is either `input` or a fixed
count from 0 to 9. The `currency` operation requires `precision` (`currency` or 0 to 9) and
`rounding`. Date and time operations require a `style` from `short`, `medium`, `long`, or `full`.
Instant formatting additionally requires a quoted time-zone identifier. These options are requests
to the adapter; the runtime does not perform locale formatting itself.

Both generated paths store the exact authored source locale tag for formatting. A caller-provided
`Formatter` receives that selected locale tag, the typed value, the operation options, and a
`core::fmt::Write` sink. The same numeric value can therefore be rendered with different locale
conventions by choosing different catalog handles or formatter implementations. A formatting
operation returns `FormatterFailed` when the adapter fails and `WriteFailed` when the output sink
rejects a write. A failed sink write can leave a partial prefix. `Catalog::render_to` streams
messages without allocating an output `String`; `Catalog::render` remains the allocating convenience
method for dynamic IDs. Keyed `MessageData::render_to` provides the same streaming behavior for one
message shard.

## Diagnostics

Syntax and semantic diagnostics identify the narrowest useful source span: the invalid escape,
repeated branch case, offending selector name, or missing-token position. Parsers recover at
top-level declaration boundaries and return all independent syntax diagnostics; a file with any
syntax error has no usable AST. Semantic validation collects independent issues across declarations
and locale files. Cross-AST errors use the locale span as primary and add the corresponding schema
key or parameter declaration as a related location. Branch-completeness errors point to the plural
expression and name the missing or unsupported category. Fallback errors point to the authored
target or chain edge. Linked source spans are discarded before generated runtime tables are emitted.
