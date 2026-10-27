use std::collections::{BTreeMap, BTreeSet};

use u18n_ast::MessageDef;

use crate::{
    diagnostic::Diagnostic,
    semantic::model::{LinkedLocale, LocaleInput, LocaleParseStatus, LocaleWork, ResolvedMessage},
};

pub(super) struct FallbackContext<'a, 'input> {
    pub(super) work: &'a [LocaleWork<'input>],
    pub(super) locale_indices: &'a BTreeMap<String, usize>,
    pub(super) schema_messages: &'a BTreeMap<String, &'a MessageDef>,
    pub(super) parse_status: &'a LocaleParseStatus,
    pub(super) schema_source: &'a str,
    pub(super) allow_missing: bool,
}

pub(super) struct FallbackLinker;

impl FallbackLinker {
    pub(super) fn resolve_catalog_locales(
        context: FallbackContext<'_, '_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<LinkedLocale> {
        let mut locales = Vec::new();
        for (locale_index, locale) in context.work.iter().enumerate() {
            let mut resolved_messages = Vec::new();
            for (key, definition) in context.schema_messages {
                let mut current = Some(locale_index);
                let mut visited = BTreeSet::new();
                let mut found = None;
                let mut incomplete_fallback = false;
                while let Some(index) = current {
                    if !visited.insert(index) {
                        break;
                    }
                    let Some(candidate) = context.work.get(index) else {
                        break;
                    };
                    if let Some(message) = candidate.messages.get(key) {
                        found = Some(message);
                        break;
                    }
                    current = candidate.fallback.as_ref().and_then(|tag| {
                        match context.locale_indices.get(tag).copied() {
                            Some(next) => Some(next),
                            None => {
                                incomplete_fallback =
                                    context.parse_status.unavailable_tags.contains(tag);
                                None
                            }
                        }
                    });
                }
                let message = found.cloned();
                if message.is_none() && !context.allow_missing && !incomplete_fallback {
                    diagnostics.push(
                        Diagnostic::from_span(
                            &locale.input.path,
                            &locale.input.source,
                            locale.input.ast.locale_span,
                            format!(
                                "locale `{}` is missing schema key `{key}` from its fallback chain",
                                locale.input.ast.locale.source_tag
                            ),
                        )
                        .with_related_span(
                            "<schema>",
                            context.schema_source,
                            definition.key.span,
                            "schema key required by this catalog",
                        ),
                    );
                }
                resolved_messages.push(ResolvedMessage {
                    key: definition.key.clone(),
                    message,
                });
            }
            locales.push(LinkedLocale {
                ast: locale.input.ast.clone(),
                messages: resolved_messages,
            });
        }
        locales
    }

    pub(super) fn check_fallbacks(
        locales: &[LocaleWork<'_>],
        locale_inputs: &[LocaleInput],
        locale_indices: &BTreeMap<String, usize>,
        unavailable_tags: &BTreeSet<String>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for input in locale_inputs {
            let Some(target) = &input.ast.fallback else {
                continue;
            };
            if !locale_indices.contains_key(&target.source_tag)
                && !unavailable_tags.contains(&target.source_tag)
            {
                diagnostics.push(Diagnostic::from_span(
                    &input.path,
                    &input.source,
                    target.span,
                    format!(
                        "fallback locale `{}` is not present in the input set",
                        target.source_tag
                    ),
                ));
            }
        }
        let mut reported: BTreeSet<Vec<String>> = BTreeSet::new();
        for (start, _) in locales.iter().enumerate() {
            let mut current = Some(start);
            let mut positions: BTreeMap<usize, usize> = BTreeMap::new();
            let mut chain: Vec<usize> = Vec::new();
            while let Some(index) = current {
                if let Some(cycle_start) = positions.get(&index).copied() {
                    let cycle = chain.get(cycle_start..).unwrap_or(&[]);
                    let mut cycle_key: Vec<String> = cycle
                        .iter()
                        .filter_map(|cycle_index| {
                            locales
                                .get(*cycle_index)
                                .map(|item| item.input.ast.locale.source_tag.clone())
                        })
                        .collect();
                    cycle_key.sort();
                    if reported.insert(cycle_key) {
                        let edge = cycle.last().copied();
                        let from = edge.and_then(|edge_index| locales.get(edge_index));
                        let to = locales.get(index);
                        if let (Some(from), Some(to)) = (from, to) {
                            diagnostics.push(
                                Diagnostic::from_span(
                                    &from.input.path,
                                    &from.input.source,
                                    from.fallback_span.unwrap_or(from.input.ast.locale_span),
                                    format!(
                                        "fallback cycle reaches locale `{}`",
                                        to.input.ast.locale.source_tag
                                    ),
                                )
                                .with_related_span(
                                    &to.input.path,
                                    &to.input.source,
                                    to.input.ast.locale.span,
                                    "locale reached again to close this fallback cycle",
                                ),
                            );
                        }
                    }
                    break;
                }
                positions.insert(index, chain.len());
                chain.push(index);
                current = locales
                    .get(index)
                    .and_then(|item| item.fallback.as_ref())
                    .and_then(|tag| locale_indices.get(tag))
                    .copied();
            }
        }
    }
}
