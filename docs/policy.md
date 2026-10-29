# Locale and plural rule policy

This document records the project's policy for locale identity, message fallback, and plural
selection. The detailed source syntax and validation rules are defined in the [Locale AST
specification](specification/locale-ast.md) and the [Schema AST
specification](specification/schema-ast.md); the [architecture](architecture.md) describes how
those rules are compiled into runtime catalogs.

## Locale identity

### Source locale tags

Each locale source file declares one well-formed BCP 47 language tag using the recommended
casing. A registered grandfathered tag is accepted as a whole. Otherwise, language, extlang,
script, region, and variant subtags are checked against the vendored IANA registry snapshot.
Version 2 rejects BCP 47 extension sequences. Private-use subtags are allowed, but do not affect
plural-rule lookup.

The complete authored spelling is the locale's identity. It is retained in the generated locale
handle and dynamic catalog and returned by `LocaleTag::as_str()`; CLDR aliases do not rewrite it.
The compiler rejects duplicate source tags and tags that differ only by ASCII case.

### Runtime catalog selection

Generated `LocaleTag` is a closed enum with one variant per included catalog. Variant names
concatenate normalized subtags in PascalCase (`en` becomes `En`, `es-419` becomes `Es419`). The
source-locale index compares tags case-insensitively as strings, so source tags are not limited
to a fixed width. Each CLDR main-parent index row stores one numeric `u64` key: bits 3–55 encode
bounded language, script, and region components, while bits 0–2 encode the fixed `ParentLocale`
discriminant that selects the canonical parent tag. The child layout supports 2- or 3-letter
languages, an optional 4-letter script, and an optional 2-letter or 3-digit region. Other valid
BCP 47 forms do not produce a parent-index key.

`LocaleTag::from_str` accepts ASCII case variations of an included tag. A valid tag without an
exact included catalog returns an unsupported-tag error. Both generated lookup methods validate
BCP 47 structure and reject version 2 extension sequences. Runtime syntax validation does not
check ordinary subtag registration, which remains a compile-time IANA check. Well-formed tags
outside the semantic-code profile can still match an exact source tag. If they do not, fallback
continues with structural truncation; representable candidates can use the numeric parent index
at later steps.

`LocaleTag::as_locale()` returns a small handle containing only the stable locale index and exact
source tag. It has no reference to a catalog or message table, so generated key macros can use it
without retaining the dynamic store. `LocaleTag::as_catalog()` remains the explicit route to the
dynamic-ID catalog API.

`LocaleTag::fallback` returns an error for malformed requests. It first tries an exact
source-locale match, follows CLDR 49 main-component parents when the candidate has a semantic code,
then tries structural truncation, and selects the lexicographically first included locale if no
candidate matches. It does not apply aliases, add likely subtags, or change locale identity.
`ParsedTag` remains the shared borrowed syntax parser. Runtime catalog and formatter locale
identities remain strings.

### Message fallback

Message fallback is explicit. A `fallback` target must name another input locale using its exact
source tag, and the fallback graph must be acyclic. The compiler follows this graph to find the
nearest declaration of each schema key. It does not infer message fallback from BCP 47 subtags
or CLDR parent locales. `LocaleTag::fallback` only chooses which generated catalog to use; it
does not supply message patterns or participate in compilation-time message resolution.

The dynamic `Catalog` path keeps sparse authored bindings and follows fallback links at runtime.
Each keyed message shard stores the complete resolved pattern choice for every generated locale,
so keyed rendering does not traverse fallback links. Both paths use the whole pattern selected by
the compiler and preserve the plural rules of its author. Formatters receive the selected locale's
exact source tag in either path.

## Plural rule lookup

Plural rules come from the compiler's pinned CLDR 49 data. CLDR aliases and plural-component
parent data resolve rule sets; they never change catalog identity or message fallback targets.
For lookup, the compiler canonicalizes the non-private portion with CLDR aliases, then searches
the requested rule family using the `plurals` component's explicit parent mapping when present,
followed by rightmost-subtag truncation. It does not borrow parent mappings from CLDR's main
component. The main-component mappings are used only by the generated runtime catalog-selection
helper. If no assigned plural rule set is found, compilation fails instead of silently assigning
generic `other` rules.

Cardinal and ordinal rules are resolved independently. Cardinal rules are required for each
input locale. An ordinal selector additionally requires an ordinal rule set for the locale that
authored that pattern. In CLDR 49, the plural-specific parent-locale mapping is empty; for
example, `az-Arab` falls back to `az` for plural rules rather than using a main-component parent
mapping.

A locale's own message pattern is validated against that locale's rules. If a message is
inherited through explicit fallback, the pattern remains whole and retains the rule-set
reference of the locale that authored it. Fallback changes which pattern is used; it does not
change the selected locale's tag. A caller-provided formatter receives the selected locale's exact
source tag, whether rendering through a keyed shard or a dynamic catalog.

## Plural selector types and branches

Cardinal `plural` selectors accept `plural_count` and `plural_decimal`. Ordinal selectors accept
`ordinal_count` only. A general `number` is not a grammatical count or rank and cannot be used
as a plural selector.

Exact branches such as `=0` are optional, use unsigned `u64` integers, and are checked before
category rules. Equivalent exact integer spellings are duplicates. A `plural_decimal` whose
fractional value is zero can match the corresponding exact integer branch when that integer is
representable as `u64`.

Category branches must contain the complete category set assigned by the authored locale's CLDR
rule set, each exactly once, with no unsupported categories. `other` is always required. The
available names are `zero`, `one`, `two`, `few`, `many`, and `other`; each locale uses only its
assigned subset. These names are grammatical categories, not shorthand for particular numbers:
`one` is determined by the locale's rule, and `zero` does not create an automatic exact-zero
branch.

At runtime the category rules are tested in the order `zero`, `one`, `two`, `few`, `many`; if
none matches, the result is `other`. Source order of category branches does not control
selection. The compiler normalizes lowered branch tables, while exact branches continue to take
priority over every category.

## Numeric and `#` behavior

`plural_count` and `ordinal_count` are non-negative integers. Their CLDR visible-fraction
operands are zero. `plural_decimal` is an exact non-negative decimal with a scale from 0 through
38; the coefficient and scale are preserved, including trailing zeroes. Cardinal rules evaluate
CLDR operands `n`, `i`, `v`, `w`, `f`, and `t`; compact exponent operands `c` and `e` are zero
because compact-number values are not supported. Ordinal rules remain integer-only.

Inside a plural or ordinal branch, `#` refers to the innermost active plural selector, including
through nested `select` branches. Counts and ranks render as ungrouped base-10 integers. A
cardinal `plural_decimal` renders as an ungrouped decimal preserving its visible scale, so
values such as `1`, `1.0`, and `1.00` remain distinguishable in both rules and output. Use `\#`
for a literal hash inside a branch. Locale-specific digit shaping, grouping, and other number
formatting belong to the caller's formatter, not plural evaluation.

Plain interpolation of `plural_decimal` also preserves its visible scale. The schema type
`number` uses the caller's `Display` implementation and is independent of CLDR plural rules.
Typed decimal, currency, date, time, and instant formatting is delegated to a caller-provided
formatter; that adapter receives the selected locale tag and the operation's explicit options.

## Pinned data

The IANA registry and CLDR files are versioned compiler inputs whose expected digests are
checked at load time. Generated metadata and the catalog fingerprint record the CLDR version,
IANA snapshot digest, cardinal-rule digest, ordinal-rule digest, alias digest,
supplemental-data digest, and extension policy. The fingerprint also includes schema and locale
source, generation settings, and output-format version (`2`).

The relevant pinned inputs are the [IANA Language Subtag Registry
snapshot](../crates/u18n-compiler/data/iana-language-subtag-registry.txt), CLDR 49 [cardinal
rules](../crates/u18n-compiler/data/cldr-49/plurals.xml), [ordinal
rules](../crates/u18n-compiler/data/cldr-49/ordinals.xml),
[aliases](../crates/u18n-compiler/data/cldr-49/supplementalMetadata.xml), and [supplemental
parent data](../crates/u18n-compiler/data/cldr-49/supplementalData.xml). Rule lookup and
evaluation are implemented in [`u18n-compiler`](../crates/u18n-compiler/src/cldr.rs) and
[`u18n`](../crates/u18n/src/plural.rs).
