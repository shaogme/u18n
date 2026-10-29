# Compilation and rendering algorithms

This document describes how u18n turns the schema and locale ASTs into immutable catalog tables, and
how the runtime evaluates those tables. The source forms and AST nodes are defined in
[Schema language and Schema AST](schema-ast.md) and [Locale language and Locale AST](locale-ast.md).
The crate responsibilities are summarized in [Architecture](../architecture.md).

## Compile pipeline

`Compiler::compile` runs the following stages. The schema and every locale file are parsed
independently; a source file with syntax errors does not produce a usable AST. Compilation stops
before code generation if parsing, linking, or generation-option validation reports an error.

1. Parse the schema and locale files into source-spanned ASTs. Parsers report independent syntax
   errors by recovering at the next top-level declaration boundary.
2. Load the vendored IANA Language Subtag Registry and CLDR 49 files. Verify their expected SHA-256
   digests before parsing any of the data.
3. Validate schema declarations, locale tags, message patterns, and explicit fallback edges. Link
   each pattern reference to a schema parameter slot and a plural rule set.
4. Resolve each catalog's messages by following its explicit fallback chain. Each result is one
   complete pattern from one authoring locale.
5. Intern complete linked patterns by structural contents and plural-rule semantics, collect literal
   fragments, construct one shared UTF-8 string pool, and lower each unique pattern into indexed
   program and branch tables.
6. Emit one shared program store and a sparse locale view for each source locale, along with
   generated message macros, exact locale parsing, a CLDR-backed runtime catalog-selection helper,
   and the input fingerprint.

The compiler uses ordered maps and sorts message keys and source locale tags at defined boundaries.
Generated IDs and tables therefore do not depend on the order in which locale files were supplied.
Source paths are used in diagnostics, not as catalog identity.

## Parsing and diagnostics

Both parsers scan UTF-8 source while tracking byte offsets. Spans are half-open ranges into the
original file, and an escape's span covers its full source spelling. A source longer than `u32::MAX`
bytes is rejected before a span is built.

The schema parser recognizes the version header, message declarations, ordered parameter signatures,
and documentation comments. The locale parser recognizes one header and recursively builds pattern
nodes. Syntax whitespace and comments are skipped only where the respective grammar permits them;
pattern text is decoded into `Text` parts while escapes are interpreted. Physical and escaped
line breaks become source-spanned `LineBreak` parts; CRLF is one logical break. Selector syntax
whitespace may span lines, while line breaks inside branch patterns remain message content. Plural,
ordinal, and select nesting is limited to 32 along each branch path.

Parsing does not establish cross-file meaning. The compiler performs key uniqueness, schema binding,
type checking, tag registration, category completeness, and fallback checks after parsing.
Diagnostics retain a source label and span. Parse diagnostics use the supplied source path; some
schema-wide semantic diagnostics use the `<schema>` label and empty-locale diagnostics use
`<locales>`. Cross-AST diagnostics can attach the matching schema declaration as a related location.
No spans are copied into generated runtime tables.

## Locale identity and CLDR rule lookup

Catalog identity and plural-rule lookup are separate. The source tag, with recommended casing, is
retained exactly for metadata, explicit fallback matching, and the catalog's `locale_tag`. Generated
source-locale lookup compares tags without ASCII case and searches an index sorted by lowercase
ASCII tag. Source catalog tags are not limited by the parent-code layout. `fallback` first checks
this exact source-locale index, then checks the CLDR main-parent index using a semantic code for the
candidate's language, optional script, and optional region. This layout accepts 2- or 3-letter
languages, an optional 4-letter script, and an optional 2-letter or 3-digit region. Each numeric
`u64` row key stores the child code shifted left three bits and the parent discriminant in bits 0–2;
bits 56–63 are reserved zeroes. Rows sort by child code. The compiler rejects unrepresentable CLDR
entries and duplicate child codes. A candidate outside the bounded profile skips parent lookup.
If neither index matches, `fallback` tries structural truncation and then the default locale. It
never applies CLDR aliases or renames a catalog. Shared syntax validation permits ASCII case
variations and tags outside the semantic profile for runtime requests; the compiler checks IANA
registration and recommended source casing.

For each distinct locale input, the compiler validates its complete tag against BCP 47 syntax and
the pinned IANA registry. It checks registered language, extlang, script, region, and variant
subtags; extlang and variant prefixes; recommended casing; duplicate variants; and the version 2
rule rejecting extension sequences. Registered grandfathered tags and private-use sequences are
handled by their specific BCP 47 cases. Private-use subtags do not require registry membership.

The rule lookup derives a separate CLDR locale ID by removing the trailing private-use sequence and
applying the pinned language, script, territory, and variant aliases. Cardinal and ordinal data are
looked up independently. For one rule family, resolution is:

```text
candidate = canonical CLDR locale ID
visited = empty set
loop:
    if candidate was visited: fail
    add candidate to visited
    if this rule family assigns rules to candidate: return that rule set
    if the plurals component has an explicit parent for candidate:
        candidate = that parent
    else if candidate has more than one subtag:
        remove its rightmost subtag
    else:
        fail
```

Only the parent-locale mappings for the `plurals` component are used. The main component's parent
mappings and likely-subtag rules are not merged into this search. A locale explicitly assigned an
`other`-only rule set resolves successfully; an unknown tag cannot inherit generic `root` rules just
because lookup reached the end of its subtags.

The compiler requires cardinal rule resolution for every catalog. It also attempts ordinal
resolution for every catalog, but an unresolved ordinal family is an error when a pattern uses an
ordinal selector. A selector's rule set is attached when its pattern is linked. If a catalog
inherits that pattern, it retains the author locale's rule-set reference.

## Schema linking and pattern validation

Schema message keys are joined from their segments and indexed by key. Duplicate keys and duplicate
parameter names are diagnosed with both the repeated and original spans. Schema parameter order is
retained as the runtime slot order; message declaration order does not determine generated message
IDs.

For every locale declaration, the compiler checks duplicate keys and rejects keys absent from the
schema, even if a more specific catalog would shadow the declaration later. Each declaration is
linked against its schema signature before fallback resolution:

1. Build a name-to-binding table from the message's schema parameters. The binding contains the
   zero-based schema-order slot and declared type.
2. Walk the typed pattern recursively. Resolve each argument, selector, or format operation by
   parameter name and replace the name with its slot in the linked pattern.
3. Check the operation against the parameter type. Plain interpolation supports strings,
   caller-displayed numbers, counts, ranks, and plural decimals. Cardinal selectors accept counts or
   plural decimals; ordinal selectors accept ordinal counts; selects accept strings. Typed values
   require their matching format operation, with plural decimals also accepted by decimal
   formatting.
4. For each plural or ordinal selector, check that category branches contain exactly the category
   set assigned by the authored locale's corresponding CLDR rules. Reject repeated exact values
   (including different zero-padded spellings of the same value), repeated categories, unsupported
   categories, and missing categories. Exact branches are optional.
5. For each select, reject duplicate case names and require `other`. A pound token is valid only
   inside a plural or ordinal branch. It remains linked through nested select branches to the active
   plural value.

Missing locale parameters are allowed. Repeated references to a parameter are allowed. Format
options are parsed into closed typed values; they are passed to a caller-provided formatter rather
than interpreted as locale data by the compiler or runtime.

## Explicit fallback resolution

Fallback edges come only from locale headers. The compiler matches targets case-sensitively against
complete source locale tags; it does not infer parents from BCP 47 subtags. It reports targets
absent from the input set and detects directed cycles by following each locale's chain while
recording visited nodes. A cycle is reported once, even if several starting locales reach it.

For every catalog and every schema key, resolution starts at that catalog and follows its fallback
edge until the first locale that declares the key:

```text
resolve(catalog, key):
    current = catalog
    visited = empty set
    while current exists and current is not in visited:
        add current to visited
        if current has a linked declaration for key:
            return that whole linked message
        current = current's explicit fallback target
    return missing
```

The resolved entry records its author locale and retains its linked pattern as a whole. The compiler
does not splice branches, text, or parameters from multiple locales. A missing key is a compile
error for normal compilation and extraction; coverage mode preserves missing entries so it can
report them.

When a parse error prevents a locale from producing an AST but its header tag was successfully read,
that tag is tracked as unavailable. This lets the compiler avoid reporting a misleading missing-key
error for a chain that could not be resolved because the target file already has a syntax error; the
syntax diagnostic remains the cause to fix.

## Program interning, lowering, and table layout

Before emission, schema messages are sorted by dotted key and assigned numeric message IDs in that
order. The compiler builds a deterministic structural key for every resolved pattern. The key
includes text, schema argument slots, selector kinds and cases, complete branch bodies, format
options, and the canonical contents of referenced plural rule sets. It excludes message keys, locale
tags, source spans, and author tags. Plural and select branches are sorted using the same order as
lowering before they enter the key. Full keys are compared, so a hash collision cannot merge
different programs. The ordered set of keys assigns stable program IDs.

Equivalent plural rule sets are interned by canonical contents. Program plural nodes refer to those
shared rule sets, preserving the rule semantics of the pattern author even when a locale inherits
the program.

The dynamic shared store contains one argument-count signature per schema message ID, one
instruction range per unique program ID, the flattened instruction and selector tables, the
referenced plural rule sets, and the shared text pool. Each dynamic locale view contains its exact
source tag, a direct link to its declared fallback, and sorted `MessageBinding` entries for
messages authored in that locale. A binding maps the schema message ID to an interned program ID.
Inherited messages have no binding in the child view.

The compiler also lowers every schema key into an independent message shard. It collects that
key's resolved pattern for each locale, interns structurally identical patterns and canonical
plural rules only within the key, then emits one locale-to-program choice per locale in stable
locale-index order. A shard owns its argument count, programs, instructions, selector and branch
tables, format nodes, rule sets, and UTF-8 pool. It contains no references to another shard or to
the global dynamic store. Each locale choice records its exact tag so runtime can reject a handle
whose index and tag do not match.

The lowerer converts linked pattern parts as follows:

| Linked part | Lowered representation |
| --- | --- |
| Text | `Text` instruction with a byte range in the shared pool |
| LineBreak | `LineBreak` instruction that appends one LF |
| Argument | `Argument` instruction with a schema slot |
| Plural / ordinal | `Plural` instruction referring to a plural node and branch range |
| Select | `Select` instruction referring to a select node and branch range |
| Format | `Format` instruction referring to a typed format node |
| Pound | `Pound` instruction using the active plural value at render time |

Branch bodies become instruction ranges in their owning table set. Exact plural branches are
ordered numerically first; category branches follow the fixed order `zero`, `one`, `two`, `few`,
`many`, `other`. Select cases are ordered by their exact case string. Source order remains available
during diagnostics but does not affect branch choice. The dynamic store contains each rule set
referenced by any unique program once. Each keyed shard contains its own copies of the rule sets
and literal fragments used by that key.

### Shared and keyed string pools

The dynamic pool collects non-empty literal text and select case names from all resolved messages,
deduplicating fragments across keys. Each keyed pool collects fragments for one key across its
resolved locale patterns and removes duplicates within that key. Text repeated by different keys
stays independent in keyed shards. The default fragment limit is 256 and applies separately to
each pool.

Overlap optimization runs only when all of these limits are met: fragment count is at most the
configured limit, total fragment bytes are at most 1 MiB, and the estimated pairwise work
(`fragment_count² × total_fragment_bytes`) is at most 256 MiB. If any limit is exceeded, every
unique fragment is appended as its own component in sorted order.

When optimization runs, fragments already contained in a longer fragment are removed from the merge
set. The pool builder repeatedly merges the pair with the greatest suffix-to-prefix overlap, using
lexical tie-breaking to make equal choices deterministic. It removes components contained in a
merged component, sorts the remaining components, concatenates them, then finds each original
fragment's byte range within the pool. Overlap boundaries must be UTF-8 character boundaries. All
offsets, lengths, and table indices must fit in `u32`; otherwise code generation fails. Pool
statistics report the dynamic pool and aggregate keyed pools separately; keyed totals count
repeated fragments once for every shard that owns them.

## Generated Rust API and fingerprint

Generated catalogs and message shards are immutable statics constructed with the runtime's trusted
constructors. The generated `LocaleTag` is a public closed enum with one variant per input locale. Variant
identifiers concatenate normalized subtags in PascalCase (`en` is `En`, `es-419` is `Es419`, and
`zh-Hant-HK` is `ZhHantHk`); the available variants depend on the compilation's locale set. The enum
is distinct from `ParsedTag` and from string locale identities in runtime catalog and formatter
data. `from_str` parses a request and searches the source-locale index sorted by lowercase ASCII
tag, accepting any ASCII casing of an included tag; malformed or extension-rejected tags and valid
tags without an exact locale produce distinct errors. `as_str` returns the authored source
spelling, `as_locale` returns the lightweight locale handle, and `as_catalog` returns the dynamic
catalog. Each locale handle contains only its stable index and exact tag. Each CLDR main-parent index row stores a single numeric `u64`
key with the semantic child code shifted left three bits and a fixed private generated
`ParentLocale` discriminant in the low three bits. The enum provides the canonical BCP 47 spelling.
`fallback` returns an error for malformed input, then checks the exact source-locale index and CLDR
49 main-parent mappings before structurally truncating the request. It removes a private-use tail as
a unit, treats grandfathered tags as atomic candidates, and returns the lexicographically first
source locale for valid requests with no included candidate. It does not apply aliases or change
message-pattern resolution.
Generated message macros use the injective key encoding defined in the schema specification. Each
macro references only its matching shard and passes a lightweight locale handle, never a `Catalog`
or global numeric message ID. The proc-macro helper validates positional arity or named fields,
evaluates named expressions once in written order, then constructs the runtime argument array in
schema order. The locale expression is evaluated once; a formatter expression, when required, is
evaluated next.

The generated `t!` macro dispatches a literal key to its message macro. Non-atomic builds require a
`locale = expression` header. Atomic builds use the current lightweight locale by default and may
still receive an explicit `locale` header. Atomic generation requires an explicit final fallback
locale; the generated selector uses pointer-width atomics. `current_catalog()` and
`FINAL_FALLBACK_CATALOG` remain dynamic-ID conveniences.

The input SHA-256 is length-delimited and includes the raw schema source, raw locale sources ordered
by source tag, output-format version (`2`), CLDR version, IANA registry digest, cardinal-rule
digest, ordinal-rule digest, alias digest, supplemental-data digest, extension policy,
atomic-selection setting, final fallback tag, and string-pool limit. Cardinal-rule and alias
digests identify inputs used for plural resolution; the supplemental digest identifies the parent
mappings embedded in `LocaleTag::fallback`. Locale path names and the original order of locale
inputs are not included. The generated header records each locale-data digest, the resulting
fingerprint, and generation metadata.

## Runtime rendering

`Catalog::render` and the streaming variants take a numeric message ID and arguments in schema
order. Dynamic rendering loads the shared message signature, searches the selected locale's sorted
bindings, and follows direct fallback links until it finds a program. `MessageData::render` and its
streaming and formatter variants take a lightweight `Locale`; keyed rendering selects the
pre-resolved program from its shard by locale index and verifies that the exact tag agrees. Both
routes check the argument count before walking the program's instruction range. The compiler
validates generated tables, and `Catalog::try_new` checks caller-built dynamic data once at
construction:

* Text appends a validated slice from the owning table set's pool.
* LineBreak appends one LF through the caller's output sink.
* Argument writes a string, caller `Display` value, count/rank digits, or the ungrouped plural
  decimal. Typed format-only values cannot be written as plain arguments.
* Select compares the string argument exactly against case names and renders the matching branch, or
  `other` if none matches.
* Format builds a typed request from the argument and stored options, then calls the supplied
  formatter with the selected locale's source tag. A formatter is required only for a
  message whose selected pattern reaches a format instruction.
* Plural evaluates exact branches first when the input is a whole integer representable as `u64`;
  otherwise, or when no exact branch matches, it evaluates the compiled CLDR rules and renders the
  resulting category branch.
* Pound writes the active plural value. A nested plural replaces that active value in its branch; a
  nested select preserves it. Returning from a branch restores the caller's value.

The locale parser rejects selector nesting deeper than 32. The runtime catalog validator and shared
renderer independently enforce the same maximum. `Catalog::try_new` checks caller-constructed
shared table ranges and references, binding order and uniqueness, message resolution through each
fallback chain, fallback acyclicity, branch completeness, rule tables, argument slots, formatter
options, and selector depth. This includes validating each rule set's categories and predicates
once, then checking that every plural node has the corresponding category branches. The evaluator
uses the validated category-table invariant during rendering without repeating that check. Keyed
rendering checks invalid locale indices, locale-tag mismatches, program references, argument
counts, table ranges, and selector depth. Compiler-generated tables use trusted constructors after
compile-time validation, and other trusted constructors require callers to preserve the same plural
rule and branch invariants. Render and formatter failures are returned as `RenderError`; streaming
can leave a partial output prefix if a write fails.

The linker can discard unused message shards when the target uses data-section garbage collection
and does not otherwise root their sections. Run `scripts/check-keyed-size.sh` to build release
fixtures that use one key, several keys, all keys, and dynamic IDs. The script checks unique text
payloads in linked artifacts and reports read-only section sizes.

### Plural rule evaluation

The compiler parses each pinned CLDR condition into a disjunction of conjunctions of range
predicates. The runtime stores those predicates as static rule tables; it does not parse CLDR XML or
rule text.

Checked dynamic catalogs validate rule categories and predicates during `Catalog::try_new`, and
validate category-branch completeness for every plural node. Plural selection relies on that
invariant and does not rescan the rule table for duplicate categories or an explicit `other` rule.
Compiler-generated tables have already passed compiler validation; callers of trusted constructors
must provide valid tables themselves.

For an input decimal, the runtime retains its coefficient and visible scale. It derives CLDR
operands as follows: `n` is the exact decimal; `i` is the integer part; `v` is the visible
fractional digit count; `w` is that count with trailing zeroes removed; `f` is the visible
fractional digits as an integer; `t` is `f` with trailing zeroes removed; and `c` and `e` are zero
because compact notation is unsupported. Integer counts and ranks have scale zero. Rules evaluate
categories in the order `zero`, `one`, `two`, `few`, `many`; the first matching category wins,
otherwise the result is `other`. Conjunction predicates all must match, while any alternative
conjunction may match. `in` predicates require an integral operand; `within` predicates also accept
fractional operands. Modulo, negation, and inclusive ranges are applied as encoded in the compiled
predicate.

For cardinal `plural_decimal`, pound writes the ungrouped decimal with its authored scale, including
trailing zeroes. For `plural_count` and `ordinal_count`, pound writes unsigned base-10 integer
digits. Exact matching treats a decimal with zero fractional value as its equivalent integer when it
fits in `u64`.

## Translator tooling

Extraction and coverage reuse parsing, locale validation, schema linking, and fallback resolution.
Extraction requires an exact source locale tag and a resolved message for every schema key. It uses
the pattern span and author locale to copy the original quoted source pattern into deterministic
JSON, along with schema documentation and parameter notes. Coverage counts each resolved message as
authored or inherited by comparing its author locale with the selected locale, and reports
unresolved keys as missing.

## Implementation map

| Algorithm | Main implementation |
| --- | --- |
| Source parsing | [`u18n-schema`](../../crates/u18n-schema/src/parser.rs), [`u18n-locale`](../../crates/u18n-locale/src/parser.rs), [`pattern parser`](../../crates/u18n-locale/src/parser/pattern.rs) |
| Validation and linking | [`compiler API`](../../crates/u18n-compiler/src/api.rs), [`semantic validator`](../../crates/u18n-compiler/src/semantic.rs), [`schema linker`](../../crates/u18n-compiler/src/semantic/schema.rs) |
| Validation and linking | [`pattern linker`](../../crates/u18n-compiler/src/semantic/pattern.rs), [`fallback linker`](../../crates/u18n-compiler/src/semantic/fallback.rs) |
| IANA and CLDR lookup | [`CLDR data loader`](../../crates/u18n-compiler/src/cldr.rs), [`tag validation and aliases`](../../crates/u18n-compiler/src/cldr/tag.rs), [`rule parser`](../../crates/u18n-compiler/src/cldr/rules.rs) |
| Lowering and emission | [`code generator`](../../crates/u18n-compiler/src/codegen.rs), [`table lowering`](../../crates/u18n-compiler/src/codegen/lower.rs), [`string pool`](../../crates/u18n-compiler/src/codegen/pool.rs), [`Rust emitter`](../../crates/u18n-compiler/src/codegen/emit.rs) |
| Runtime validation and evaluation | [`catalog tables`](../../crates/u18n/src/catalog.rs), [`renderer`](../../crates/u18n/src/format.rs), [`plural evaluator`](../../crates/u18n/src/plural.rs), [`catalog validator`](../../crates/u18n/src/validation.rs) |
| Extraction and fingerprint | [`extractor`](../../crates/u18n-compiler/src/extract.rs), [`fingerprint`](../../crates/u18n-compiler/src/fingerprint.rs) |
