use std::collections::BTreeSet;

use proc_macro2::{Delimiter, Group, Span, TokenStream as TokenStream2, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    Error as SynError, Expr, Ident, LitBool, LitInt, LitStr, Path, Result, Token, braced,
    bracketed,
    ext::IdentExt,
    parenthesized,
    parse::{Parse, ParseStream, Parser},
    parse_quote_spanned, parse2,
    punctuated::Punctuated,
    spanned::Spanned,
};

#[cfg(test)]
use syn::parse_quote;

#[derive(Clone, Copy)]
enum ArgumentKind {
    String,
    Number,
    PluralCount,
    PluralDecimal,
    OrdinalCount,
    Decimal,
    Money,
    Date,
    Time,
    Instant,
}

struct ParameterSpec {
    name: String,
    kind: ArgumentKind,
}

struct InvocationMetadata {
    needs_formatter: bool,
    cow_macro_output: bool,
    allow_missing_formatter: bool,
    runtime_anchor: Path,
    parameters: Vec<ParameterSpec>,
}

pub(super) struct InvocationFields {
    name: Option<LitStr>,
    locale: Option<Expr>,
    formatter: Option<Expr>,
    sink: Option<Expr>,
    arguments: Option<TokenStream2>,
}

struct NamedField {
    name: String,
    name_span: Span,
    value: Expr,
    shorthand: bool,
}

struct BoundArguments {
    evaluation_order: Vec<Expr>,
    schema_slots: Vec<usize>,
}

struct RenderInvocation {
    locale: Expr,
    message_data: Expr,
    formatter: Option<Expr>,
    sink: Option<Expr>,
    arguments: BoundArguments,
}

pub(super) struct InvocationExpansion;

impl Parse for InvocationMetadata {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let needs_formatter = input.parse::<LitBool>()?.value;
        input.parse::<Token![,]>()?;
        let cow_macro_output = input.parse::<LitBool>()?.value;
        input.parse::<Token![,]>()?;
        let allow_missing_formatter = input.parse::<LitBool>()?.value;
        input.parse::<Token![,]>()?;
        let runtime_content;
        bracketed!(runtime_content in input);
        let runtime_anchor = runtime_content.parse::<Path>()?;
        if !runtime_content.is_empty() {
            return Err(runtime_content.error("unexpected tokens in generated runtime anchor"));
        }
        input.parse::<Token![,]>()?;
        let parameters_content;
        bracketed!(parameters_content in input);
        let parameters = parameters_content
            .parse_terminated(ParameterSpec::parse, Token![,])?
            .into_iter()
            .collect::<Vec<_>>();
        Self::validate_parameter_names(&parameters)?;
        Ok(Self {
            needs_formatter,
            cow_macro_output,
            allow_missing_formatter,
            runtime_anchor,
            parameters,
        })
    }
}

impl Parse for InvocationFields {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut name = None;
        let mut locale = None;
        let mut formatter = None;
        let mut sink = None;
        let mut arguments = None;

        while !input.is_empty() {
            let field_name = if input.peek(LitStr) {
                return Err(input
                    .error("key-first `t!` calls are no longer supported; use `name: \"key\"`"));
            } else if input.peek(Ident::peek_any) {
                input.call(Ident::parse_any)?
            } else {
                return Err(input.error(
                    "expected a named field; use `locale: expression` and `args: { ... }`",
                ));
            };
            let field_text = field_name.unraw().to_string();
            if !input.peek(Token![:]) {
                let message = if input.peek(Token![=]) {
                    format!(
                        "legacy `{field_text} = ...` syntax is no longer supported; use `{field_text}: ...`"
                    )
                } else {
                    format!(
                        "expected `:` after `{field_text}`; message parameters must use `args: {{ ... }}`"
                    )
                };
                return Err(SynError::new(field_name.span(), message));
            }
            input.parse::<Token![:]>()?;
            match field_text.as_str() {
                "name" => {
                    if name.is_some() {
                        return Err(SynError::new(field_name.span(), "duplicate `name` field"));
                    }
                    name = Some(input.parse::<LitStr>().map_err(|_| {
                        SynError::new(field_name.span(), "`name` must be a string literal")
                    })?);
                }
                "locale" | "formatter" | "sink" => {
                    let target = match field_text.as_str() {
                        "locale" => &mut locale,
                        "formatter" => &mut formatter,
                        _ => &mut sink,
                    };
                    if target.is_some() {
                        return Err(SynError::new(
                            field_name.span(),
                            format!("duplicate `{field_text}` field"),
                        ));
                    }
                    *target = Some(input.parse::<Expr>()?);
                }
                "args" => {
                    if arguments.is_some() {
                        return Err(SynError::new(field_name.span(), "duplicate `args` field"));
                    }
                    let content;
                    braced!(content in input);
                    let argument_tokens = content.parse::<TokenStream2>()?;
                    parse_named_fields(argument_tokens.clone())?;
                    arguments = Some(argument_tokens);
                }
                _ => {
                    return Err(SynError::new(
                        field_name.span(),
                        format!(
                            "unknown field `{field_text}`; expected `name`, `locale`, `formatter`, `sink`, or `args`"
                        ),
                    ));
                }
            }
            if !input.is_empty() {
                if !input.peek(Token![,]) {
                    return Err(input.error("expected `,` between fields"));
                }
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            name,
            locale,
            formatter,
            sink,
            arguments,
        })
    }
}

impl InvocationFields {
    pub(super) fn parse_tokens(tokens: TokenStream2) -> Result<Self> {
        parse2(tokens)
    }

    pub(super) fn name(&self) -> Option<&LitStr> {
        self.name.as_ref()
    }

    pub(super) fn locale(&self) -> Option<&Expr> {
        self.locale.as_ref()
    }

    pub(super) fn formatter(&self) -> Option<&Expr> {
        self.formatter.as_ref()
    }

    pub(super) fn sink(&self) -> Option<&Expr> {
        self.sink.as_ref()
    }

    pub(super) fn arguments(&self) -> Option<&TokenStream2> {
        self.arguments.as_ref()
    }
}

impl InvocationMetadata {
    fn validate_parameter_names(parameters: &[ParameterSpec]) -> Result<()> {
        let mut names = BTreeSet::new();
        for parameter in parameters {
            if !Self::is_source_identifier(&parameter.name) {
                return Err(SynError::new(
                    Span::call_site(),
                    "generated message metadata contains an invalid parameter name",
                ));
            }
            if !names.insert(parameter.name.as_str()) {
                return Err(SynError::new(
                    Span::call_site(),
                    format!(
                        "generated message metadata repeats parameter `{}`",
                        parameter.name
                    ),
                ));
            }
        }
        Ok(())
    }

    fn is_source_identifier(name: &str) -> bool {
        let mut bytes = name.bytes();
        let Some(first) = bytes.next() else {
            return false;
        };
        (first.is_ascii_alphabetic() || first == b'_')
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    }
}

impl Parse for ParameterSpec {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let parameter_content;
        parenthesized!(parameter_content in input);
        let (name, _) = parse_source_identifier(&parameter_content)?;
        parameter_content.parse::<Token![,]>()?;
        let kind_ident = Ident::parse_any(&parameter_content)?;
        if !parameter_content.is_empty() {
            return Err(
                parameter_content.error("unexpected tokens in generated parameter metadata")
            );
        }
        let kind = match kind_ident.unraw().to_string().as_str() {
            "string" => ArgumentKind::String,
            "number" => ArgumentKind::Number,
            "plural_count" => ArgumentKind::PluralCount,
            "plural_decimal" => ArgumentKind::PluralDecimal,
            "ordinal_count" => ArgumentKind::OrdinalCount,
            "decimal" => ArgumentKind::Decimal,
            "money" => ArgumentKind::Money,
            "date" => ArgumentKind::Date,
            "time" => ArgumentKind::Time,
            "instant" => ArgumentKind::Instant,
            _ => {
                return Err(SynError::new(
                    kind_ident.span(),
                    "generated message metadata contains an unknown argument type",
                ));
            }
        };
        Ok(Self { name, kind })
    }
}

impl Parse for NamedField {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let (name, name_span) = parse_source_identifier(input)?;
        let shorthand = !input.peek(Token![:]);
        let value = if !shorthand {
            input.parse::<Token![:]>()?;
            input.parse::<Expr>()?
        } else if Self::requires_raw_reference(&name) {
            parse_quote_spanned!(name_span=> _)
        } else {
            let value_ident = if Self::is_keyword(&name) {
                Ident::new_raw(&name, name_span)
            } else {
                Ident::new(&name, name_span)
            };
            parse_quote_spanned!(name_span=> #value_ident)
        };
        Ok(Self {
            name,
            name_span,
            value,
            shorthand,
        })
    }
}

fn parse_source_identifier(input: ParseStream<'_>) -> Result<(String, Span)> {
    if input.peek(Token![_]) {
        input.parse::<Token![_]>()?;
        return Ok(("_".to_string(), Span::call_site()));
    }
    let identifier = Ident::parse_any(input)?;
    Ok((identifier.unraw().to_string(), identifier.span()))
}

impl NamedField {
    fn requires_raw_reference(name: &str) -> bool {
        matches!(name, "_" | "self" | "Self" | "super" | "crate")
    }

    fn is_keyword(name: &str) -> bool {
        matches!(
            name,
            "abstract"
                | "as"
                | "async"
                | "await"
                | "become"
                | "box"
                | "break"
                | "const"
                | "continue"
                | "do"
                | "dyn"
                | "else"
                | "enum"
                | "extern"
                | "false"
                | "final"
                | "fn"
                | "for"
                | "gen"
                | "if"
                | "impl"
                | "in"
                | "let"
                | "loop"
                | "macro"
                | "match"
                | "mod"
                | "move"
                | "mut"
                | "override"
                | "priv"
                | "pub"
                | "raw"
                | "ref"
                | "return"
                | "safe"
                | "static"
                | "struct"
                | "trait"
                | "try"
                | "type"
                | "typeof"
                | "union"
                | "unsafe"
                | "unsized"
                | "use"
                | "virtual"
                | "where"
                | "while"
                | "yield"
        )
    }
}

impl InvocationExpansion {
    pub(super) fn expand(input: TokenStream2) -> Result<TokenStream2> {
        let mut tokens = input.into_iter();
        match tokens.next() {
            Some(TokenTree::Punct(punctuation)) if punctuation.as_char() == '@' => {}
            Some(tree) => {
                return Err(SynError::new(
                    tree.span(),
                    "expected generated invocation mode",
                ));
            }
            None => {
                return Err(SynError::new(
                    Span::call_site(),
                    "missing generated invocation mode",
                ));
            }
        }
        let mode = match tokens.next() {
            Some(TokenTree::Ident(identifier)) => identifier.to_string(),
            Some(tree) => return Err(SynError::new(tree.span(), "expected invocation mode")),
            None => {
                return Err(SynError::new(Span::call_site(), "missing invocation mode"));
            }
        };
        let metadata_group = take_group(&mut tokens, Delimiter::Bracket, "signature metadata")?;
        let metadata = parse2::<InvocationMetadata>(metadata_group.stream())?;
        match mode.as_str() {
            "direct" => {
                let message_data_group =
                    take_group(&mut tokens, Delimiter::Brace, "message shard")?;
                let raw_arguments = tokens.collect::<TokenStream2>();
                let fields = InvocationFields::parse_tokens(raw_arguments)?;
                if let Some(name) = fields.name() {
                    return Err(SynError::new(
                        name.span(),
                        "per-message macros do not accept `name`; remove that field",
                    ));
                }
                let locale = fields.locale().cloned().ok_or_else(|| {
                    SynError::new(
                        Span::call_site(),
                        "per-message calls require a `locale: expression` field",
                    )
                })?;
                let message_data = parse_group_expression(message_data_group, "message shard")?;
                let arguments = parse_arguments(
                    fields.arguments().cloned(),
                    &metadata.parameters,
                    Span::call_site(),
                )?;
                emit_render_expression(
                    metadata,
                    RenderInvocation {
                        locale,
                        message_data,
                        formatter: fields.formatter().cloned(),
                        sink: fields.sink().cloned(),
                        arguments,
                    },
                )
            }
            "dispatch" => {
                let locale_group = take_group(&mut tokens, Delimiter::Brace, "locale expression")?;
                let message_data_group =
                    take_group(&mut tokens, Delimiter::Brace, "message shard")?;
                let formatter_group =
                    take_group(&mut tokens, Delimiter::Brace, "formatter expression")?;
                let sink_group = take_group(&mut tokens, Delimiter::Brace, "sink expression")?;
                let arguments_group = take_group(&mut tokens, Delimiter::Brace, "argument values")?;
                if let Some(extra) = tokens.next() {
                    return Err(SynError::new(
                        extra.span(),
                        "unexpected tokens after generated message invocation",
                    ));
                }
                let locale = parse_group_expression(locale_group, "locale")?;
                let message_data = parse_group_expression(message_data_group, "message shard")?;
                let formatter = if formatter_group.stream().is_empty() {
                    None
                } else {
                    Some(parse_group_expression(formatter_group, "formatter")?)
                };
                let sink = if sink_group.stream().is_empty() {
                    None
                } else {
                    Some(parse_group_expression(sink_group, "sink")?)
                };
                let arguments = parse_arguments(
                    Some(arguments_group.stream()),
                    &metadata.parameters,
                    arguments_group.span(),
                )?;
                emit_render_expression(
                    metadata,
                    RenderInvocation {
                        locale,
                        message_data,
                        formatter,
                        sink,
                        arguments,
                    },
                )
            }
            _ => Err(SynError::new(
                Span::call_site(),
                "unknown generated invocation mode",
            )),
        }
    }
}

fn take_group(
    tokens: &mut impl Iterator<Item = TokenTree>,
    delimiter: Delimiter,
    description: &str,
) -> Result<Group> {
    match tokens.next() {
        Some(TokenTree::Group(group)) if group.delimiter() == delimiter => Ok(group),
        Some(tree) => Err(SynError::new(
            tree.span(),
            format!("expected generated {description} group"),
        )),
        None => Err(SynError::new(
            Span::call_site(),
            format!("missing generated {description} group"),
        )),
    }
}

fn parse_group_expression(group: Group, description: &str) -> Result<Expr> {
    if group.stream().is_empty() {
        return Err(SynError::new(
            group.span(),
            format!("missing {description} expression"),
        ));
    }
    parse2::<Expr>(group.stream())
}

fn parse_arguments(
    tokens: Option<TokenStream2>,
    parameters: &[ParameterSpec],
    argument_span: Span,
) -> Result<BoundArguments> {
    let fields = tokens.map_or_else(|| Ok(Vec::new()), parse_named_fields)?;
    bind_named_arguments(parameters, fields, argument_span)
}

fn parse_named_fields(tokens: TokenStream2) -> Result<Vec<NamedField>> {
    Ok(Punctuated::<NamedField, Token![,]>::parse_terminated
        .parse2(tokens)?
        .into_iter()
        .collect())
}

fn bind_named_arguments(
    parameters: &[ParameterSpec],
    fields: Vec<NamedField>,
    argument_span: Span,
) -> Result<BoundArguments> {
    let mut schema_to_evaluation = vec![None; parameters.len()];
    let mut evaluation_order = Vec::with_capacity(fields.len());
    for field in fields {
        let source_name = field.name;
        if field.shorthand && NamedField::requires_raw_reference(&source_name) {
            return Err(SynError::new(
                field.name_span,
                format!(
                    "named shorthand `{source_name}` cannot be used as a value; use an explicit expression"
                ),
            ));
        }
        let Some(schema_index) = parameters
            .iter()
            .position(|parameter| parameter.name == source_name)
        else {
            return Err(SynError::new(
                field.name_span,
                format!("unknown named message argument `{source_name}`"),
            ));
        };
        let slot = schema_to_evaluation
            .get_mut(schema_index)
            .ok_or_else(|| SynError::new(field.name_span, "invalid message signature metadata"))?;
        if slot.is_some() {
            return Err(SynError::new(
                field.name_span,
                format!("duplicate named message argument `{source_name}`"),
            ));
        }
        *slot = Some(evaluation_order.len());
        evaluation_order.push(field.value);
    }

    let mut schema_slots = Vec::with_capacity(parameters.len());
    let mut missing = Vec::new();
    for (index, slot) in schema_to_evaluation.into_iter().enumerate() {
        match slot {
            Some(evaluation_index) => schema_slots.push(evaluation_index),
            None => {
                let parameter = parameters.get(index).ok_or_else(|| {
                    SynError::new(argument_span, "invalid message signature metadata")
                })?;
                missing.push(parameter.name.as_str());
            }
        }
    }
    if !missing.is_empty() {
        let missing_names = missing
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let message = if missing.len() == 1 {
            format!("missing named message argument {missing_names}")
        } else {
            format!("missing named message arguments {missing_names}")
        };
        return Err(SynError::new(argument_span, message));
    }
    Ok(BoundArguments {
        evaluation_order,
        schema_slots,
    })
}

fn emit_render_expression(
    metadata: InvocationMetadata,
    invocation: RenderInvocation,
) -> Result<TokenStream2> {
    let RenderInvocation {
        locale,
        message_data,
        formatter,
        sink,
        arguments,
    } = invocation;
    let locale_binding = format_ident!("__u18n_locale", span = Span::mixed_site());
    let message_data_binding = format_ident!("__u18n_message_data", span = Span::mixed_site());
    let formatter_binding = format_ident!("__u18n_formatter", span = Span::mixed_site());
    let sink_binding = format_ident!("__u18n_sink", span = Span::mixed_site());
    if metadata.needs_formatter
        && formatter.is_none()
        && sink.is_none()
        && !metadata.allow_missing_formatter
    {
        return Err(SynError::new(
            Span::call_site(),
            "message requires a `formatter: expression` field when no sink is specified",
        ));
    }
    if !metadata.needs_formatter && formatter.is_some() {
        return Err(SynError::new(
            Span::call_site(),
            "message does not accept a formatter",
        ));
    }
    if metadata.parameters.is_empty() && metadata.needs_formatter {
        return Err(SynError::new(
            Span::call_site(),
            "zero-argument message cannot require a formatter",
        ));
    }
    let argument_bindings = (0..arguments.evaluation_order.len())
        .map(|index| format_ident!("__u18n_value_{index}", span = Span::mixed_site()))
        .collect::<Vec<_>>();
    let argument_binding_statements = arguments
        .evaluation_order
        .iter()
        .zip(&argument_bindings)
        .map(
            |(expression, binding)| quote_spanned!(expression.span()=> let #binding = #expression;),
        )
        .collect::<Vec<_>>();
    let mut argument_values = Vec::with_capacity(arguments.schema_slots.len());
    for (schema_index, evaluation_index) in arguments.schema_slots.iter().enumerate() {
        let parameter = metadata.parameters.get(schema_index).ok_or_else(|| {
            SynError::new(Span::call_site(), "invalid message signature metadata")
        })?;
        let binding = argument_bindings
            .get(*evaluation_index)
            .ok_or_else(|| SynError::new(Span::call_site(), "invalid message argument mapping"))?;
        argument_values.push(argument_value(
            parameter.kind,
            binding,
            &metadata.runtime_anchor,
        ));
    }

    let argument_count = LitInt::new(&metadata.parameters.len().to_string(), Span::call_site());
    let runtime_anchor = &metadata.runtime_anchor;
    let argument_values = quote! { #(#argument_values),* };
    let has_formatter = formatter.is_some();
    let cow_macro_output = metadata.cow_macro_output;
    let formatter_binding_statement = if let Some(formatter_expression) = formatter {
        quote_spanned!(formatter_expression.span()=> let #formatter_binding = #formatter_expression;)
    } else {
        quote! {}
    };
    let sink_binding_statement = if let Some(sink_expression) = sink {
        quote_spanned!(sink_expression.span()=> let #sink_binding = #sink_expression;)
    } else {
        quote! {}
    };

    if metadata.parameters.is_empty() {
        let render = if sink_binding_statement.is_empty() {
            if metadata.cow_macro_output {
                quote!(#message_data_binding.render(#locale_binding, &[]))
            } else {
                quote!(#message_data_binding.render_static(#locale_binding))
            }
        } else {
            quote!(#message_data_binding.render_to(
                #locale_binding,
                &[],
                &mut *#sink_binding
            ))
        };
        return Ok(quote! {{
            let #locale_binding = #locale;
            let #message_data_binding = #message_data;
            #sink_binding_statement
            #render
        }});
    }

    if metadata.needs_formatter {
        let render = if sink_binding_statement.is_empty() {
            if has_formatter {
                let method = if cow_macro_output {
                    quote!(render_with)
                } else {
                    quote!(render_with_owned)
                };
                quote!(#message_data_binding.#method(
                    #locale_binding,
                    #formatter_binding,
                    &__u18n_arguments
                ))
            } else {
                let method = if cow_macro_output {
                    quote!(render)
                } else {
                    quote!(render_owned)
                };
                quote!(#message_data_binding.#method(
                    #locale_binding,
                    &__u18n_arguments
                ))
            }
        } else {
            if has_formatter {
                quote!(#message_data_binding.render_with_to(
                    #locale_binding,
                    #formatter_binding,
                    &__u18n_arguments,
                    &mut *#sink_binding
                ))
            } else {
                quote!(#message_data_binding.render_to(
                #locale_binding,
                    &__u18n_arguments,
                    &mut *#sink_binding
                ))
            }
        };
        Ok(quote! {{
            let #locale_binding = #locale;
            let #message_data_binding = #message_data;
            #formatter_binding_statement
            #sink_binding_statement
            #(#argument_binding_statements)*
            let __u18n_arguments: [#runtime_anchor::ArgumentValue<'_>; #argument_count] = [#argument_values];
            #render
        }})
    } else {
        let render = if sink_binding_statement.is_empty() {
            let method = if cow_macro_output {
                quote!(render)
            } else {
                quote!(render_owned)
            };
            quote!(#message_data_binding.#method(#locale_binding, &__u18n_arguments))
        } else {
            quote!(#message_data_binding.render_to(
                #locale_binding,
                &__u18n_arguments,
                &mut *#sink_binding
            ))
        };
        Ok(quote! {{
            let #locale_binding = #locale;
            let #message_data_binding = #message_data;
            #sink_binding_statement
            #(#argument_binding_statements)*
            let __u18n_arguments: [#runtime_anchor::ArgumentValue<'_>; #argument_count] = [#argument_values];
            #render
        }})
    }
}

fn argument_value(kind: ArgumentKind, binding: &Ident, runtime_anchor: &Path) -> TokenStream2 {
    match kind {
        ArgumentKind::String => quote!(#runtime_anchor::ArgumentValue::String(#binding)),
        ArgumentKind::Number => quote!(#runtime_anchor::ArgumentValue::Number(&#binding)),
        ArgumentKind::PluralCount => {
            quote!(#runtime_anchor::ArgumentValue::PluralCount(#binding))
        }
        ArgumentKind::PluralDecimal => {
            quote!(#runtime_anchor::ArgumentValue::PluralDecimal(#binding))
        }
        ArgumentKind::OrdinalCount => {
            quote!(#runtime_anchor::ArgumentValue::OrdinalCount(#binding))
        }
        ArgumentKind::Decimal => quote!(#runtime_anchor::ArgumentValue::Decimal(#binding)),
        ArgumentKind::Money => quote!(#runtime_anchor::ArgumentValue::Money(#binding)),
        ArgumentKind::Date => quote!(#runtime_anchor::ArgumentValue::Date(#binding)),
        ArgumentKind::Time => quote!(#runtime_anchor::ArgumentValue::Time(#binding)),
        ArgumentKind::Instant => quote!(#runtime_anchor::ArgumentValue::Instant(#binding)),
    }
}

#[cfg(test)]
#[test]
fn binds_named_values_in_source_order_and_schema_order() {
    let parameters = vec![
        ParameterSpec {
            name: String::from("first"),
            kind: ArgumentKind::String,
        },
        ParameterSpec {
            name: String::from("second"),
            kind: ArgumentKind::Number,
        },
    ];
    let input: TokenStream2 = quote!(second: right(), first,);
    let result = parse_arguments(Some(input), &parameters, Span::call_site());

    assert!(result.is_ok());
    let Some(arguments) = result.ok() else {
        return;
    };
    assert_eq!(arguments.schema_slots, vec![1, 0]);
    assert_eq!(arguments.evaluation_order.len(), 2);
}

#[cfg(test)]
#[test]
fn rejects_missing_extra_duplicate_unknown_mixed_and_wrong_arity() {
    let parameters = vec![
        ParameterSpec {
            name: String::from("first"),
            kind: ArgumentKind::String,
        },
        ParameterSpec {
            name: String::from("second"),
            kind: ArgumentKind::Number,
        },
    ];
    let invalid_inputs: [TokenStream2; 4] = [
        quote!(first: value),
        quote!(first: a, second: b, extra: c),
        quote!(first: a, first: b),
        quote!(unknown: value, second: other),
    ];
    for input in invalid_inputs {
        assert!(parse_arguments(Some(input), &parameters, Span::call_site()).is_err());
    }
    let named_error = parse_arguments(Some(quote!(first: value)), &parameters, Span::call_site())
        .err()
        .map(|error| error.to_string());
    assert!(
        named_error
            .is_some_and(|error| { error.contains("missing named message argument `second`") })
    );
}

#[cfg(test)]
#[test]
fn permits_omitted_or_empty_arguments_only_for_zero_argument_messages() {
    let parameters = Vec::new();
    let omitted = parse_arguments(None, &parameters, Span::call_site());
    let named = parse_arguments(Some(TokenStream2::new()), &parameters, Span::call_site());

    assert!(omitted.is_ok());
    assert!(named.is_ok());
}

#[cfg(test)]
#[test]
fn formatter_requirement_changes_only_for_sink_and_fixed_fallback_paths() {
    let expand = |allow_missing_formatter, formatter, sink| {
        let parameters = vec![ParameterSpec {
            name: String::from("value"),
            kind: ArgumentKind::Number,
        }];
        let arguments =
            parse_arguments(Some(quote!(value: value)), &parameters, Span::call_site())?;
        emit_render_expression(
            InvocationMetadata {
                needs_formatter: true,
                cow_macro_output: false,
                allow_missing_formatter,
                runtime_anchor: parse_quote!(crate_anchor),
                parameters,
            },
            RenderInvocation {
                locale: parse_quote!(locale),
                message_data: parse_quote!(message),
                formatter,
                sink,
                arguments,
            },
        )
    };

    assert!(expand(false, None, None).is_err());
    assert!(
        expand(false, None, Some(parse_quote!(&mut output)))
            .is_ok_and(|tokens| tokens.to_string().contains("render_to"))
    );
    assert!(
        expand(true, None, None).is_ok_and(|tokens| tokens.to_string().contains("render_owned"))
    );
}

#[cfg(test)]
#[test]
fn supports_schema_parameter_named_underscore() {
    let metadata =
        parse2::<InvocationMetadata>(quote!(false, false, false, [crate_anchor], [(_, string)]));
    assert!(metadata.is_ok());
    let Some(metadata) = metadata.ok() else {
        return;
    };
    let arguments = parse_arguments(
        Some(quote!(_: value)),
        &metadata.parameters,
        Span::call_site(),
    );

    assert!(arguments.is_ok());
    assert!(parse_arguments(Some(quote!(_)), &metadata.parameters, Span::call_site()).is_err());
}

#[cfg(test)]
#[test]
fn parses_try_expressions_and_trailing_commas() {
    let parameters = vec![
        ParameterSpec {
            name: String::from("first"),
            kind: ArgumentKind::String,
        },
        ParameterSpec {
            name: String::from("second"),
            kind: ArgumentKind::Number,
        },
    ];
    let expression = parse2::<Expr>(quote!(require_value(first, second)?));
    let input: TokenStream2 = quote!(
        first: require_value(first, second)?,
        second: make_another(first, second),
    );
    let arguments = parse_arguments(Some(input), &parameters, Span::call_site());

    assert!(expression.is_ok());
    assert!(arguments.is_ok());
}

#[cfg(test)]
#[test]
fn parses_closures_macros_and_nested_expression_commas() {
    let parameters = vec![
        ParameterSpec {
            name: String::from("first"),
            kind: ArgumentKind::String,
        },
        ParameterSpec {
            name: String::from("second"),
            kind: ArgumentKind::Number,
        },
    ];
    let arguments = parse_arguments(
        Some(quote!(
            first: apply(|left, right| combine(left, right), one, two)?,
            second: number!(choose(3, 4)),
        )),
        &parameters,
        Span::call_site(),
    );

    assert!(arguments.is_ok());
    let Some(arguments) = arguments.ok() else {
        return;
    };
    assert_eq!(arguments.evaluation_order.len(), 2);
    assert!(arguments.schema_slots == [0, 1]);
}

#[cfg(test)]
#[test]
fn accepts_a_brace_delimited_block_as_a_named_expression() {
    let parameters = vec![ParameterSpec {
        name: String::from("value"),
        kind: ArgumentKind::Number,
    }];
    let input = quote!(value: {
        let value = make_value();
        value
    });
    let arguments = parse_arguments(Some(input), &parameters, Span::call_site());

    assert!(arguments.is_ok());
    let Some(arguments) = arguments.ok() else {
        return;
    };
    assert_eq!(arguments.schema_slots, vec![0]);
    assert!(matches!(
        arguments.evaluation_order.as_slice(),
        [Expr::Block(_)]
    ));
}
