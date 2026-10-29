use std::collections::BTreeMap;

use u18n_ast::{
    ArgumentType, FormatOptions, Identifier, Pattern, PatternPart, PluralBranch, PluralCase,
    PluralCategory, PluralKind, SelectBranch, Span,
};

use crate::{
    cldr::RuleSetData,
    diagnostic::Diagnostic,
    semantic::model::{
        LinkedPattern, LinkedPatternPart, LinkedPluralBranch, LinkedSelectBranch, ParameterBinding,
    },
};

pub(super) struct PatternLinkContext<'a> {
    pub(super) path: &'a str,
    pub(super) source: &'a str,
    pub(super) schema_source: &'a str,
    pub(super) schema_message_span: Span,
    pub(super) parameters: &'a BTreeMap<String, ParameterBinding>,
    pub(super) rules: &'a BTreeMap<PluralKind, RuleSetData>,
    pub(super) rule_set_indices: &'a BTreeMap<PluralKind, u32>,
    pub(super) rule_errors: &'a BTreeMap<PluralKind, String>,
}

pub(super) struct PatternLinker<'a> {
    path: &'a str,
    source: &'a str,
    schema_source: &'a str,
    schema_message_span: Span,
    parameters: &'a BTreeMap<String, ParameterBinding>,
    rules: &'a BTreeMap<PluralKind, RuleSetData>,
    rule_set_indices: &'a BTreeMap<PluralKind, u32>,
    rule_errors: &'a BTreeMap<PluralKind, String>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl<'a> PatternLinker<'a> {
    pub(super) fn new(
        context: PatternLinkContext<'a>,
        diagnostics: &'a mut Vec<Diagnostic>,
    ) -> Self {
        Self {
            path: context.path,
            source: context.source,
            schema_source: context.schema_source,
            schema_message_span: context.schema_message_span,
            parameters: context.parameters,
            rules: context.rules,
            rule_set_indices: context.rule_set_indices,
            rule_errors: context.rule_errors,
            diagnostics,
        }
    }
}

impl PatternLinker<'_> {
    pub(super) fn link(&mut self, pattern: &Pattern, inside_plural: bool) -> LinkedPattern {
        let parts = pattern
            .parts
            .iter()
            .map(|part| match part {
                PatternPart::Text { value, span } => LinkedPatternPart::Text {
                    value: value.clone(),
                    span: *span,
                },
                PatternPart::LineBreak { span } => LinkedPatternPart::LineBreak { span: *span },
                PatternPart::Argument { name, span } => {
                    let slot = self.bind_plain(name);
                    LinkedPatternPart::Argument { slot, span: *span }
                }
                PatternPart::Plural {
                    name,
                    kind,
                    branches,
                    span,
                } => {
                    let slot = match kind {
                        PluralKind::Cardinal => self.bind_cardinal(name),
                        PluralKind::Ordinal => self.bind(name, Some(ArgumentType::OrdinalCount)),
                    };
                    let categories = self
                        .rules
                        .get(kind)
                        .map(|rules| rules.categories.as_slice())
                        .unwrap_or(&[]);
                    if let Some(message) = self.rule_errors.get(kind) {
                        self.diagnostics.push(Diagnostic::from_span(
                            self.path,
                            self.source,
                            *span,
                            message.clone(),
                        ));
                    } else {
                        self.check_plural_branches(branches, *span, categories);
                    }
                    let linked_branches = branches
                        .iter()
                        .map(|branch| LinkedPluralBranch {
                            case: branch.case,
                            pattern: self.link(&branch.pattern, true),
                            span: branch.span,
                        })
                        .collect();
                    LinkedPatternPart::Plural {
                        slot,
                        kind: *kind,
                        rule_set_index: self
                            .rule_set_indices
                            .get(kind)
                            .copied()
                            .unwrap_or_default(),
                        branches: linked_branches,
                        span: *span,
                    }
                }
                PatternPart::Select {
                    name,
                    branches,
                    span,
                } => {
                    let slot = self.bind(name, Some(ArgumentType::String));
                    self.check_select_branches(branches);
                    let linked_branches = branches
                        .iter()
                        .map(|branch| LinkedSelectBranch {
                            value: branch.name.text.clone(),
                            pattern: self.link(&branch.pattern, inside_plural),
                            span: branch.span,
                        })
                        .collect();
                    LinkedPatternPart::Select {
                        slot,
                        branches: linked_branches,
                        span: *span,
                    }
                }
                PatternPart::Format {
                    name,
                    options,
                    span,
                } => {
                    let slot = match options {
                        FormatOptions::Decimal { .. } => self.bind_decimal(name),
                        FormatOptions::Currency { .. } => {
                            self.bind(name, Some(ArgumentType::Money))
                        }
                        FormatOptions::Date { .. } => self.bind(name, Some(ArgumentType::Date)),
                        FormatOptions::Time { .. } => self.bind(name, Some(ArgumentType::Time)),
                        FormatOptions::Instant { .. } => {
                            self.bind(name, Some(ArgumentType::Instant))
                        }
                    };
                    LinkedPatternPart::Format {
                        slot,
                        options: options.clone(),
                        span: *span,
                    }
                }
                PatternPart::Pound { span } => {
                    if !inside_plural {
                        self.diagnostics.push(Diagnostic::from_span(
                            self.path,
                            self.source,
                            *span,
                            "`#` is only valid inside a plural or ordinal branch".to_string(),
                        ));
                    }
                    LinkedPatternPart::Pound { span: *span }
                }
            })
            .collect();
        LinkedPattern {
            parts,
            span: pattern.span,
        }
    }

    fn bind(&mut self, name: &Identifier, expected_type: Option<ArgumentType>) -> u32 {
        let Some(binding) = self.parameters.get(&name.text) else {
            self.diagnostics.push(
                Diagnostic::from_span(
                    self.path,
                    self.source,
                    name.span,
                    format!("unknown schema parameter `{}`", name.text),
                )
                .with_related_span(
                    "<schema>",
                    self.schema_source,
                    self.schema_message_span,
                    "schema signature for this message",
                ),
            );
            return 0;
        };
        if let Some(expected_type) = expected_type
            && binding.argument_type != expected_type
        {
            let expected = Self::type_name(expected_type);
            self.diagnostics.push(
                Diagnostic::from_span(
                    self.path,
                    self.source,
                    name.span,
                    format!(
                        "selector `{}` has type `{}`; expected `{expected}`",
                        name.text,
                        Self::type_name(binding.argument_type)
                    ),
                )
                .with_related_span(
                    "<schema>",
                    self.schema_source,
                    binding.type_span,
                    "parameter type declared here",
                ),
            );
        }
        binding.slot
    }

    fn bind_plain(&mut self, name: &Identifier) -> u32 {
        let Some(binding) = self.parameters.get(&name.text) else {
            return self.bind(name, None);
        };
        if !matches!(
            binding.argument_type,
            ArgumentType::String
                | ArgumentType::Number
                | ArgumentType::PluralCount
                | ArgumentType::PluralDecimal
                | ArgumentType::OrdinalCount
        ) {
            self.diagnostics.push(
                Diagnostic::from_span(
                    self.path,
                    self.source,
                    name.span,
                    format!(
                        "parameter `{}` of type `{}` requires a typed format operation",
                        name.text,
                        Self::type_name(binding.argument_type)
                    ),
                )
                .with_related_span(
                    "<schema>",
                    self.schema_source,
                    binding.type_span,
                    "parameter type declared here",
                ),
            );
        }
        binding.slot
    }

    fn bind_cardinal(&mut self, name: &Identifier) -> u32 {
        let Some(binding) = self.parameters.get(&name.text) else {
            return self.bind(name, None);
        };
        if !matches!(
            binding.argument_type,
            ArgumentType::PluralCount | ArgumentType::PluralDecimal
        ) {
            self.diagnostics.push(
                Diagnostic::from_span(
                    self.path,
                    self.source,
                    name.span,
                    format!(
                        "selector `{}` has type `{}`; expected `plural_count` or `plural_decimal`",
                        name.text,
                        Self::type_name(binding.argument_type)
                    ),
                )
                .with_related_span(
                    "<schema>",
                    self.schema_source,
                    binding.type_span,
                    "parameter type declared here",
                ),
            );
        }
        binding.slot
    }

    fn bind_decimal(&mut self, name: &Identifier) -> u32 {
        let Some(binding) = self.parameters.get(&name.text) else {
            return self.bind(name, None);
        };
        if !matches!(
            binding.argument_type,
            ArgumentType::Decimal | ArgumentType::PluralDecimal
        ) {
            self.diagnostics.push(
                Diagnostic::from_span(
                    self.path,
                    self.source,
                    name.span,
                    format!(
                        "format operation `{}` has type `{}`; expected `decimal` or `plural_decimal`",
                        name.text,
                        Self::type_name(binding.argument_type)
                    ),
                )
                .with_related_span(
                    "<schema>",
                    self.schema_source,
                    binding.type_span,
                    "parameter type declared here",
                ),
            );
        }
        binding.slot
    }

    fn check_plural_branches(
        &mut self,
        branches: &[PluralBranch],
        span: Span,
        supported_categories: &[PluralCategory],
    ) {
        let mut categories: Vec<(PluralCategory, Span)> = Vec::new();
        let mut exact_values: BTreeMap<u64, Span> = BTreeMap::new();
        for branch in branches {
            match branch.case {
                PluralCase::Exact(value) => {
                    if let Some(first_span) = exact_values.get(&value).copied() {
                        self.diagnostics.push(
                            Diagnostic::from_span(
                                self.path,
                                self.source,
                                branch.case_span,
                                format!("exact plural case `={value}` is repeated"),
                            )
                            .with_related_span(
                                self.path,
                                self.source,
                                first_span,
                                "first exact case with this value",
                            ),
                        );
                    } else {
                        exact_values.insert(value, branch.case_span);
                    }
                }
                PluralCase::Category(category) => {
                    if let Some((_, first_span)) =
                        categories.iter().find(|(found, _)| *found == category)
                    {
                        self.diagnostics.push(
                            Diagnostic::from_span(
                                self.path,
                                self.source,
                                branch.case_span,
                                format!(
                                    "plural category `{}` is repeated",
                                    Self::category_name(category)
                                ),
                            )
                            .with_related_span(
                                self.path,
                                self.source,
                                *first_span,
                                "first branch with this category",
                            ),
                        );
                    } else {
                        categories.push((category, branch.case_span));
                    }
                    if !supported_categories.contains(&category) {
                        self.diagnostics.push(Diagnostic::from_span(
                            self.path,
                            self.source,
                            branch.case_span,
                            format!(
                                "plural category `{}` is unsupported for this locale",
                                Self::category_name(category)
                            ),
                        ));
                    }
                }
            }
        }
        for category in supported_categories {
            if !categories.iter().any(|(found, _)| found == category) {
                self.diagnostics.push(Diagnostic::from_span(
                    self.path,
                    self.source,
                    span,
                    format!(
                        "plural expression is missing category `{}`",
                        Self::category_name(*category)
                    ),
                ));
            }
        }
    }

    fn check_select_branches(&mut self, branches: &[SelectBranch]) {
        let mut cases: BTreeMap<&str, Span> = BTreeMap::new();
        for branch in branches {
            if let Some(first_span) = cases.get(branch.name.text.as_str()).copied() {
                self.diagnostics.push(
                    Diagnostic::from_span(
                        self.path,
                        self.source,
                        branch.name.span,
                        format!("select case `{}` is repeated", branch.name.text),
                    )
                    .with_related_span(
                        self.path,
                        self.source,
                        first_span,
                        "first branch with this case",
                    ),
                );
            } else {
                cases.insert(branch.name.text.as_str(), branch.name.span);
            }
        }
        if !cases.contains_key("other")
            && let Some(branch) = branches.first()
        {
            self.diagnostics.push(Diagnostic::from_span(
                self.path,
                self.source,
                branch.span,
                "select expression is missing the mandatory `other` case".to_string(),
            ));
        }
    }

    fn type_name(argument_type: ArgumentType) -> &'static str {
        match argument_type {
            ArgumentType::PluralCount => "plural_count",
            ArgumentType::PluralDecimal => "plural_decimal",
            ArgumentType::OrdinalCount => "ordinal_count",
            ArgumentType::String => "string",
            ArgumentType::Number => "number",
            ArgumentType::Decimal => "decimal",
            ArgumentType::Money => "money",
            ArgumentType::Date => "date",
            ArgumentType::Time => "time",
            ArgumentType::Instant => "instant",
        }
    }

    fn category_name(category: PluralCategory) -> &'static str {
        match category {
            PluralCategory::Zero => "zero",
            PluralCategory::One => "one",
            PluralCategory::Two => "two",
            PluralCategory::Few => "few",
            PluralCategory::Many => "many",
            PluralCategory::Other => "other",
        }
    }
}
