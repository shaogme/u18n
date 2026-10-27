mod fallback;
mod model;
mod pattern;
mod schema;

use std::{collections::BTreeMap, convert::TryFrom};
use u18n_locale_tag::ParsedTag;

use u18n_ast::{PluralCategory, PluralKind, SchemaAst};

use crate::{
    cldr::{CldrData, RuleSetData},
    diagnostic::Diagnostic,
    semantic::{
        fallback::{FallbackContext, FallbackLinker},
        model::LocaleWork,
        schema::SchemaLinker,
    },
};

pub(crate) use model::{
    LinkedCatalog, LinkedLocale, LinkedPattern, LinkedPatternPart, LinkedPluralBranch,
    LinkedSelectBranch, LocaleInput, LocaleParseStatus,
};

pub(crate) struct ValidationRequest<'a> {
    pub(crate) schema: &'a SchemaAst,
    pub(crate) locale_inputs: &'a [LocaleInput],
    pub(crate) cldr: &'a CldrData,
    pub(crate) schema_source: &'a str,
    pub(crate) parse_status: &'a LocaleParseStatus,
    pub(crate) allow_missing: bool,
}

pub(super) struct Validator;

impl Validator {
    pub(super) fn validate(
        request: ValidationRequest<'_>,
    ) -> Result<LinkedCatalog, Vec<Diagnostic>> {
        let ValidationRequest {
            schema,
            locale_inputs,
            cldr,
            schema_source,
            parse_status,
            allow_missing,
        } = request;
        let mut diagnostics = Vec::new();
        if locale_inputs.is_empty() && !parse_status.had_errors {
            diagnostics.push(Diagnostic::from_span(
                "<locales>",
                "",
                schema.span,
                "at least one locale file is required".to_string(),
            ));
        }
        if schema.messages.is_empty() {
            diagnostics.push(Diagnostic::from_span(
                "<schema>",
                schema_source,
                schema.span,
                "schema must declare at least one message".to_string(),
            ));
        }

        let schema_messages = SchemaLinker::check_schema(schema, schema_source, &mut diagnostics);
        let mut source_tags: BTreeMap<String, &LocaleInput> = BTreeMap::new();
        let mut normalized_tags: BTreeMap<String, &LocaleInput> = BTreeMap::new();
        let mut duplicate_inputs = Vec::new();
        for input in locale_inputs {
            let tag = &input.ast.locale.source_tag;
            if let Some(original) = source_tags.get(tag) {
                diagnostics.push(
                    Diagnostic::from_span(
                        &input.path,
                        &input.source,
                        input.ast.locale.span,
                        format!("duplicate source locale tag `{tag}`"),
                    )
                    .with_related_span(
                        &original.path,
                        &original.source,
                        original.ast.locale.span,
                        "first catalog with this source tag",
                    ),
                );
                duplicate_inputs.push(input);
            } else {
                if ParsedTag::parse(tag).is_ok() {
                    let normalized_tag = tag.to_ascii_lowercase();
                    if let Some(original) = normalized_tags.get(&normalized_tag) {
                        diagnostics.push(
                            Diagnostic::from_span(
                                &input.path,
                                &input.source,
                                input.ast.locale.span,
                                format!(
                                    "source locale tag `{tag}` duplicates a normalized locale tag"
                                ),
                            )
                            .with_related_span(
                                &original.path,
                                &original.source,
                                original.ast.locale.span,
                                "first catalog with this normalized locale tag",
                            ),
                        );
                    } else {
                        normalized_tags.insert(normalized_tag, input);
                    }
                }
                source_tags.insert(tag.clone(), input);
            }
        }

        let mut work = Vec::new();
        let mut rule_sets = Vec::new();
        for input in source_tags.values() {
            let tag = &input.ast.locale.source_tag;
            let mut rules = BTreeMap::new();
            let mut rule_set_indices = BTreeMap::new();
            let mut rule_errors = BTreeMap::new();
            for kind in [PluralKind::Cardinal, PluralKind::Ordinal] {
                let index = u32::try_from(rule_sets.len()).unwrap_or(u32::MAX);
                let rule_set = match cldr.resolve(tag, kind) {
                    Ok(rule_set) => rule_set.clone(),
                    Err(message) => {
                        if kind == PluralKind::Cardinal {
                            diagnostics.push(Diagnostic::from_span(
                                &input.path,
                                &input.source,
                                input.ast.locale.span,
                                message.clone(),
                            ));
                        }
                        rule_errors.insert(kind, message);
                        RuleSetData {
                            kind,
                            categories: vec![PluralCategory::Other],
                            rules: Vec::new(),
                        }
                    }
                };
                rule_set_indices.insert(kind, index);
                rules.insert(kind, rule_set.clone());
                rule_sets.push(rule_set);
            }
            let mut locale_work = LocaleWork {
                input,
                fallback: input
                    .ast
                    .fallback
                    .as_ref()
                    .map(|locale| locale.source_tag.clone()),
                fallback_span: input.ast.fallback.as_ref().map(|locale| locale.span),
                rules,
                rule_set_indices,
                rule_errors,
                messages: BTreeMap::new(),
            };
            SchemaLinker::link_locale_messages(
                &mut locale_work,
                &schema_messages,
                schema_source,
                &mut diagnostics,
            );
            work.push(locale_work);
        }

        let locale_indices = work
            .iter()
            .enumerate()
            .map(|(index, locale)| (locale.input.ast.locale.source_tag.clone(), index))
            .collect::<BTreeMap<_, _>>();
        for input in duplicate_inputs {
            let tag = &input.ast.locale.source_tag;
            let Some(index) = locale_indices.get(tag).copied() else {
                continue;
            };
            let Some(original) = work.get(index) else {
                continue;
            };
            let mut duplicate_work = LocaleWork {
                input,
                fallback: input
                    .ast
                    .fallback
                    .as_ref()
                    .map(|locale| locale.source_tag.clone()),
                fallback_span: input.ast.fallback.as_ref().map(|locale| locale.span),
                rules: original.rules.clone(),
                rule_set_indices: original.rule_set_indices.clone(),
                rule_errors: original.rule_errors.clone(),
                messages: BTreeMap::new(),
            };
            SchemaLinker::link_locale_messages(
                &mut duplicate_work,
                &schema_messages,
                schema_source,
                &mut diagnostics,
            );
        }
        FallbackLinker::check_fallbacks(
            &work,
            locale_inputs,
            &locale_indices,
            &parse_status.unavailable_tags,
            &mut diagnostics,
        );

        let mut messages = schema.messages.clone();
        messages.sort_by_key(|message| SchemaLinker::key_text(&message.key));
        let locales = FallbackLinker::resolve_catalog_locales(
            FallbackContext {
                work: &work,
                locale_indices: &locale_indices,
                schema_messages: &schema_messages,
                parse_status,
                schema_source,
                allow_missing,
            },
            &mut diagnostics,
        );

        if diagnostics.is_empty() {
            Ok(LinkedCatalog {
                messages,
                locales,
                rule_sets,
            })
        } else {
            Err(diagnostics)
        }
    }
}
