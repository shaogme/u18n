# Result-returning `t!`

This example compiles its catalog with `CompilerOptions::default()` and leaves
`render_error_fallback_message` unset. As a result, both zero-argument and parameterized `t!`
calls return `Result` values that can be propagated with `?`.

Run it with:

```sh
cargo run -p u18n-result-api-example
```

The explicit type annotations in `src/main.rs` demonstrate the success types: `&'static str` for a
zero-argument message and `String` for a parameterized message. The compiler errors remain available
as `u18n::RenderError` through the returned `Result`.
