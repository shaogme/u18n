# Changelog

## [0.3.1]

### Added

- Added opt-in `Cow<'static, str>` results for generated keyed macros with
  `CompilerOptions::with_cow_macro_output(true)` or `u18n-cli compile --cow-macro-output`. The
  public `RenderedMessage` alias represents the result: zero-argument messages and fixed `t!`
  fallbacks are borrowed, while parameterized messages are owned. The default macro return types
  remain unchanged.

### Changed

- Generated metadata and the input fingerprint now record the Cow macro-output setting. The
  fingerprint domain and generated output-format version remain `v3` and `3`.

## [0.3.0]

### Added

- Added optional runtime locale selection fallback. Enable `LocaleTag::fallback` and its CLDR 49
  main-parent lookup data with `CompilerOptions::with_locale_tag_fallback(true)` or
  `u18n-cli compile --locale-tag-fallback`. Exact, case-insensitive `LocaleTag` parsing remains
  available with the default options. This selects an included catalog; message inheritance still
  follows only the fallback edges declared in locale files.
- Added `LocaleProvider`, allowing generated message macros and `MessageData` rendering to accept
  `Locale`, generated `LocaleTag`, or application-defined locale types.
- Added an optional fixed render-error message for generated `t!` calls, configurable with
  `CompilerOptions::with_render_error_fallback_message` or
  `u18n-cli compile --render-error-fallback-message TEXT`. When configured, `t!` returns the
  rendered message on success and the fixed text on error. This option does not change the
  `Result` behavior of generated `m_*` macros, dynamic catalog methods, or streaming methods.
- Added a [result-returning `t!` example](examples/result-api/README.md) for builds that leave the
  render-error fallback unset.

### Changed

- Zero-argument generated `m_*` macros now return borrowed `&'static str` text without allocating;
  parameterized macros continue to return owned `String` text. Both forms return
  `Result<_, RenderError>`.
- Dynamic `Catalog::render` and `MessageData::render` methods now return
  `Result<RenderedMessage, RenderError>`, where `RenderedMessage` is borrowed for zero-argument
  messages and owned for parameterized messages. Call `.into_owned()` when the result must always
  be a `String`.
- Generated `t!` calls return the same `Result` as their matching `m_*` macro by default. With a
  configured render-error fallback, they return `&'static str` for zero-argument messages or
  `String` for parameterized messages, using the configured text only when rendering fails.
- Generated catalog metadata and fingerprints now record the generation options and use output
  format version 3 and the `u18n-catalog-input-v3` hash domain.
- Updated all workspace crates to version `0.3.0`.
