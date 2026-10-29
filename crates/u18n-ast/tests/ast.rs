use u18n_ast::{
    ArgumentType, FileId, Identifier, LocaleAst, LocaleId, LocalizedMessage, MessageDef,
    MessageKey, ParameterDef, Pattern, PatternPart, PluralBranch, PluralCase, PluralCategory,
    PluralKind, SchemaAst, SchemaVersion, Span,
};

fn span(start: u32, end: u32) -> Span {
    Span {
        file_id: FileId(7),
        start,
        end,
    }
}

fn identifier(text: &str, start: u32, end: u32) -> Identifier {
    Identifier {
        text: String::from(text),
        span: span(start, end),
    }
}

#[test]
fn schema_ast_keeps_parameter_order_and_source_spans() {
    let schema = SchemaAst {
        version: SchemaVersion::V1,
        messages: vec![MessageDef {
            key: MessageKey {
                segments: vec![identifier("cart", 8, 12), identifier("items", 13, 18)],
                span: span(8, 18),
            },
            parameters: vec![
                ParameterDef {
                    name: identifier("count", 19, 24),
                    argument_type: ArgumentType::PluralCount,
                    type_span: span(26, 38),
                    documentation: Vec::new(),
                    span: span(19, 38),
                },
                ParameterDef {
                    name: identifier("label", 40, 45),
                    argument_type: ArgumentType::String,
                    type_span: span(47, 53),
                    documentation: Vec::new(),
                    span: span(40, 53),
                },
            ],
            documentation: Vec::new(),
            span: span(0, 55),
        }],
        span: span(0, 55),
    };

    assert!(matches!(
        schema.messages.as_slice(),
        [MessageDef { parameters, .. }]
            if matches!(
                parameters.as_slice(),
                [
                    ParameterDef { argument_type: ArgumentType::PluralCount, .. },
                    ParameterDef { argument_type: ArgumentType::String, .. }
                ]
            )
    ));
    assert_eq!(schema.span, span(0, 55));
}

#[test]
fn locale_ast_keeps_plural_tokens_and_recursive_patterns() {
    let locale = LocaleAst {
        locale: LocaleId {
            source_tag: String::from("en"),
            span: span(7, 9),
        },
        fallback: None,
        locale_span: span(0, 10),
        messages: vec![LocalizedMessage {
            key: MessageKey {
                segments: vec![identifier("cart", 19, 23), identifier("items", 24, 29)],
                span: span(19, 29),
            },
            pattern: Pattern {
                parts: vec![
                    PatternPart::Text {
                        value: String::from("You have "),
                        span: span(33, 42),
                    },
                    PatternPart::LineBreak { span: span(42, 43) },
                    PatternPart::Plural {
                        name: identifier("count", 44, 49),
                        kind: PluralKind::Cardinal,
                        branches: vec![
                            PluralBranch {
                                case: PluralCase::Category(PluralCategory::One),
                                case_span: span(59, 62),
                                pattern: Pattern {
                                    parts: vec![PatternPart::Pound { span: span(65, 66) }],
                                    span: span(64, 72),
                                },
                                span: span(59, 73),
                            },
                            PluralBranch {
                                case: PluralCase::Category(PluralCategory::Other),
                                case_span: span(74, 79),
                                pattern: Pattern {
                                    parts: vec![
                                        PatternPart::Text {
                                            value: String::from("many items for "),
                                            span: span(82, 97),
                                        },
                                        PatternPart::Plural {
                                            name: identifier("days", 99, 103),
                                            kind: PluralKind::Cardinal,
                                            branches: vec![PluralBranch {
                                                case: PluralCase::Category(PluralCategory::Other),
                                                case_span: span(113, 118),
                                                pattern: Pattern {
                                                    parts: vec![PatternPart::Text {
                                                        value: String::from("days"),
                                                        span: span(121, 125),
                                                    }],
                                                    span: span(120, 126),
                                                },
                                                span: span(113, 127),
                                            }],
                                            span: span(98, 128),
                                        },
                                    ],
                                    span: span(81, 129),
                                },
                                span: span(74, 130),
                            },
                        ],
                        span: span(43, 131),
                    },
                ],
                span: span(32, 132),
            },
            span: span(11, 134),
        }],
        span: span(0, 135),
    };

    assert!(matches!(
        locale.messages.as_slice(),
        [LocalizedMessage {
            pattern: Pattern { parts, .. },
            ..
        }]
            if matches!(
                parts.as_slice(),
                [
                    PatternPart::Text { .. },
                    PatternPart::LineBreak { .. },
                    PatternPart::Plural { branches, .. }
                ]
                if matches!(
                    branches.as_slice(),
                    [
                        PluralBranch {
                            case: PluralCase::Category(PluralCategory::One),
                            pattern: Pattern { parts: one_parts, .. },
                            ..
                        },
                        PluralBranch {
                            case: PluralCase::Category(PluralCategory::Other),
                            pattern: Pattern { parts: other_parts, .. },
                            ..
                        }
                    ]
                    if matches!(one_parts.as_slice(), [PatternPart::Pound { .. }])
                        && matches!(
                            other_parts.as_slice(),
                            [PatternPart::Text { .. }, PatternPart::Plural { .. }]
                        )
                )
    )));
    assert_eq!(locale.locale.source_tag, "en");
}
