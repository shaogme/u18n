use std::iter;

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Error as SynError, Expr, Ident, LitBool, LitStr, Path, Result, Token, bracketed,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    parse2,
};

#[cfg(test)]
use proc_macro2::Span;

#[cfg(test)]
use syn::parse_str;

struct DispatchArguments {
    locale: Option<Expr>,
    formatter: Option<Expr>,
    values: TokenStream2,
}

struct DispatchInvocation {
    key: LitStr,
    atomic_locale: bool,
    anchor: Path,
    arguments: DispatchArguments,
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
        let key = input.parse::<LitStr>()?;
        input.parse::<Token![,]>()?;
        let atomic_locale = input.parse::<LitBool>()?.value;
        input.parse::<Token![,]>()?;
        let anchor_content;
        bracketed!(anchor_content in input);
        let anchor = anchor_content.parse::<Path>()?;
        if !anchor_content.is_empty() {
            return Err(anchor_content.error("unexpected tokens in generated crate anchor"));
        }
        input.parse::<Token![;]>()?;
        let arguments = input.parse::<DispatchArguments>()?;
        Ok(Self {
            key,
            atomic_locale,
            anchor,
            arguments,
        })
    }
}

impl Parse for DispatchArguments {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut locale = None;
        let mut formatter = None;
        let mut formatter_seen = false;

        while Self::starts_control_header(input) {
            let name = input.parse::<Ident>()?;
            let name_text = name.unraw().to_string();
            input.parse::<Token![=]>()?;
            let expression = input.parse::<Expr>()?;
            match name_text.as_str() {
                "locale" => {
                    if formatter_seen {
                        return Err(SynError::new(
                            name.span(),
                            "the locale header must precede the formatter header",
                        ));
                    }
                    if locale.replace(expression).is_some() {
                        return Err(SynError::new(name.span(), "duplicate locale header"));
                    }
                }
                "formatter" => {
                    if formatter.replace(expression).is_some() {
                        return Err(SynError::new(name.span(), "duplicate formatter header"));
                    }
                    formatter_seen = true;
                }
                _ => {}
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
                if input.is_empty() {
                    break;
                }
            }
        }

        let values = input.parse::<TokenStream2>()?;
        Ok(Self {
            locale,
            formatter,
            values,
        })
    }
}

impl DispatchArguments {
    fn starts_control_header(input: ParseStream<'_>) -> bool {
        let fork = input.fork();
        let Ok(name) = fork.parse::<Ident>() else {
            return false;
        };
        if !fork.peek(Token![=]) {
            return false;
        }
        matches!(name.unraw().to_string().as_str(), "locale" | "formatter")
    }
}

impl TranslationDispatcher {
    pub(super) fn expand(input: TokenStream2) -> Result<TokenStream2> {
        let invocation = parse2::<DispatchInvocation>(input)?;
        let message_name = SelfKey::macro_name(&invocation.key)?;
        let arguments = invocation.arguments;
        if !invocation.atomic_locale && arguments.locale.is_none() {
            return Err(SynError::new(
                invocation.key.span(),
                "non-atomic translation calls require a leading `locale = expression` header",
            ));
        }
        let anchor = invocation.anchor;
        let locale = match arguments.locale {
            Some(locale) => quote!(#locale),
            None => quote!(#anchor::current_locale()),
        };
        let formatter = arguments
            .formatter
            .map_or_else(TokenStream2::new, |value| quote!(#value));
        let values = arguments.values;
        Ok(quote! {
            #anchor::#message_name! {
                @__u18n_dispatch [#locale] [#formatter] { #values }
            }
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
    assert!(parse2::<DispatchInvocation>(quote!(42, true, [crate_anchor];)).is_err());
}

#[test]
fn parses_control_headers_and_preserves_argument_forms() {
    let positional =
        parse2::<DispatchArguments>(quote!(locale = &CATALOG, formatter = &formatter, value,));
    let named = parse2::<DispatchArguments>(quote!({ value: value, }));
    let empty_named = parse2::<DispatchArguments>(quote!({}));

    assert!(positional.is_ok());
    assert!(named.is_ok());
    assert!(empty_named.is_ok());
    let Some(positional) = positional.ok() else {
        return;
    };
    let Some(named) = named.ok() else {
        return;
    };
    let Some(empty_named) = empty_named.ok() else {
        return;
    };
    assert!(positional.locale.is_some());
    assert!(positional.formatter.is_some());
    assert_eq!(positional.values.to_string(), "value ,");
    assert_eq!(named.values.to_string(), "{ value : value , }");
    assert_eq!(empty_named.values.to_string(), "{ }");
}

#[test]
fn diagnoses_invalid_control_header_order_and_duplicates() {
    let invalid_headers: [TokenStream2; 4] = [
        quote!(locale = &CATALOG, locale = other),
        quote!(formatter = &formatter, locale = &CATALOG),
        quote!(formatter =),
        quote!(locale = &CATALOG formatter = &formatter),
    ];
    for input in invalid_headers {
        assert!(parse2::<DispatchArguments>(input).is_err());
    }
}

#[test]
fn enforces_mode_locale_rules_and_emits_anchored_dispatch() {
    let atomic = TranslationDispatcher::expand(quote!(
        r#"cart.decimal_count"#,
        true,
        [crate_anchor];
        { value, }
    ));
    let explicit = TranslationDispatcher::expand(quote!(
        "cart.decimal_count",
        false,
        [crate_anchor];
        locale = &CATALOG, value
    ));
    let missing_locale = TranslationDispatcher::expand(quote!(
        "cart.decimal_count",
        false,
        [crate_anchor];
        value
    ));

    assert!(atomic.is_ok());
    assert!(explicit.is_ok());
    assert!(missing_locale.is_err());
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
