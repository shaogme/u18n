use std::collections::BTreeSet;

use proc_macro2::{Delimiter, Group, Span, TokenStream as TokenStream2, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    Error as SynError, Expr, Ident, LitBool, LitInt, Result, Token, bracketed,
    ext::IdentExt,
    parenthesized,
    parse::{Parse, ParseStream, Parser},
    parse_quote_spanned, parse2,
    punctuated::Punctuated,
    spanned::Spanned,
};

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
    parameters: Vec<ParameterSpec>,
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

pub(super) struct InvocationExpansion;

impl Parse for InvocationMetadata {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let needs_formatter = input.parse::<LitBool>()?.value;
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
            parameters,
        })
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
        let metadata_group = take_group(&mut tokens, Delimiter::Bracket, "signature metadata")?;
        let locale_group = take_group(&mut tokens, Delimiter::Brace, "locale expression")?;
        let message_data_group = take_group(&mut tokens, Delimiter::Brace, "message shard")?;
        let formatter_group = take_group(&mut tokens, Delimiter::Brace, "formatter expression")?;
        let arguments_group = take_group(&mut tokens, Delimiter::Brace, "argument values")?;
        if let Some(extra) = tokens.next() {
            return Err(SynError::new(
                extra.span(),
                "unexpected tokens after generated message invocation",
            ));
        }

        let metadata = parse2::<InvocationMetadata>(metadata_group.stream())?;
        let locale = parse_group_expression(locale_group, "locale")?;
        let message_data = parse_group_expression(message_data_group, "message shard")?;
        let formatter = if metadata.needs_formatter {
            Some(parse_group_expression(formatter_group, "formatter")?)
        } else if formatter_group.stream().is_empty() {
            None
        } else {
            return Err(SynError::new(
                formatter_group.span(),
                "message does not accept a formatter",
            ));
        };
        let arguments = parse_arguments(
            arguments_group.stream(),
            &metadata.parameters,
            arguments_group.span(),
        )?;
        emit_render_expression(metadata, locale, message_data, formatter, arguments)
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
    tokens: TokenStream2,
    parameters: &[ParameterSpec],
    argument_span: Span,
) -> Result<BoundArguments> {
    let token_trees = tokens.into_iter().collect::<Vec<_>>();
    let first = token_trees.first();
    let mut named_parse_error = None;
    if let Some(TokenTree::Group(group)) = first
        && group.delimiter() == Delimiter::Brace
    {
        if token_trees.len() == 1 {
            match parse_named_fields(group.stream()) {
                Ok(fields) => return bind_named_arguments(parameters, fields, argument_span),
                Err(error) => named_parse_error = Some(error),
            }
        }
        if token_trees.len() > 1 && parse_named_fields(group.stream()).is_ok() {
            return Err(SynError::new(
                group.span(),
                "cannot mix named and positional message arguments",
            ));
        }
    }

    let expressions = match Punctuated::<Expr, Token![,]>::parse_terminated
        .parse2(token_trees.into_iter().collect())
    {
        Ok(expressions) => expressions.into_iter().collect::<Vec<_>>(),
        Err(error) => return Err(named_parse_error.unwrap_or(error)),
    };
    if expressions.len() != parameters.len() {
        return Err(SynError::new(
            argument_span,
            format!(
                "expected {} positional message arguments but received {}",
                parameters.len(),
                expressions.len()
            ),
        ));
    }
    let schema_slots = (0..parameters.len()).collect::<Vec<_>>();
    Ok(BoundArguments {
        evaluation_order: expressions,
        schema_slots,
    })
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
    for (index, slot) in schema_to_evaluation.into_iter().enumerate() {
        match slot {
            Some(evaluation_index) => schema_slots.push(evaluation_index),
            None => {
                let parameter = parameters.get(index).ok_or_else(|| {
                    SynError::new(argument_span, "invalid message signature metadata")
                })?;
                return Err(SynError::new(
                    argument_span,
                    format!("missing named message argument `{}`", parameter.name),
                ));
            }
        }
    }
    Ok(BoundArguments {
        evaluation_order,
        schema_slots,
    })
}

fn emit_render_expression(
    metadata: InvocationMetadata,
    locale: Expr,
    message_data: Expr,
    formatter: Option<Expr>,
    arguments: BoundArguments,
) -> Result<TokenStream2> {
    let locale_binding = format_ident!("__u18n_locale", span = Span::mixed_site());
    let message_data_binding = format_ident!("__u18n_message_data", span = Span::mixed_site());
    let formatter_binding = format_ident!("__u18n_formatter", span = Span::mixed_site());
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
        argument_values.push(argument_value(parameter.kind, binding));
    }

    let argument_count = LitInt::new(&metadata.parameters.len().to_string(), Span::call_site());
    let argument_values = quote! { #(#argument_values),* };
    let formatter_binding_statement = if let Some(formatter_expression) = formatter {
        quote_spanned!(formatter_expression.span()=> let #formatter_binding = #formatter_expression;)
    } else {
        quote! {}
    };
    if metadata.needs_formatter {
        Ok(quote! {{
            let #locale_binding = #locale;
            let #message_data_binding = #message_data;
            #formatter_binding_statement
            #(#argument_binding_statements)*
            let __u18n_arguments: [::u18n::ArgumentValue<'_>; #argument_count] = [#argument_values];
            #message_data_binding.render_with(#locale_binding, #formatter_binding, &__u18n_arguments)
        }})
    } else {
        Ok(quote! {{
            let #locale_binding = #locale;
            let #message_data_binding = #message_data;
            #(#argument_binding_statements)*
            let __u18n_arguments: [::u18n::ArgumentValue<'_>; #argument_count] = [#argument_values];
            #message_data_binding.render(#locale_binding, &__u18n_arguments)
        }})
    }
}

fn argument_value(kind: ArgumentKind, binding: &Ident) -> TokenStream2 {
    match kind {
        ArgumentKind::String => quote!(::u18n::ArgumentValue::String(#binding)),
        ArgumentKind::Number => quote!(::u18n::ArgumentValue::Number(&#binding)),
        ArgumentKind::PluralCount => quote!(::u18n::ArgumentValue::PluralCount(#binding)),
        ArgumentKind::PluralDecimal => quote!(::u18n::ArgumentValue::PluralDecimal(#binding)),
        ArgumentKind::OrdinalCount => quote!(::u18n::ArgumentValue::OrdinalCount(#binding)),
        ArgumentKind::Decimal => quote!(::u18n::ArgumentValue::Decimal(#binding)),
        ArgumentKind::Money => quote!(::u18n::ArgumentValue::Money(#binding)),
        ArgumentKind::Date => quote!(::u18n::ArgumentValue::Date(#binding)),
        ArgumentKind::Time => quote!(::u18n::ArgumentValue::Time(#binding)),
        ArgumentKind::Instant => quote!(::u18n::ArgumentValue::Instant(#binding)),
    }
}

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
    let input: TokenStream2 = quote!({ second: right(), first, });
    let result = parse_arguments(input, &parameters, Span::call_site());

    assert!(result.is_ok());
    let Some(arguments) = result.ok() else {
        return;
    };
    assert_eq!(arguments.schema_slots, vec![1, 0]);
    assert_eq!(arguments.evaluation_order.len(), 2);
}

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
    let invalid_inputs: [TokenStream2; 6] = [
        quote!({ first: value }),
        quote!({ first: a, second: b, extra: c }),
        quote!({ first: a, first: b }),
        quote!({ unknown: value, second: other }),
        quote!({ first: value }, other),
        quote!(one, two, three),
    ];
    for input in invalid_inputs {
        assert!(parse_arguments(input, &parameters, Span::call_site()).is_err());
    }
    let named_error = parse_arguments(quote!({ first: value }), &parameters, Span::call_site())
        .err()
        .map(|error| error.to_string());
    assert!(
        named_error
            .is_some_and(|error| { error.contains("missing named message argument `second`") })
    );
}

#[test]
fn accepts_zero_argument_positional_and_named_forms() {
    let parameters = Vec::new();
    let positional = parse_arguments(TokenStream2::new(), &parameters, Span::call_site());
    let named = parse_arguments(quote!({}), &parameters, Span::call_site());

    assert!(positional.is_ok());
    assert!(named.is_ok());
}

#[test]
fn supports_schema_parameter_named_underscore() {
    let metadata = parse2::<InvocationMetadata>(quote!(false, [(_, string)]));
    assert!(metadata.is_ok());
    let Some(metadata) = metadata.ok() else {
        return;
    };
    let arguments = parse_arguments(
        quote!({ _: value }),
        &metadata.parameters,
        Span::call_site(),
    );

    assert!(arguments.is_ok());
    assert!(parse_arguments(quote!({ _ }), &metadata.parameters, Span::call_site(),).is_err());
}

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
    let input: TokenStream2 = quote!(require_value(first, second)?, another,);
    let arguments = parse_arguments(input, &parameters, Span::call_site());

    assert!(expression.is_ok());
    assert!(arguments.is_ok());
}

#[test]
fn accepts_a_brace_delimited_block_as_a_positional_expression() {
    let parameters = vec![ParameterSpec {
        name: String::from("value"),
        kind: ArgumentKind::Number,
    }];
    let input = quote!({
        let value = make_value();
        value
    });
    let arguments = parse_arguments(input, &parameters, Span::call_site());

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
