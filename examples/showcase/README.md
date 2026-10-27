# u18n showcase

This crate demonstrates the schema-to-runtime flow: schema parameters produce
typed message macros, locale source compiles to static catalogs, and the
runtime renders exact and decimal-aware cardinal and integer ordinal plural
branches and string selectors. `LocaleTag` performs validated,
case-insensitive exact-tag lookup, and the
showcase build uses the optional process-wide selector with English as its
final fallback. Its tests exercise key-based `t!` calls as well as direct
per-message macros. Typed decimal, currency, date, time, and instant operations
use a caller-provided formatter. The Spanish catalog inherits messages through
an explicit fallback to English, resolved when the catalog is compiled;
formatter requests still receive the selected catalog tag (`es-419`).
The five-parameter checkout summary demonstrates the named macro form, while
the other messages demonstrate positional and shorthand named values.

The package `build.rs` compiles the schema and locale sources in memory and
writes generated Rust to Cargo's `OUT_DIR`. Editing any of those files reruns
the build script, and `src/lib.rs` includes the generated module from there.
The compiler is a build dependency; `u18n` remains the normal dependency used
by the generated catalog and its hidden host-side proc-macro re-export.

Export translator context for the Spanish catalog and its English fallback:

```sh
cargo run -p u18n-cli -- extract \
  --schema examples/showcase/schema.u18n \
  --locale examples/showcase/locales/en.u18n \
  --locale examples/showcase/locales/es-419.u18n \
  --source-locale es-419 \
  --output messages.json
```

Report authored, inherited, and missing messages across the locale set:

```sh
cargo run -p u18n-cli -- coverage \
  --schema examples/showcase/schema.u18n \
  --locale examples/showcase/locales/en.u18n \
  --locale examples/showcase/locales/es-419.u18n
```

Run the demo with `cargo run -p u18n-showcase`. Extraction and coverage remain
available through `u18n-cli` for translator context and fallback reporting.
