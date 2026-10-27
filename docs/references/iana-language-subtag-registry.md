# IANA Language Subtag Registry snapshot

The authoritative, unmodified registry snapshot is bundled at [`crates/u18n-compiler/data/iana-language-subtag-registry.txt`](../../crates/u18n-compiler/data/iana-language-subtag-registry.txt). It was downloaded from the [official IANA registry URL](https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry) on 2026-10-24.

| Field | Value |
| --- | --- |
| Registry `File-Date` | `2026-09-17` |
| SHA-256 of exact file bytes | `755fad43283be7b41ebe3c89ad054b6eaf928f404f9c0edb74799e0eab74beb1` |

`u18n-compiler` uses these bytes for BCP 47 registered-subtag validation. Builds must use the vendored bytes and must not fetch the live registry. Generated metadata and the catalog fingerprint identify the snapshot by the SHA-256 value above. The digest is also stored in [`iana-language-subtag-registry.sha256`](iana-language-subtag-registry.sha256). From `docs/references`, verify the bundled file with:

```sh
sha256sum --check iana-language-subtag-registry.sha256
```
