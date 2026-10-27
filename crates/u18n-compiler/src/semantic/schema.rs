use std::collections::BTreeMap;

use u18n_ast::{MessageDef, MessageKey, SchemaAst, Span};

use crate::{
    diagnostic::Diagnostic,
    semantic::{
        model::{LinkedMessage, LocaleWork, ParameterBinding},
        pattern::{PatternLinkContext, PatternLinker},
    },
};

pub(super) struct SchemaLinker;

impl SchemaLinker {
    pub(super) fn check_schema<'schema>(
        schema: &'schema SchemaAst,
        schema_source: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> BTreeMap<String, &'schema MessageDef> {
        let mut messages: BTreeMap<String, &'schema MessageDef> = BTreeMap::new();
        for message in &schema.messages {
            let key = Self::key_text(&message.key);
            if let Some(first) = messages.get(&key) {
                diagnostics.push(
                    Diagnostic::from_span(
                        "<schema>",
                        schema_source,
                        message.key.span,
                        format!("duplicate schema message key `{key}`"),
                    )
                    .with_related_span(
                        "<schema>",
                        schema_source,
                        first.key.span,
                        "first schema declaration",
                    ),
                );
            } else {
                messages.insert(key.clone(), message);
            }
            let mut parameters: BTreeMap<&str, Span> = BTreeMap::new();
            for parameter in &message.parameters {
                if let Some(first_span) = parameters.get(parameter.name.text.as_str()).copied() {
                    diagnostics.push(
                        Diagnostic::from_span(
                            "<schema>",
                            schema_source,
                            parameter.name.span,
                            format!(
                                "duplicate parameter `{}` in message `{key}`",
                                parameter.name.text
                            ),
                        )
                        .with_related_span(
                            "<schema>",
                            schema_source,
                            first_span,
                            "first parameter declaration",
                        ),
                    );
                } else {
                    parameters.insert(parameter.name.text.as_str(), parameter.name.span);
                }
            }
        }
        messages
    }

    pub(super) fn link_locale_messages(
        locale: &mut LocaleWork<'_>,
        schema_messages: &BTreeMap<String, &MessageDef>,
        schema_source: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut first_declarations: BTreeMap<String, Span> = BTreeMap::new();
        for message in &locale.input.ast.messages {
            let key = Self::key_text(&message.key);
            let duplicate = first_declarations.get(&key).copied();
            if let Some(first_span) = duplicate {
                diagnostics.push(
                    Diagnostic::from_span(
                        &locale.input.path,
                        &locale.input.source,
                        message.key.span,
                        format!("duplicate locale message key `{key}`"),
                    )
                    .with_related_span(
                        &locale.input.path,
                        &locale.input.source,
                        first_span,
                        "first locale declaration",
                    ),
                );
            } else {
                first_declarations.insert(key.clone(), message.key.span);
            }
            let Some(definition) = schema_messages.get(&key).copied() else {
                diagnostics.push(Diagnostic::from_span(
                    &locale.input.path,
                    &locale.input.source,
                    message.key.span,
                    format!("locale key `{key}` is not declared in the schema"),
                ));
                continue;
            };
            let parameters = definition
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let slot = u32::try_from(index).unwrap_or(u32::MAX);
                    (
                        parameter.name.text.clone(),
                        ParameterBinding {
                            slot,
                            argument_type: parameter.argument_type,
                            type_span: parameter.type_span,
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let linked_pattern = {
                let context = PatternLinkContext {
                    path: &locale.input.path,
                    source: &locale.input.source,
                    schema_source,
                    schema_message_span: definition.span,
                    parameters: &parameters,
                    rules: &locale.rules,
                    rule_set_indices: &locale.rule_set_indices,
                    rule_errors: &locale.rule_errors,
                };
                PatternLinker::new(context, diagnostics).link(&message.pattern, false)
            };
            if duplicate.is_none() {
                locale.messages.insert(
                    key,
                    LinkedMessage {
                        key: message.key.clone(),
                        pattern: linked_pattern,
                        author_locale: locale.input.ast.locale.source_tag.clone(),
                    },
                );
            }
        }
    }

    pub(super) fn key_text(key: &MessageKey) -> String {
        key.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}
