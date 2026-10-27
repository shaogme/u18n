# CLDR 49 reference files

The compiler's authoritative pinned inputs are bundled under [`crates/u18n-compiler/data/cldr-49`](../../../crates/u18n-compiler/data/cldr-49). This document records their provenance and license for the locale and plural-rule specifications.

## Reference pages

| Local file | Official source | Use |
| --- | --- | --- |
| `cldr-49-release-notes.html` | <https://cldr.unicode.org/downloads/cldr-49> | Release date and changes to CLDR 49. |
| `cldr-49-language-plural-rules.html` | <https://unicode.org/cldr/charts/49/supplemental/language_plural_rules.html> | Cardinal category sets and sample rules by locale. |
| `uts35-v79-part1-core.html` | <https://www.unicode.org/reports/tr35/tr35-79/tr35.html> | Locale identifiers, aliases, and inheritance. |
| `uts35-v79-part3-numbers.html` | <https://www.unicode.org/reports/tr35/tr35-79/tr35-numbers.html> | Plural operands, categories, and evaluation order. |
| `uts35-v79-part6-supplemental-draft.html` | <https://www.unicode.org/reports/tr35/tr35-79/tr35-info.html> | Supplemental data structure; this part is marked draft by Unicode. |
| `unicode-terms-of-use.html` | <https://www.unicode.org/copyright.html> | Unicode terms of use. |
| `unicode-license-v3.txt` | <https://www.unicode.org/license.txt> | Unicode License v3, bundled with the compiler data. |

The downloaded HTML snapshots are ignored by Git because the CLDR specification's copyright notice limits copies to personal or internal business use and prohibits public distribution without Unicode's permission. Use the official URLs above for published material.

## Supplemental data

`plurals.xml`, `ordinals.xml`, `supplementalData.xml`, and `supplementalMetadata.xml` were extracted from the official [`cldr-common-49.zip`](https://www.unicode.org/Public/cldr/49/cldr-common-49.zip). They provide cardinal and ordinal plural rules, parent-locale data for plural lookup and generated runtime catalog selection, and locale alias metadata. The compiler keeps the main-component and plural-component parent mappings separate. The full 35.64 MB archive was not retained. The cardinal rules, ordinal rules, alias metadata, and supplemental data files have SHA-256 digests `69a30c57a621de9c52ec6ca95fe7bf9f06df6f8c13ca71a72e447b532c92cff7`, `6c62fa2250f4f0150a3c7e24fc93c20ae6722f65bd26b49aaad799b40f8014b3`, `b06614fc2c9aab3b9a9f7e8bf9328f4f7a95d89b2d2dbb300d0caff0f885c164`, and `745150ec1408d50e1a272b65654a5dd645f3e60f5cc2898b576f725b28538d1a`, respectively. `u18n-compiler` checks all four before use and records the digests in generated metadata and the catalog fingerprint.

The archive's SHA-512 was checked against `CLDR-49-SHASUM512.txt`, the official [Unicode checksum manifest](https://www.unicode.org/Public/cldr/49/hashes/SHASUM512.txt):

```text
c704a2baf9ce4e28f40e242f8b2fbd0123579bb2c56daaaf8a92d72abd02485a1f60095c3cc4fe89eaf2d5c9f5fd07954c2eaee88839f97e18b1ff37cd62e2d8  cldr-common-49.zip
```

The extracted data is covered by Unicode License v3. The bundled copies include [`LICENSE`](../../../crates/u18n-compiler/data/cldr-49/LICENSE) and [`unicode-license-v3.txt`](../../../crates/u18n-compiler/data/cldr-49/unicode-license-v3.txt).
