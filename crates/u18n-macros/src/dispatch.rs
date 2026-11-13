use std::iter;

use super::invocation::InvocationFields;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    Error as SynError, Ident, LitBool, LitStr, Path, Result, Token, bracketed,
    parse::{Parse, ParseStream},
    parse2,
};

#[cfg(test)]
use syn::parse_str;

struct DispatchInvocation {
    atomic_locale: bool,
    anchor: Path,
    render_error_fallback_message: Option<LitStr>,
    fields: InvocationFields,
}

struct SelfKey;

pub(super) struct TranslationDispatcher;

impl SelfKey {
    fn macro_name(key: &LitStr) -> Result<Ident> {
        let key_text = key.value();
        let mut encoded = String::new();
        let segments = key_text.split('.').collect::<Vec<_>>();
        for (index, segment) in segments.iter().enumerate() {
            let mut bytes = segment.bytes();
            let Some(first) = bytes.next() else {
                return Err(SynError::new(
                    key.span(),
                    "message key segments must not be empty",
                ));
            };
            if !first.is_ascii_alphabetic() && first != b'_' {
                return Err(SynError::new(
                    key.span(),
                    "message key segments must begin with an ASCII letter or underscore",
                ));
            }
            for byte in iter::once(first).chain(bytes) {
                match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => {
                        encoded.push(char::from(byte));
                    }
                    b'_' => encoded.push_str("_u"),
                    b'-' => encoded.push_str("_h"),
                    _ => {
                        return Err(SynError::new(
                            key.span(),
                            "message keys allow only ASCII letters, digits, `_`, `-`, and `.`",
                        ));
                    }
                }
            }
            if index + 1 < segments.len() {
                encoded.push_str("_s");
            }
        }
        Ok(format_ident!("m_{encoded}", span = key.span()))
    }
}

impl Parse for DispatchInvocation {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let atomic_locale = input.parse::<LitBool>()?.value;
        input.parse::<Token![,]>()?;
        let anchor_content;
        bracketed!(anchor_content in input);
        let anchor = anchor_content.parse::<Path>()?;
        if !anchor_content.is_empty() {
            return Err(anchor_content.error("unexpected tokens in generated crate anchor"));
        }
        let render_error_fallback_message = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            Some(input.parse::<LitStr>()?)
        } else {
            None
        };
        input.parse::<Token![;]>()?;
        let fields = InvocationFields::parse_tokens(input.parse::<TokenStream2>()?)?;
        Ok(Self {
            atomic_locale,
            anchor,
            render_error_fallback_message,
            fields,
        })
    }
}

impl TranslationDispatcher {
    pub(super) fn expand(input: TokenStream2) -> Result<TokenStream2> {
        let invocation = parse2::<DispatchInvocation>(input)?;
        let key = invocation.fields.name().ok_or_else(|| {
            SynError::new(
                Span::call_site(),
                "structured `t!` calls require exactly one `name: \"key\"` field",
            )
        })?;
        let message_name = SelfKey::macro_name(key)?;
        if !invocation.atomic_locale && invocation.fields.locale().is_none() {
            return Err(SynError::new(
                key.span(),
                "non-atomic translation calls require a `locale: expression` field",
            ));
        }
        let anchor = invocation.anchor;
        let locale = match invocation.fields.locale() {
            Some(locale) => quote!(#locale),
            None => quote!(#anchor::current_locale()),
        };
        let formatter = invocation
            .fields
            .formatter()
            .map_or_else(TokenStream2::new, |value| quote!(#value));
        let has_sink = invocation.fields.sink().is_some();
        let sink = invocation
            .fields
            .sink()
            .map_or_else(TokenStream2::new, |value| quote!(#value));
        let values = invocation.fields.arguments().cloned().unwrap_or_default();
        let allow_missing_formatter =
            invocation.render_error_fallback_message.is_some() && !has_sink;
        let dispatch = quote! {
            #anchor::#message_name! {
                @__u18n_dispatch [#allow_missing_formatter] [#locale] [#formatter] [#sink] { #values }
            }
        };
        Ok(match invocation.render_error_fallback_message {
            Some(fallback) if !has_sink => {
                quote!((#dispatch).unwrap_or_else(|_| #fallback.into()))
            }
            None => dispatch,
            Some(_) => dispatch,
        })
    }
}

#[test]
fn encodes_raw_and_escaped_message_keys_like_the_compiler() {
    let raw = parse_str::<LitStr>(r####"r#"cart.decimal_count"#"####);
    let escaped = parse_str::<LitStr>(r#""cart\u{2e}decimal_count""#);

    assert!(raw.is_ok());
    assert!(escaped.is_ok());
    let Some(raw) = raw.ok() else {
        return;
    };
    let Some(escaped) = escaped.ok() else {
        return;
    };
    assert!(SelfKey::macro_name(&raw).is_ok_and(|name| name == "m_cart_sdecimal_ucount"));
    assert!(SelfKey::macro_name(&escaped).is_ok_and(|name| name == "m_cart_sdecimal_ucount"));

    let hyphenated = parse_str::<LitStr>(r#""checkout.order-summary""#);
    assert!(hyphenated.is_ok_and(|key| {
        SelfKey::macro_name(&key).is_ok_and(|name| name == "m_checkout_sorder_hsummary")
    }));
}

#[test]
fn rejects_invalid_message_key_shapes() {
    let invalid_keys = [
        "",
        ".title",
        "app.",
        "app..title",
        "9app.title",
        "app.bad$key",
    ];
    for source in invalid_keys {
        let key = LitStr::new(source, Span::call_site());
        assert!(SelfKey::macro_name(&key).is_err());
    }
}

#[test]
fn requires_a_string_literal_key() {
    let invalid = TranslationDispatcher::expand(quote!(true, [crate_anchor]; name: 42));
    assert!(invalid.is_err());
}

#[test]
fn optional_render_error_fallback_wraps_only_the_keyed_dispatch() {
    let fallback = TranslationDispatcher::expand(quote!(
        false,
        [crate_anchor],
        "could not render";
        locale: &CATALOG,
        name: "cart.title"
    ));
    let unchanged = TranslationDispatcher::expand(quote!(
        false,
        [crate_anchor];
        locale: &CATALOG,
        name: "cart.title"
    ));

    assert!(fallback.is_ok_and(|tokens| {
        tokens.to_string().contains("unwrap_or_else")
            && tokens.to_string().contains("could not render")
            && tokens.to_string().contains("m_cart_stitle")
            && tokens.to_string().contains("true")
    }));
    assert!(unchanged.is_ok_and(|tokens| !tokens.to_string().contains("unwrap_or_else")));
}

#[test]
fn parses_reordered_fields_and_preserves_nested_arguments() {
    let fields = InvocationFields::parse_tokens(quote!(
        args: { value: value, },
        sink: &mut output,
        formatter: &formatter,
        locale: &CATALOG,
        name: r#"cart.title"#,
    ));
    let named = InvocationFields::parse_tokens(quote!(args: { value: value, }));
    let empty_named = InvocationFields::parse_tokens(quote!(args: {}));

    assert!(fields.is_ok());
    assert!(named.is_ok());
    assert!(empty_named.is_ok());
    let Some(fields) = fields.ok() else {
        return;
    };
    let Some(named) = named.ok() else {
        return;
    };
    let Some(empty_named) = empty_named.ok() else {
        return;
    };
    assert!(fields.locale().is_some());
    assert!(fields.formatter().is_some());
    assert!(fields.sink().is_some());
    assert!(
        fields
            .name()
            .is_some_and(|name| name.value() == "cart.title")
    );
    assert_eq!(
        fields.arguments().map(TokenStream2::to_string).as_deref(),
        Some("value : value ,")
    );
    assert_eq!(
        named.arguments().map(TokenStream2::to_string).as_deref(),
        Some("value : value ,")
    );
    assert_eq!(
        empty_named
            .arguments()
            .map(TokenStream2::to_string)
            .as_deref(),
        Some("")
    );
}

#[test]
fn diagnoses_duplicate_fields_and_legacy_syntax() {
    let invalid_headers: [TokenStream2; 5] = [
        quote!(locale: &CATALOG, locale: other),
        quote!(sink: &mut output, sink: &mut other),
        quote!(formatter:),
        quote!(locale = &CATALOG),
        quote!(locale: &CATALOG formatter: &formatter),
    ];
    for input in invalid_headers {
        assert!(InvocationFields::parse_tokens(input).is_err());
    }
}

#[test]
fn enforces_mode_locale_rules_and_emits_anchored_dispatch() {
    let atomic = TranslationDispatcher::expand(quote!(
        true,
        [crate_anchor];
        name: r#"cart.decimal_count"#,
        args: { value, }
    ));
    let explicit = TranslationDispatcher::expand(quote!(
        false,
        [crate_anchor];
        locale: &CATALOG,
        name: "cart.decimal_count",
        args: { value }
    ));
    let missing_locale = TranslationDispatcher::expand(quote!(
        false,
        [crate_anchor];
        name: "cart.decimal_count",
        args: { value }
    ));
    let sink_fallback = TranslationDispatcher::expand(quote!(
        false,
        [crate_anchor],
        "could not render";
        locale: &CATALOG,
        name: "cart.decimal_count",
        sink: &mut output,
        args: { value }
    ));

    assert!(atomic.is_ok());
    assert!(explicit.is_ok());
    assert!(missing_locale.is_err());
    assert!(sink_fallback.is_ok_and(|tokens| {
        let rendered = tokens.to_string();
        rendered.contains("@ __u18n_dispatch")
            && rendered.contains("output")
            && !rendered.contains("unwrap_or_else")
    }));
    assert!(atomic.is_ok_and(|tokens| {
        tokens
            .to_string()
            .contains("crate_anchor :: current_locale")
            && tokens.to_string().contains("m_cart_sdecimal_ucount")
    }));
    assert!(explicit.is_ok_and(|tokens| {
        tokens.to_string().contains("m_cart_sdecimal_ucount")
            && tokens.to_string().contains("CATALOG")
    }));
}
