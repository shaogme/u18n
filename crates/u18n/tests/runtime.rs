use core::fmt::{Error as FmtError, Result as FmtResult, Write};

use u18n::{
    ArgumentValue, Catalog, CatalogData, Decimal, FormatNode, FormatOptions, FormatRequest,
    FormatStyle, FormatValue, Formatter, FormatterError, Grouping, Instruction, Locale,
    LocaleProgram, MessageBinding, MessageData, MessageProgram, MessageShardData, MessageSignature,
    Money, PluralBranchRecord, PluralCategory, PluralConjunction, PluralCount, PluralDecimal,
    PluralKind, PluralNode, PluralNumber, PluralOperand, PluralPredicate, PluralRange, PluralRule,
    PluralSelector, Precision, RenderError, Rounding, RuleSet, SelectBranchRecord, SelectNode,
    SharedProgramStore,
};

fn shared_store(store: SharedProgramStore) -> &'static SharedProgramStore {
    Box::leak(Box::new(store))
}

struct LocaleFormatter;

impl Formatter for LocaleFormatter {
    fn format(
        &self,
        request: FormatRequest<'_>,
        output: &mut dyn Write,
    ) -> Result<(), FormatterError> {
        match (request.value, request.options) {
            (FormatValue::Decimal(value), FormatOptions::Decimal { .. }) => output
                .write_str(&format!("{}:{}", request.locale, value.coefficient()))
                .map_err(|_| FormatterError::Failed),
            _ => Err(FormatterError::Unsupported),
        }
    }
}

struct PartialFailureSink {
    value: String,
}

static KEYED_LOCALE_PROGRAMS: [LocaleProgram; 1] = [LocaleProgram {
    locale_tag: "en",
    program_id: 0,
}];
static KEYED_PROGRAMS: [MessageProgram; 1] = [MessageProgram {
    instruction_start: 0,
    instruction_end: 1,
}];
static KEYED_INSTRUCTIONS: [Instruction; 1] = [Instruction::Text {
    offset: 0,
    length: 5,
}];
static KEYED_MESSAGE: MessageData = MessageData::new_trusted(MessageShardData {
    argument_count: 0,
    locale_programs: &KEYED_LOCALE_PROGRAMS,
    text_pool: "Hello",
    programs: &KEYED_PROGRAMS,
    instructions: &KEYED_INSTRUCTIONS,
    plurals: &[],
    selects: &[],
    branches: &[],
    select_branches: &[],
    formats: &[],
    rule_sets: &[],
});

impl Write for PartialFailureSink {
    fn write_str(&mut self, value: &str) -> FmtResult {
        self.value
            .push_str(value.get(..value.len().min(3)).unwrap_or(value));
        Err(FmtError)
    }
}

static ONE_RULE_SET: [RuleSet; 1] = [RuleSet {
    kind: PluralKind::Cardinal,
    rules: &[PluralRule {
        category: PluralCategory::One,
        alternatives: &[PluralConjunction {
            predicates: &[PluralPredicate {
                operand: PluralOperand::N,
                modulus: None,
                ranges: &[PluralRange {
                    start: PluralNumber {
                        coefficient: 1,
                        scale: 0,
                    },
                    end: PluralNumber {
                        coefficient: 1,
                        scale: 0,
                    },
                }],
                negated: false,
                within: true,
            }],
        }],
    }],
}];
static DECIMAL_RULE_SET: [RuleSet; 1] = [RuleSet {
    kind: PluralKind::Cardinal,
    rules: &[
        PluralRule {
            category: PluralCategory::One,
            alternatives: &[PluralConjunction {
                predicates: &[
                    PluralPredicate {
                        operand: PluralOperand::N,
                        modulus: None,
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                            end: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                    PluralPredicate {
                        operand: PluralOperand::V,
                        modulus: None,
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                            end: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                ],
            }],
        },
        PluralRule {
            category: PluralCategory::Few,
            alternatives: &[PluralConjunction {
                predicates: &[
                    PluralPredicate {
                        operand: PluralOperand::I,
                        modulus: None,
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                            end: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                    PluralPredicate {
                        operand: PluralOperand::V,
                        modulus: None,
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 2,
                                scale: 0,
                            },
                            end: PluralNumber {
                                coefficient: 2,
                                scale: 0,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                    PluralPredicate {
                        operand: PluralOperand::W,
                        modulus: None,
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                            end: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                    PluralPredicate {
                        operand: PluralOperand::F,
                        modulus: None,
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 50,
                                scale: 0,
                            },
                            end: PluralNumber {
                                coefficient: 50,
                                scale: 0,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                    PluralPredicate {
                        operand: PluralOperand::T,
                        modulus: None,
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 5,
                                scale: 0,
                            },
                            end: PluralNumber {
                                coefficient: 5,
                                scale: 0,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                    PluralPredicate {
                        operand: PluralOperand::I,
                        modulus: None,
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                            end: PluralNumber {
                                coefficient: 1,
                                scale: 0,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                ],
            }],
        },
        PluralRule {
            category: PluralCategory::Many,
            alternatives: &[PluralConjunction {
                predicates: &[PluralPredicate {
                    operand: PluralOperand::N,
                    modulus: None,
                    ranges: &[PluralRange {
                        start: PluralNumber {
                            coefficient: 15,
                            scale: 1,
                        },
                        end: PluralNumber {
                            coefficient: 15,
                            scale: 1,
                        },
                    }],
                    negated: false,
                    within: true,
                }],
            }],
        },
    ],
}];
static DECIMAL_MODULO_RULE_SET: [RuleSet; 1] = [RuleSet {
    kind: PluralKind::Cardinal,
    rules: &[
        PluralRule {
            category: PluralCategory::Zero,
            alternatives: &[PluralConjunction {
                predicates: &[PluralPredicate {
                    operand: PluralOperand::N,
                    modulus: None,
                    ranges: &[PluralRange {
                        start: PluralNumber {
                            coefficient: 15,
                            scale: 1,
                        },
                        end: PluralNumber {
                            coefficient: 15,
                            scale: 1,
                        },
                    }],
                    negated: false,
                    within: false,
                }],
            }],
        },
        PluralRule {
            category: PluralCategory::One,
            alternatives: &[PluralConjunction {
                predicates: &[
                    PluralPredicate {
                        operand: PluralOperand::N,
                        modulus: Some(10),
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 15,
                                scale: 1,
                            },
                            end: PluralNumber {
                                coefficient: 15,
                                scale: 1,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                    PluralPredicate {
                        operand: PluralOperand::N,
                        modulus: None,
                        ranges: &[PluralRange {
                            start: PluralNumber {
                                coefficient: 11,
                                scale: 0,
                            },
                            end: PluralNumber {
                                coefficient: 12,
                                scale: 0,
                            },
                        }],
                        negated: false,
                        within: true,
                    },
                ],
            }],
        },
    ],
}];
static TEXT_INSTRUCTIONS: [Instruction; 3] = [
    Instruction::Text {
        offset: 0,
        length: 7,
    },
    Instruction::Argument { argument_index: 0 },
    Instruction::Text {
        offset: 7,
        length: 1,
    },
];
static TEXT_MESSAGES_SIGNATURES: [MessageSignature; 1] = [MessageSignature { argument_count: 1 }];
static TEXT_MESSAGES_PROGRAMS: [MessageProgram; 1] = [MessageProgram {
    instruction_start: 0,
    instruction_end: 3,
}];
static TEXT_MESSAGES_BINDINGS: [MessageBinding; 1] = [MessageBinding {
    message_id: 0,
    program_id: 0,
}];
static EMPTY_PLURALS: [PluralNode; 0] = [];
static EMPTY_BRANCHES: [PluralBranchRecord; 0] = [];
static EMPTY_RULE_SETS: [RuleSet; 0] = [];
static SPACE_TIME_ZONE_FORMATS: [FormatNode; 1] = [FormatNode {
    argument_index: 0,
    options: FormatOptions::Instant {
        style: FormatStyle::Medium,
        time_zone: "America/New York",
    },
}];
static UNICODE_TIME_ZONE_FORMATS: [FormatNode; 1] = [FormatNode {
    argument_index: 0,
    options: FormatOptions::Instant {
        style: FormatStyle::Medium,
        time_zone: "日本",
    },
}];
static EMPTY_TIME_ZONE_FORMATS: [FormatNode; 1] = [FormatNode {
    argument_index: 0,
    options: FormatOptions::Instant {
        style: FormatStyle::Medium,
        time_zone: "",
    },
}];
static VALID_TIME_ZONE_FORMATS: [FormatNode; 1] = [FormatNode {
    argument_index: 0,
    options: FormatOptions::Instant {
        style: FormatStyle::Medium,
        time_zone: "America/New_York",
    },
}];
static PLURAL_INSTRUCTIONS: [Instruction; 5] = [
    Instruction::Plural { plural_index: 0 },
    Instruction::Text {
        offset: 0,
        length: 4,
    },
    Instruction::Pound,
    Instruction::Text {
        offset: 4,
        length: 5,
    },
    Instruction::Pound,
];
static PLURAL_MESSAGES_SIGNATURES: [MessageSignature; 1] = [MessageSignature { argument_count: 1 }];
static PLURAL_MESSAGES_PROGRAMS: [MessageProgram; 1] = [MessageProgram {
    instruction_start: 0,
    instruction_end: 1,
}];
static PLURAL_MESSAGES_BINDINGS: [MessageBinding; 1] = [MessageBinding {
    message_id: 0,
    program_id: 0,
}];
static PLURAL_NODES: [PluralNode; 1] = [PluralNode {
    argument_index: 0,
    kind: PluralKind::Cardinal,
    rule_set_index: 0,
    branch_start: 0,
    branch_end: 2,
}];
static PLURAL_BRANCHES: [PluralBranchRecord; 2] = [
    PluralBranchRecord {
        selector: PluralSelector::Category(PluralCategory::One),
        instruction_start: 1,
        instruction_end: 3,
    },
    PluralBranchRecord {
        selector: PluralSelector::Category(PluralCategory::Other),
        instruction_start: 3,
        instruction_end: 5,
    },
];
static NESTED_INSTRUCTIONS: [Instruction; 5] = [
    Instruction::Plural { plural_index: 0 },
    Instruction::Pound,
    Instruction::Plural { plural_index: 1 },
    Instruction::Pound,
    Instruction::Pound,
];
static NESTED_MESSAGES_SIGNATURES: [MessageSignature; 1] = [MessageSignature { argument_count: 2 }];
static NESTED_MESSAGES_PROGRAMS: [MessageProgram; 1] = [MessageProgram {
    instruction_start: 0,
    instruction_end: 1,
}];
static NESTED_MESSAGES_BINDINGS: [MessageBinding; 1] = [MessageBinding {
    message_id: 0,
    program_id: 0,
}];
static NESTED_PLURALS: [PluralNode; 2] = [
    PluralNode {
        argument_index: 0,
        kind: PluralKind::Cardinal,
        rule_set_index: 0,
        branch_start: 0,
        branch_end: 2,
    },
    PluralNode {
        argument_index: 1,
        kind: PluralKind::Cardinal,
        rule_set_index: 0,
        branch_start: 2,
        branch_end: 4,
    },
];
static NESTED_BRANCHES: [PluralBranchRecord; 4] = [
    PluralBranchRecord {
        selector: PluralSelector::Category(PluralCategory::One),
        instruction_start: 1,
        instruction_end: 2,
    },
    PluralBranchRecord {
        selector: PluralSelector::Category(PluralCategory::Other),
        instruction_start: 2,
        instruction_end: 3,
    },
    PluralBranchRecord {
        selector: PluralSelector::Category(PluralCategory::One),
        instruction_start: 3,
        instruction_end: 4,
    },
    PluralBranchRecord {
        selector: PluralSelector::Category(PluralCategory::Other),
        instruction_start: 4,
        instruction_end: 5,
    },
];
static CYCLIC_INSTRUCTIONS: [Instruction; 2] = [
    Instruction::Plural { plural_index: 0 },
    Instruction::Plural { plural_index: 0 },
];
static CYCLIC_MESSAGES_SIGNATURES: [MessageSignature; 1] = [MessageSignature { argument_count: 1 }];
static CYCLIC_MESSAGES_PROGRAMS: [MessageProgram; 1] = [MessageProgram {
    instruction_start: 0,
    instruction_end: 1,
}];
static CYCLIC_MESSAGES_BINDINGS: [MessageBinding; 1] = [MessageBinding {
    message_id: 0,
    program_id: 0,
}];
static CYCLIC_PLURALS: [PluralNode; 1] = [PluralNode {
    argument_index: 0,
    kind: PluralKind::Cardinal,
    rule_set_index: 0,
    branch_start: 0,
    branch_end: 1,
}];
static CYCLIC_BRANCHES: [PluralBranchRecord; 1] = [PluralBranchRecord {
    selector: PluralSelector::Category(PluralCategory::Other),
    instruction_start: 1,
    instruction_end: 2,
}];

fn catalog_with_deep_unselected_branch() -> CatalogData {
    let mut instructions = Vec::new();
    instructions.push(Instruction::Plural { plural_index: 0 });
    instructions.push(Instruction::Text {
        offset: 0,
        length: 1,
    });
    instructions.push(Instruction::Plural { plural_index: 1 });
    for plural_index in 2..=32 {
        instructions.push(Instruction::Plural { plural_index });
    }
    instructions.push(Instruction::Text {
        offset: 0,
        length: 1,
    });

    let mut plurals = Vec::new();
    plurals.push(PluralNode {
        argument_index: 0,
        kind: PluralKind::Cardinal,
        rule_set_index: 0,
        branch_start: 0,
        branch_end: 2,
    });
    for plural_index in 1..=32 {
        plurals.push(PluralNode {
            argument_index: 0,
            kind: PluralKind::Cardinal,
            rule_set_index: 0,
            branch_start: plural_index + 1,
            branch_end: plural_index + 2,
        });
    }

    let mut branches = vec![
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::One),
            instruction_start: 1,
            instruction_end: 2,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::Other),
            instruction_start: 2,
            instruction_end: 3,
        },
    ];
    for plural_index in 1..=32 {
        branches.push(PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::Other),
            instruction_start: plural_index + 2,
            instruction_end: plural_index + 3,
        });
    }

    CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: Box::leak(
            vec![MessageBinding {
                message_id: 0,
                program_id: 0,
            }]
            .into_boxed_slice(),
        ),
        shared_store: shared_store(SharedProgramStore {
            text_pool: "x",
            message_signatures: Box::leak(
                vec![MessageSignature { argument_count: 1 }].into_boxed_slice(),
            ),
            programs: Box::leak(
                vec![MessageProgram {
                    instruction_start: 0,
                    instruction_end: 1,
                }]
                .into_boxed_slice(),
            ),
            instructions: Box::leak(instructions.into_boxed_slice()),
            plurals: Box::leak(plurals.into_boxed_slice()),
            selects: &[],
            branches: Box::leak(branches.into_boxed_slice()),
            select_branches: &[],
            formats: &[],
            rule_sets: &ONE_RULE_SET,
        }),
    }
}

fn checked_catalog_with_rule_sets(rule_sets: &'static [RuleSet]) -> Result<Catalog, RenderError> {
    Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &[],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "",
            message_signatures: &[],
            programs: &[],
            instructions: &[],
            plurals: &[],
            selects: &[],
            branches: &[],
            select_branches: &[],
            formats: &[],
            rule_sets,
        }),
    })
}

fn checked_catalog_with_categories(categories: &[PluralCategory]) -> Result<Catalog, RenderError> {
    let rules: &'static [PluralRule] = Box::leak(
        categories
            .iter()
            .copied()
            .map(|category| PluralRule {
                category,
                alternatives: &[],
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let rule_sets: &'static [RuleSet] = Box::leak(
        vec![RuleSet {
            kind: PluralKind::Cardinal,
            rules,
        }]
        .into_boxed_slice(),
    );
    checked_catalog_with_rule_sets(rule_sets)
}

fn checked_catalog_with_predicate(
    range: PluralRange,
    modulus: Option<u64>,
) -> Result<Catalog, RenderError> {
    let ranges: &'static [PluralRange] = Box::leak(vec![range].into_boxed_slice());
    let predicates: &'static [PluralPredicate] = Box::leak(
        vec![PluralPredicate {
            operand: PluralOperand::N,
            modulus,
            ranges,
            negated: false,
            within: true,
        }]
        .into_boxed_slice(),
    );
    let alternatives: &'static [PluralConjunction] =
        Box::leak(vec![PluralConjunction { predicates }].into_boxed_slice());
    let rules: &'static [PluralRule] = Box::leak(
        vec![PluralRule {
            category: PluralCategory::One,
            alternatives,
        }]
        .into_boxed_slice(),
    );
    let rule_sets: &'static [RuleSet] = Box::leak(
        vec![RuleSet {
            kind: PluralKind::Cardinal,
            rules,
        }]
        .into_boxed_slice(),
    );
    checked_catalog_with_rule_sets(rule_sets)
}

#[test]
fn renders_text_and_schema_order_arguments() {
    let catalog = Catalog::new_trusted(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &TEXT_MESSAGES_BINDINGS,
        shared_store: shared_store(SharedProgramStore {
            text_pool: "Hello, !",
            message_signatures: &TEXT_MESSAGES_SIGNATURES,
            programs: &TEXT_MESSAGES_PROGRAMS,
            instructions: &TEXT_INSTRUCTIONS,
            plurals: &EMPTY_PLURALS,
            selects: &[],
            branches: &EMPTY_BRANCHES,
            select_branches: &[],
            formats: &[],
            rule_sets: &EMPTY_RULE_SETS,
        }),
    });

    assert_eq!(
        catalog.render(0, &[ArgumentValue::String("Ada")]),
        Ok(String::from("Hello, Ada!"))
    );

    let amount = 17;
    assert_eq!(
        catalog.render(0, &[ArgumentValue::Number(&amount)]),
        Ok(String::from("Hello, 17!"))
    );
    assert_eq!(
        catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(17))]),
        Ok(String::from("Hello, 17!"))
    );
}

#[test]
fn keyed_messages_check_locale_program_and_argument_references() {
    let locale = Locale::new_trusted(0, "en");
    assert_eq!(
        KEYED_MESSAGE.render(&locale, &[]),
        Ok(String::from("Hello"))
    );

    let mut output = String::new();
    KEYED_MESSAGE
        .render_to(&locale, &[], &mut output)
        .expect("keyed streaming render succeeds");
    assert_eq!(output, "Hello");
    assert_eq!(
        KEYED_MESSAGE.render(&Locale::new_trusted(0, "fr"), &[]),
        Err(RenderError::LocaleMismatch)
    );
    assert_eq!(
        KEYED_MESSAGE.render(&Locale::new_trusted(1, "fr"), &[]),
        Err(RenderError::LocaleOutOfRange)
    );
    assert_eq!(
        KEYED_MESSAGE.render(&locale, &[ArgumentValue::String("extra")]),
        Err(RenderError::ArgumentCountMismatch)
    );

    let invalid_program = MessageData::new_trusted(MessageShardData {
        argument_count: 0,
        locale_programs: &[LocaleProgram {
            locale_tag: "en",
            program_id: 1,
        }],
        text_pool: "Hello",
        programs: &KEYED_PROGRAMS,
        instructions: &KEYED_INSTRUCTIONS,
        plurals: &[],
        selects: &[],
        branches: &[],
        select_branches: &[],
        formats: &[],
        rule_sets: &[],
    });
    assert_eq!(
        invalid_program.render(&locale, &[]),
        Err(RenderError::InvalidCatalog)
    );

    let invalid_range = MessageData::new_trusted(MessageShardData {
        argument_count: 0,
        locale_programs: &KEYED_LOCALE_PROGRAMS,
        text_pool: "Hello",
        programs: &[MessageProgram {
            instruction_start: 0,
            instruction_end: 2,
        }],
        instructions: &KEYED_INSTRUCTIONS,
        plurals: &[],
        selects: &[],
        branches: &[],
        select_branches: &[],
        formats: &[],
        rule_sets: &[],
    });
    assert_eq!(
        invalid_range.render(&locale, &[]),
        Err(RenderError::InvalidCatalog)
    );
}

#[test]
fn checked_catalogs_accept_valid_tables() {
    let result = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &TEXT_MESSAGES_BINDINGS,
        shared_store: shared_store(SharedProgramStore {
            text_pool: "Hello, !",
            message_signatures: &TEXT_MESSAGES_SIGNATURES,
            programs: &TEXT_MESSAGES_PROGRAMS,
            instructions: &TEXT_INSTRUCTIONS,
            plurals: &EMPTY_PLURALS,
            selects: &[],
            branches: &EMPTY_BRANCHES,
            select_branches: &[],
            formats: &[],
            rule_sets: &EMPTY_RULE_SETS,
        }),
    });

    assert!(result.is_ok());
}

#[test]
fn checked_catalogs_reject_duplicate_and_other_plural_rule_categories() {
    assert_eq!(
        checked_catalog_with_categories(&[PluralCategory::One, PluralCategory::One]).err(),
        Some(RenderError::InvalidCatalog)
    );
    assert_eq!(
        checked_catalog_with_categories(&[PluralCategory::Other]).err(),
        Some(RenderError::InvalidCatalog)
    );
}

#[test]
fn checked_catalogs_reject_invalid_plural_ranges_and_moduli() {
    for (range, modulus) in [
        (
            PluralRange {
                start: PluralNumber {
                    coefficient: 2,
                    scale: 0,
                },
                end: PluralNumber {
                    coefficient: 1,
                    scale: 0,
                },
            },
            None,
        ),
        (
            PluralRange {
                start: PluralNumber {
                    coefficient: 1,
                    scale: 39,
                },
                end: PluralNumber {
                    coefficient: 1,
                    scale: 39,
                },
            },
            None,
        ),
        (
            PluralRange {
                start: PluralNumber {
                    coefficient: 0,
                    scale: 0,
                },
                end: PluralNumber {
                    coefficient: 1,
                    scale: 0,
                },
            },
            Some(0),
        ),
    ] {
        assert_eq!(
            checked_catalog_with_predicate(range, modulus).err(),
            Some(RenderError::InvalidCatalog)
        );
    }
}

#[test]
fn checked_catalogs_reject_incomplete_plural_category_branches() {
    let result = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &[MessageBinding {
            message_id: 0,
            program_id: 0,
        }],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "x",
            message_signatures: &[MessageSignature { argument_count: 1 }],
            programs: &[MessageProgram {
                instruction_start: 0,
                instruction_end: 1,
            }],
            instructions: &[
                Instruction::Plural { plural_index: 0 },
                Instruction::Text {
                    offset: 0,
                    length: 1,
                },
                Instruction::Text {
                    offset: 0,
                    length: 1,
                },
            ],
            plurals: &[PluralNode {
                argument_index: 0,
                kind: PluralKind::Cardinal,
                rule_set_index: 0,
                branch_start: 0,
                branch_end: 2,
            }],
            selects: &[],
            branches: &[
                PluralBranchRecord {
                    selector: PluralSelector::Category(PluralCategory::One),
                    instruction_start: 1,
                    instruction_end: 2,
                },
                PluralBranchRecord {
                    selector: PluralSelector::Category(PluralCategory::Other),
                    instruction_start: 2,
                    instruction_end: 3,
                },
            ],
            select_branches: &[],
            formats: &[],
            rule_sets: &DECIMAL_RULE_SET,
        }),
    });

    assert_eq!(result.err(), Some(RenderError::InvalidCatalog));
}

#[test]
fn selects_cardinal_branches_and_renders_pound() {
    let catalog = Catalog::new_trusted(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &PLURAL_MESSAGES_BINDINGS,
        shared_store: shared_store(SharedProgramStore {
            text_pool: "one many ",
            message_signatures: &PLURAL_MESSAGES_SIGNATURES,
            programs: &PLURAL_MESSAGES_PROGRAMS,
            instructions: &PLURAL_INSTRUCTIONS,
            plurals: &PLURAL_NODES,
            selects: &[],
            branches: &PLURAL_BRANCHES,
            select_branches: &[],
            formats: &[],
            rule_sets: &ONE_RULE_SET,
        }),
    });

    assert_eq!(
        catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(1))]),
        Ok(String::from("one 1"))
    );
    assert_eq!(
        catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(2))]),
        Ok(String::from("many 2"))
    );
}

#[test]
fn preserves_plural_decimal_scale_for_rules_interpolation_and_pound() {
    static MESSAGES_SIGNATURES: [MessageSignature; 3] = [
        MessageSignature { argument_count: 1 },
        MessageSignature { argument_count: 1 },
        MessageSignature { argument_count: 1 },
    ];
    static MESSAGES_PROGRAMS: [MessageProgram; 3] = [
        MessageProgram {
            instruction_start: 0,
            instruction_end: 1,
        },
        MessageProgram {
            instruction_start: 11,
            instruction_end: 12,
        },
        MessageProgram {
            instruction_start: 12,
            instruction_end: 13,
        },
    ];
    static MESSAGES_BINDINGS: [MessageBinding; 3] = [
        MessageBinding {
            message_id: 0,
            program_id: 0,
        },
        MessageBinding {
            message_id: 1,
            program_id: 1,
        },
        MessageBinding {
            message_id: 2,
            program_id: 2,
        },
    ];
    static INSTRUCTIONS: [Instruction; 13] = [
        Instruction::Plural { plural_index: 0 },
        Instruction::Text {
            offset: 0,
            length: 6,
        },
        Instruction::Pound,
        Instruction::Text {
            offset: 6,
            length: 4,
        },
        Instruction::Pound,
        Instruction::Text {
            offset: 10,
            length: 4,
        },
        Instruction::Pound,
        Instruction::Text {
            offset: 14,
            length: 5,
        },
        Instruction::Pound,
        Instruction::Text {
            offset: 19,
            length: 6,
        },
        Instruction::Pound,
        Instruction::Argument { argument_index: 0 },
        Instruction::Format { format_index: 0 },
    ];
    static PLURALS: [PluralNode; 1] = [PluralNode {
        argument_index: 0,
        kind: PluralKind::Cardinal,
        rule_set_index: 0,
        branch_start: 0,
        branch_end: 5,
    }];
    static BRANCHES: [PluralBranchRecord; 5] = [
        PluralBranchRecord {
            selector: PluralSelector::Exact(2),
            instruction_start: 1,
            instruction_end: 3,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::One),
            instruction_start: 3,
            instruction_end: 5,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::Few),
            instruction_start: 5,
            instruction_end: 7,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::Many),
            instruction_start: 7,
            instruction_end: 9,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::Other),
            instruction_start: 9,
            instruction_end: 11,
        },
    ];
    static FORMATS: [FormatNode; 1] = [FormatNode {
        argument_index: 0,
        options: FormatOptions::Decimal {
            grouping: Grouping::None,
            precision: Precision::Input,
            rounding: Rounding::HalfEven,
        },
    }];
    let catalog = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &MESSAGES_BINDINGS,
        shared_store: shared_store(SharedProgramStore {
            text_pool: "exact:one:few:many:other:",
            message_signatures: &MESSAGES_SIGNATURES,
            programs: &MESSAGES_PROGRAMS,
            instructions: &INSTRUCTIONS,
            plurals: &PLURALS,
            selects: &[],
            branches: &BRANCHES,
            select_branches: &[],
            formats: &FORMATS,
            rule_sets: &DECIMAL_RULE_SET,
        }),
    })
    .expect("valid decimal plural catalog");

    for (value, expected) in [
        (10, "one:1.0"),
        (100, "other:1.00"),
        (150, "few:1.50"),
        (15, "many:1.5"),
        (149, "other:1.49"),
        (200, "exact:2.00"),
    ] {
        let scale = if value == 10 || value == 15 { 1 } else { 2 };
        let value = PluralDecimal::new(value, scale).expect("valid plural decimal");
        assert_eq!(
            catalog.render(0, &[ArgumentValue::PluralDecimal(value)]),
            Ok(String::from(expected))
        );
    }

    let value = PluralDecimal::new(150, 2).expect("valid plural decimal");
    assert_eq!(
        catalog.render(1, &[ArgumentValue::PluralDecimal(value)]),
        Ok(String::from("1.50"))
    );
    assert_eq!(
        catalog.render_with(&LocaleFormatter, 2, &[ArgumentValue::PluralDecimal(value)]),
        Ok(String::from("en:150"))
    );
    assert_eq!(PluralDecimal::new(-1, 0), None);
    assert_eq!(PluralDecimal::new(1, 39), None);
}

#[test]
fn evaluates_exact_decimal_modulo_and_in_vs_within() {
    static MESSAGES_SIGNATURES: [MessageSignature; 1] = [MessageSignature { argument_count: 1 }];
    static MESSAGES_PROGRAMS: [MessageProgram; 1] = [MessageProgram {
        instruction_start: 0,
        instruction_end: 1,
    }];
    static MESSAGES_BINDINGS: [MessageBinding; 1] = [MessageBinding {
        message_id: 0,
        program_id: 0,
    }];
    static INSTRUCTIONS: [Instruction; 4] = [
        Instruction::Plural { plural_index: 0 },
        Instruction::Text {
            offset: 0,
            length: 4,
        },
        Instruction::Text {
            offset: 4,
            length: 3,
        },
        Instruction::Text {
            offset: 7,
            length: 5,
        },
    ];
    static PLURALS: [PluralNode; 1] = [PluralNode {
        argument_index: 0,
        kind: PluralKind::Cardinal,
        rule_set_index: 0,
        branch_start: 0,
        branch_end: 3,
    }];
    static BRANCHES: [PluralBranchRecord; 3] = [
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::Zero),
            instruction_start: 1,
            instruction_end: 2,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::One),
            instruction_start: 2,
            instruction_end: 3,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::Other),
            instruction_start: 3,
            instruction_end: 4,
        },
    ];
    let catalog = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &MESSAGES_BINDINGS,
        shared_store: shared_store(SharedProgramStore {
            text_pool: "zerooneother",
            message_signatures: &MESSAGES_SIGNATURES,
            programs: &MESSAGES_PROGRAMS,
            instructions: &INSTRUCTIONS,
            plurals: &PLURALS,
            selects: &[],
            branches: &BRANCHES,
            select_branches: &[],
            formats: &[],
            rule_sets: &DECIMAL_MODULO_RULE_SET,
        }),
    })
    .expect("valid decimal modulo catalog");
    let decimal = |coefficient| {
        ArgumentValue::PluralDecimal(
            PluralDecimal::new(coefficient, 1).expect("valid plural decimal"),
        )
    };

    assert_eq!(catalog.render(0, &[decimal(115)]), Ok(String::from("one")));
    assert_eq!(catalog.render(0, &[decimal(15)]), Ok(String::from("other")));
    let boundary =
        ArgumentValue::PluralDecimal(PluralDecimal::new(149, 2).expect("valid plural decimal"));
    assert_eq!(catalog.render(0, &[boundary]), Ok(String::from("other")));
}

#[test]
fn nested_pound_uses_the_innermost_selector() {
    let catalog = Catalog::new_trusted(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &NESTED_MESSAGES_BINDINGS,
        shared_store: shared_store(SharedProgramStore {
            text_pool: "",
            message_signatures: &NESTED_MESSAGES_SIGNATURES,
            programs: &NESTED_MESSAGES_PROGRAMS,
            instructions: &NESTED_INSTRUCTIONS,
            plurals: &NESTED_PLURALS,
            selects: &[],
            branches: &NESTED_BRANCHES,
            select_branches: &[],
            formats: &[],
            rule_sets: &ONE_RULE_SET,
        }),
    });

    assert_eq!(
        catalog.render(
            0,
            &[
                ArgumentValue::PluralCount(PluralCount::new(2)),
                ArgumentValue::PluralCount(PluralCount::new(1)),
            ],
        ),
        Ok(String::from("1"))
    );
}

#[test]
fn nested_decimal_pound_restores_the_outer_value_after_inner_selection() {
    static MESSAGES_SIGNATURES: [MessageSignature; 1] = [MessageSignature { argument_count: 2 }];
    static MESSAGES_PROGRAMS: [MessageProgram; 1] = [MessageProgram {
        instruction_start: 0,
        instruction_end: 1,
    }];
    static MESSAGES_BINDINGS: [MessageBinding; 1] = [MessageBinding {
        message_id: 0,
        program_id: 0,
    }];
    static INSTRUCTIONS: [Instruction; 7] = [
        Instruction::Plural { plural_index: 0 },
        Instruction::Pound,
        Instruction::Text {
            offset: 0,
            length: 1,
        },
        Instruction::Plural { plural_index: 1 },
        Instruction::Text {
            offset: 0,
            length: 1,
        },
        Instruction::Pound,
        Instruction::Pound,
    ];
    static PLURALS: [PluralNode; 2] = [
        PluralNode {
            argument_index: 0,
            kind: PluralKind::Cardinal,
            rule_set_index: 0,
            branch_start: 0,
            branch_end: 2,
        },
        PluralNode {
            argument_index: 1,
            kind: PluralKind::Cardinal,
            rule_set_index: 0,
            branch_start: 2,
            branch_end: 4,
        },
    ];
    static BRANCHES: [PluralBranchRecord; 4] = [
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::One),
            instruction_start: 1,
            instruction_end: 1,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::Other),
            instruction_start: 1,
            instruction_end: 6,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::One),
            instruction_start: 6,
            instruction_end: 7,
        },
        PluralBranchRecord {
            selector: PluralSelector::Category(PluralCategory::Other),
            instruction_start: 6,
            instruction_end: 7,
        },
    ];
    let catalog = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &MESSAGES_BINDINGS,
        shared_store: shared_store(SharedProgramStore {
            text_pool: " ",
            message_signatures: &MESSAGES_SIGNATURES,
            programs: &MESSAGES_PROGRAMS,
            instructions: &INSTRUCTIONS,
            plurals: &PLURALS,
            selects: &[],
            branches: &BRANCHES,
            select_branches: &[],
            formats: &[],
            rule_sets: &ONE_RULE_SET,
        }),
    })
    .expect("valid nested decimal plural catalog");
    let decimal = PluralDecimal::new(150, 2).expect("valid plural decimal");

    assert_eq!(
        catalog.render(
            0,
            &[
                ArgumentValue::PluralDecimal(decimal),
                ArgumentValue::PluralCount(PluralCount::new(1)),
            ],
        ),
        Ok(String::from("1.50 1 1.50"))
    );
}

#[test]
fn rejects_runtime_catalogs_that_exceed_plural_nesting_limit() {
    let store = shared_store(SharedProgramStore {
        text_pool: "",
        message_signatures: &CYCLIC_MESSAGES_SIGNATURES,
        programs: &CYCLIC_MESSAGES_PROGRAMS,
        instructions: &CYCLIC_INSTRUCTIONS,
        plurals: &CYCLIC_PLURALS,
        selects: &[],
        branches: &CYCLIC_BRANCHES,
        select_branches: &[],
        formats: &[],
        rule_sets: &ONE_RULE_SET,
    });
    let catalog = Catalog::new_trusted(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &CYCLIC_MESSAGES_BINDINGS,
        shared_store: store,
    });
    let message = MessageData::new_trusted(MessageShardData {
        argument_count: 1,
        locale_programs: &[LocaleProgram {
            locale_tag: "en",
            program_id: 0,
        }],
        text_pool: store.text_pool,
        programs: store.programs,
        instructions: store.instructions,
        plurals: store.plurals,
        selects: store.selects,
        branches: store.branches,
        select_branches: store.select_branches,
        formats: store.formats,
        rule_sets: store.rule_sets,
    });

    assert_eq!(
        catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(2))]),
        Err(RenderError::InvalidCatalog)
    );
    assert_eq!(
        message.render(
            &Locale::new_trusted(0, "en"),
            &[ArgumentValue::PluralCount(PluralCount::new(2))]
        ),
        Err(RenderError::InvalidCatalog)
    );
}

fn exact_catalog_data() -> CatalogData {
    CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &[MessageBinding {
            message_id: 0,
            program_id: 0,
        }],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "zeroexactoneother",
            message_signatures: &[MessageSignature { argument_count: 1 }],
            programs: &[MessageProgram {
                instruction_start: 0,
                instruction_end: 1,
            }],
            instructions: &[
                Instruction::Plural { plural_index: 0 },
                Instruction::Text {
                    offset: 0,
                    length: 4,
                },
                Instruction::Text {
                    offset: 4,
                    length: 5,
                },
                Instruction::Text {
                    offset: 9,
                    length: 3,
                },
                Instruction::Text {
                    offset: 12,
                    length: 5,
                },
            ],
            plurals: &[PluralNode {
                argument_index: 0,
                kind: PluralKind::Cardinal,
                rule_set_index: 0,
                branch_start: 0,
                branch_end: 4,
            }],
            selects: &[],
            branches: &[
                PluralBranchRecord {
                    selector: PluralSelector::Exact(0),
                    instruction_start: 1,
                    instruction_end: 2,
                },
                PluralBranchRecord {
                    selector: PluralSelector::Exact(1),
                    instruction_start: 2,
                    instruction_end: 3,
                },
                PluralBranchRecord {
                    selector: PluralSelector::Category(PluralCategory::One),
                    instruction_start: 3,
                    instruction_end: 4,
                },
                PluralBranchRecord {
                    selector: PluralSelector::Category(PluralCategory::Other),
                    instruction_start: 4,
                    instruction_end: 5,
                },
            ],
            select_branches: &[],
            formats: &[],
            rule_sets: &ONE_RULE_SET,
        }),
    }
}

fn select_catalog_data() -> CatalogData {
    CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &[MessageBinding {
            message_id: 0,
            program_id: 0,
        }],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "femaleShemaleHeotherThey",
            message_signatures: &[MessageSignature { argument_count: 1 }],
            programs: &[MessageProgram {
                instruction_start: 0,
                instruction_end: 1,
            }],
            instructions: &[
                Instruction::Select { select_index: 0 },
                Instruction::Text {
                    offset: 6,
                    length: 3,
                },
                Instruction::Text {
                    offset: 13,
                    length: 2,
                },
                Instruction::Text {
                    offset: 20,
                    length: 4,
                },
            ],
            plurals: &[],
            selects: &[SelectNode {
                argument_index: 0,
                branch_start: 0,
                branch_end: 3,
            }],
            branches: &[],
            select_branches: &[
                SelectBranchRecord {
                    value_offset: 0,
                    value_length: 6,
                    instruction_start: 1,
                    instruction_end: 2,
                },
                SelectBranchRecord {
                    value_offset: 9,
                    value_length: 4,
                    instruction_start: 2,
                    instruction_end: 3,
                },
                SelectBranchRecord {
                    value_offset: 15,
                    value_length: 5,
                    instruction_start: 3,
                    instruction_end: 4,
                },
            ],
            formats: &[],
            rule_sets: &[],
        }),
    }
}

#[test]
fn exact_plural_cases_precede_cardinal_categories() {
    let catalog = Catalog::try_new(exact_catalog_data()).expect("valid exact plural catalog");

    for (count, expected) in [(0, "zero"), (1, "exact"), (3, "other"), (1, "exact")] {
        assert_eq!(
            catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(count))]),
            Ok(String::from(expected))
        );
    }
}

#[test]
fn selects_exact_string_cases_and_uses_other_for_unmatched_values() {
    let catalog = Catalog::try_new(select_catalog_data()).expect("valid select catalog");

    assert_eq!(
        catalog.render(0, &[ArgumentValue::String("female")]),
        Ok(String::from("She"))
    );
    assert_eq!(
        catalog.render(0, &[ArgumentValue::String("male")]),
        Ok(String::from("He"))
    );
    assert_eq!(
        catalog.render(0, &[ArgumentValue::String("unknown")]),
        Ok(String::from("They"))
    );
    assert_eq!(
        catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(1))]),
        Err(RenderError::ArgumentTypeMismatch)
    );
}

#[test]
fn checked_catalogs_reject_pound_inside_top_level_select_cases() {
    let result = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &[MessageBinding {
            message_id: 0,
            program_id: 0,
        }],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "other",
            message_signatures: &[MessageSignature { argument_count: 1 }],
            programs: &[MessageProgram {
                instruction_start: 0,
                instruction_end: 1,
            }],
            instructions: &[Instruction::Select { select_index: 0 }, Instruction::Pound],
            plurals: &[],
            selects: &[SelectNode {
                argument_index: 0,
                branch_start: 0,
                branch_end: 1,
            }],
            branches: &[],
            select_branches: &[SelectBranchRecord {
                value_offset: 0,
                value_length: 5,
                instruction_start: 1,
                instruction_end: 2,
            }],
            formats: &[],
            rule_sets: &[],
        }),
    });

    assert_eq!(result.err(), Some(RenderError::InvalidCatalog));
}

#[test]
fn checked_catalogs_revalidate_shared_selects_in_each_pound_context() {
    let result = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &[MessageBinding {
            message_id: 0,
            program_id: 0,
        }],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "other",
            message_signatures: &[MessageSignature { argument_count: 2 }],
            programs: &[MessageProgram {
                instruction_start: 0,
                instruction_end: 2,
            }],
            instructions: &[
                Instruction::Plural { plural_index: 0 },
                Instruction::Select { select_index: 0 },
                Instruction::Pound,
            ],
            plurals: &[PluralNode {
                argument_index: 0,
                kind: PluralKind::Cardinal,
                rule_set_index: 0,
                branch_start: 0,
                branch_end: 2,
            }],
            selects: &[SelectNode {
                argument_index: 1,
                branch_start: 0,
                branch_end: 1,
            }],
            branches: &[
                PluralBranchRecord {
                    selector: PluralSelector::Category(PluralCategory::One),
                    instruction_start: 1,
                    instruction_end: 2,
                },
                PluralBranchRecord {
                    selector: PluralSelector::Category(PluralCategory::Other),
                    instruction_start: 1,
                    instruction_end: 2,
                },
            ],
            select_branches: &[SelectBranchRecord {
                value_offset: 0,
                value_length: 5,
                instruction_start: 2,
                instruction_end: 3,
            }],
            formats: &[],
            rule_sets: &ONE_RULE_SET,
        }),
    });

    assert_eq!(result.err(), Some(RenderError::InvalidCatalog));
}

#[test]
fn rejects_deep_nesting_in_an_unselected_plural_branch() {
    assert!(matches!(
        Catalog::try_new(catalog_with_deep_unselected_branch()),
        Err(RenderError::InvalidCatalog)
    ));
}

#[test]
fn rejects_invalid_message_and_selector_arguments() {
    let catalog = Catalog::new_trusted(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &PLURAL_MESSAGES_BINDINGS,
        shared_store: shared_store(SharedProgramStore {
            text_pool: "one many ",
            message_signatures: &PLURAL_MESSAGES_SIGNATURES,
            programs: &PLURAL_MESSAGES_PROGRAMS,
            instructions: &PLURAL_INSTRUCTIONS,
            plurals: &PLURAL_NODES,
            selects: &[],
            branches: &PLURAL_BRANCHES,
            select_branches: &[],
            formats: &[],
            rule_sets: &ONE_RULE_SET,
        }),
    });

    assert_eq!(catalog.render(1, &[]), Err(RenderError::MessageOutOfRange));
    assert_eq!(
        catalog.render(0, &[ArgumentValue::String("1")]),
        Err(RenderError::ArgumentTypeMismatch)
    );
    assert_eq!(
        catalog.render(0, &[]),
        Err(RenderError::ArgumentCountMismatch)
    );
}

#[test]
fn renders_formatted_values_to_a_caller_sink_and_distinguishes_failures() {
    let catalog = Catalog::try_new(CatalogData {
        locale_tag: "fr-CA",
        fallback: None,
        bindings: &[MessageBinding {
            message_id: 0,
            program_id: 0,
        }],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "",
            message_signatures: &[MessageSignature { argument_count: 1 }],
            programs: &[MessageProgram {
                instruction_start: 0,
                instruction_end: 1,
            }],
            instructions: &[Instruction::Format { format_index: 0 }],
            plurals: &[],
            selects: &[],
            branches: &[],
            select_branches: &[],
            formats: &[FormatNode {
                argument_index: 0,
                options: FormatOptions::Decimal {
                    grouping: Grouping::Auto,
                    precision: Precision::Fixed(2),
                    rounding: Rounding::HalfEven,
                },
            }],
            rule_sets: &[],
        }),
    })
    .expect("valid formatter catalog");
    let arguments = [ArgumentValue::Decimal(Decimal::new(12345, 2))];
    let formatter = LocaleFormatter;
    let mut output = String::new();

    catalog
        .render_with_to(&formatter, 0, &arguments, &mut output)
        .expect("formatter writes to caller sink");
    assert_eq!(output, "fr-CA:12345");

    let mut output = String::new();
    assert_eq!(
        catalog.render_to(0, &arguments, &mut output),
        Err(RenderError::FormatterRequired)
    );

    let mut partial = PartialFailureSink {
        value: String::new(),
    };
    assert_eq!(
        catalog.render_with_to(&formatter, 0, &arguments, &mut partial),
        Err(RenderError::WriteFailed)
    );
    assert_eq!(partial.value, "fr-");
}

#[test]
fn checked_catalogs_reject_malformed_time_zone_identifiers() {
    for formats in [
        &SPACE_TIME_ZONE_FORMATS,
        &UNICODE_TIME_ZONE_FORMATS,
        &EMPTY_TIME_ZONE_FORMATS,
    ] {
        let result = Catalog::try_new(CatalogData {
            locale_tag: "en",
            fallback: None,
            bindings: &[],
            shared_store: shared_store(SharedProgramStore {
                text_pool: "",
                message_signatures: &[],
                programs: &[],
                instructions: &[],
                plurals: &[],
                selects: &[],
                branches: &[],
                select_branches: &[],
                formats,
                rule_sets: &[],
            }),
        });

        assert_eq!(result.err(), Some(RenderError::InvalidCatalog));
    }

    let valid = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &[],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "",
            message_signatures: &[],
            programs: &[],
            instructions: &[],
            plurals: &[],
            selects: &[],
            branches: &[],
            select_branches: &[],
            formats: &VALID_TIME_ZONE_FORMATS,
            rule_sets: &[],
        }),
    });

    assert!(valid.is_ok());
}

#[test]
fn checked_catalogs_validate_every_table_and_trusted_catalogs_render_selected_paths() {
    let invalid_message_catalog = Catalog::new_trusted(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &[
            MessageBinding {
                message_id: 0,
                program_id: 0,
            },
            MessageBinding {
                message_id: 1,
                program_id: 1,
            },
        ],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "x",
            message_signatures: &[
                MessageSignature { argument_count: 0 },
                MessageSignature { argument_count: 0 },
            ],
            programs: &[
                MessageProgram {
                    instruction_start: 0,
                    instruction_end: 1,
                },
                MessageProgram {
                    instruction_start: 1,
                    instruction_end: 2,
                },
            ],
            instructions: &[
                Instruction::Text {
                    offset: 0,
                    length: 1,
                },
                Instruction::Text {
                    offset: 1,
                    length: 1,
                },
            ],
            plurals: &[],
            selects: &[],
            branches: &[],
            select_branches: &[],
            formats: &[],
            rule_sets: &[],
        }),
    });
    let invalid_branch_catalog = Catalog::new_trusted(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &[MessageBinding {
            message_id: 0,
            program_id: 0,
        }],
        shared_store: shared_store(SharedProgramStore {
            text_pool: "x",
            message_signatures: &[MessageSignature { argument_count: 1 }],
            programs: &[MessageProgram {
                instruction_start: 0,
                instruction_end: 1,
            }],
            instructions: &[
                Instruction::Plural { plural_index: 0 },
                Instruction::Text {
                    offset: 0,
                    length: 1,
                },
                Instruction::Text {
                    offset: 1,
                    length: 1,
                },
            ],
            plurals: &[PluralNode {
                argument_index: 0,
                kind: PluralKind::Cardinal,
                rule_set_index: 0,
                branch_start: 0,
                branch_end: 2,
            }],
            selects: &[],
            branches: &[
                PluralBranchRecord {
                    selector: PluralSelector::Category(PluralCategory::One),
                    instruction_start: 1,
                    instruction_end: 2,
                },
                PluralBranchRecord {
                    selector: PluralSelector::Category(PluralCategory::Other),
                    instruction_start: 2,
                    instruction_end: 3,
                },
            ],
            select_branches: &[],
            formats: &[],
            rule_sets: &ONE_RULE_SET,
        }),
    });

    assert_eq!(
        invalid_message_catalog.render(0, &[]),
        Ok(String::from("x"))
    );
    assert_eq!(
        invalid_branch_catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(1))]),
        Ok(String::from("x"))
    );
    assert!(matches!(
        Catalog::try_new(CatalogData {
            locale_tag: "en",
            fallback: None,
            bindings: &[
                MessageBinding {
                    message_id: 0,
                    program_id: 0
                },
                MessageBinding {
                    message_id: 1,
                    program_id: 1
                }
            ],
            shared_store: shared_store(SharedProgramStore {
                text_pool: "x",
                message_signatures: &[
                    MessageSignature { argument_count: 0 },
                    MessageSignature { argument_count: 0 }
                ],
                programs: &[
                    MessageProgram {
                        instruction_start: 0,
                        instruction_end: 1
                    },
                    MessageProgram {
                        instruction_start: 1,
                        instruction_end: 2
                    }
                ],
                instructions: &[
                    Instruction::Text {
                        offset: 0,
                        length: 1,
                    },
                    Instruction::Text {
                        offset: 1,
                        length: 1,
                    },
                ],
                plurals: &[],
                selects: &[],
                branches: &[],
                select_branches: &[],
                formats: &[],
                rule_sets: &[]
            }),
        }),
        Err(RenderError::InvalidCatalog)
    ));
    assert!(matches!(
        Catalog::try_new(CatalogData {
            locale_tag: "en",
            fallback: None,
            bindings: &[MessageBinding {
                message_id: 0,
                program_id: 0
            }],
            shared_store: shared_store(SharedProgramStore {
                text_pool: "x",
                message_signatures: &[MessageSignature { argument_count: 1 }],
                programs: &[MessageProgram {
                    instruction_start: 0,
                    instruction_end: 1
                }],
                instructions: &[
                    Instruction::Plural { plural_index: 0 },
                    Instruction::Text {
                        offset: 0,
                        length: 1,
                    },
                    Instruction::Text {
                        offset: 1,
                        length: 1,
                    },
                ],
                plurals: &[PluralNode {
                    argument_index: 0,
                    kind: PluralKind::Cardinal,
                    rule_set_index: 0,
                    branch_start: 0,
                    branch_end: 2,
                }],
                selects: &[],
                branches: &[
                    PluralBranchRecord {
                        selector: PluralSelector::Category(PluralCategory::One),
                        instruction_start: 1,
                        instruction_end: 2,
                    },
                    PluralBranchRecord {
                        selector: PluralSelector::Category(PluralCategory::Other),
                        instruction_start: 2,
                        instruction_end: 3,
                    },
                ],
                select_branches: &[],
                formats: &[],
                rule_sets: &ONE_RULE_SET
            }),
        }),
        Err(RenderError::InvalidCatalog)
    ));
}

#[test]
fn money_requires_three_uppercase_ascii_letters() {
    let amount = Decimal::new(125, 2);

    assert!(Money::new(amount, "USD").is_some());
    assert!(Money::new(amount, "US").is_none());
    assert!(Money::new(amount, "USDA").is_none());
    assert!(Money::new(amount, "Usd").is_none());
    assert!(Money::new(amount, "U$D").is_none());
    assert!(Money::new(amount, "éU").is_none());
}

static FALLBACK_SIGNATURES: [MessageSignature; 2] = [
    MessageSignature { argument_count: 1 },
    MessageSignature { argument_count: 1 },
];
static FALLBACK_PROGRAMS: [MessageProgram; 2] = [
    MessageProgram {
        instruction_start: 0,
        instruction_end: 1,
    },
    MessageProgram {
        instruction_start: 1,
        instruction_end: 2,
    },
];
static FALLBACK_BINDINGS: [MessageBinding; 2] = [
    MessageBinding {
        message_id: 0,
        program_id: 0,
    },
    MessageBinding {
        message_id: 1,
        program_id: 1,
    },
];
static FALLBACK_RULE_SETS: [RuleSet; 2] = [
    RuleSet {
        kind: PluralKind::Cardinal,
        rules: &[PluralRule {
            category: PluralCategory::One,
            alternatives: &[PluralConjunction {
                predicates: &[PluralPredicate {
                    operand: PluralOperand::N,
                    modulus: None,
                    ranges: &[PluralRange {
                        start: PluralNumber {
                            coefficient: 1,
                            scale: 0,
                        },
                        end: PluralNumber {
                            coefficient: 1,
                            scale: 0,
                        },
                    }],
                    negated: false,
                    within: true,
                }],
            }],
        }],
    },
    RuleSet {
        kind: PluralKind::Cardinal,
        rules: &[PluralRule {
            category: PluralCategory::One,
            alternatives: &[PluralConjunction {
                predicates: &[PluralPredicate {
                    operand: PluralOperand::N,
                    modulus: None,
                    ranges: &[PluralRange {
                        start: PluralNumber {
                            coefficient: 0,
                            scale: 0,
                        },
                        end: PluralNumber {
                            coefficient: 1,
                            scale: 0,
                        },
                    }],
                    negated: false,
                    within: true,
                }],
            }],
        }],
    },
];
static FALLBACK_INSTRUCTIONS: [Instruction; 4] = [
    Instruction::Plural { plural_index: 0 },
    Instruction::Format { format_index: 0 },
    Instruction::Text {
        offset: 0,
        length: 3,
    },
    Instruction::Text {
        offset: 3,
        length: 5,
    },
];
static FALLBACK_PLURALS: [PluralNode; 1] = [PluralNode {
    argument_index: 0,
    kind: PluralKind::Cardinal,
    rule_set_index: 0,
    branch_start: 0,
    branch_end: 2,
}];
static FALLBACK_BRANCHES: [PluralBranchRecord; 2] = [
    PluralBranchRecord {
        selector: PluralSelector::Category(PluralCategory::One),
        instruction_start: 2,
        instruction_end: 3,
    },
    PluralBranchRecord {
        selector: PluralSelector::Category(PluralCategory::Other),
        instruction_start: 3,
        instruction_end: 4,
    },
];
static FALLBACK_FORMATS: [FormatNode; 1] = [FormatNode {
    argument_index: 0,
    options: FormatOptions::Decimal {
        grouping: Grouping::None,
        precision: Precision::Input,
        rounding: Rounding::HalfEven,
    },
}];
static FALLBACK_SHARED_STORE: SharedProgramStore = SharedProgramStore {
    text_pool: "oneother",
    message_signatures: &FALLBACK_SIGNATURES,
    programs: &FALLBACK_PROGRAMS,
    instructions: &FALLBACK_INSTRUCTIONS,
    plurals: &FALLBACK_PLURALS,
    selects: &[],
    branches: &FALLBACK_BRANCHES,
    select_branches: &[],
    formats: &FALLBACK_FORMATS,
    rule_sets: &FALLBACK_RULE_SETS,
};
static NO_FALLBACK_BINDINGS: [MessageBinding; 0] = [];
static ENGLISH_FALLBACK_CATALOG: Catalog = Catalog::new_trusted(CatalogData {
    locale_tag: "en",
    fallback: None,
    bindings: &FALLBACK_BINDINGS,
    shared_store: &FALLBACK_SHARED_STORE,
});
static FRENCH_FALLBACK_CATALOG: Catalog = Catalog::new_trusted(CatalogData {
    locale_tag: "fr",
    fallback: Some(&ENGLISH_FALLBACK_CATALOG),
    bindings: &NO_FALLBACK_BINDINGS,
    shared_store: &FALLBACK_SHARED_STORE,
});
static CANADIAN_FRENCH_CATALOG: Catalog = Catalog::new_trusted(CatalogData {
    locale_tag: "fr-CA",
    fallback: Some(&FRENCH_FALLBACK_CATALOG),
    bindings: &NO_FALLBACK_BINDINGS,
    shared_store: &FALLBACK_SHARED_STORE,
});

#[test]
fn sparse_fallback_lookup_preserves_author_rules_and_selected_formatter_locale() {
    let catalog = Catalog::try_new(CatalogData {
        locale_tag: "fr-CA",
        fallback: Some(&FRENCH_FALLBACK_CATALOG),
        bindings: &NO_FALLBACK_BINDINGS,
        shared_store: &FALLBACK_SHARED_STORE,
    })
    .expect("complete acyclic fallback chain should validate");

    assert_eq!(
        catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(0))]),
        Ok(String::from("other"))
    );
    assert_eq!(
        catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(1))]),
        Ok(String::from("one"))
    );
    assert_eq!(
        CANADIAN_FRENCH_CATALOG.render(0, &[ArgumentValue::PluralCount(PluralCount::new(0))]),
        catalog.render(0, &[ArgumentValue::PluralCount(PluralCount::new(0))])
    );
    assert_eq!(
        catalog.render_with(
            &LocaleFormatter,
            1,
            &[ArgumentValue::Decimal(Decimal::new(1995, 2))]
        ),
        Ok(String::from("fr-CA:1995"))
    );
}

static EMPTY_SHARED_STORE: SharedProgramStore = SharedProgramStore {
    text_pool: "",
    message_signatures: &[],
    programs: &[],
    instructions: &[],
    plurals: &[],
    selects: &[],
    branches: &[],
    select_branches: &[],
    formats: &[],
    rule_sets: &[],
};

#[test]
fn checked_catalogs_validate_syntax_without_requiring_registry_membership() {
    let catalog = Catalog::try_new(CatalogData {
        locale_tag: "zz",
        fallback: None,
        bindings: &NO_FALLBACK_BINDINGS,
        shared_store: &EMPTY_SHARED_STORE,
    });
    assert!(catalog.is_ok());
}

static FALLBACK_CYCLE_A: Catalog = Catalog::new_trusted(CatalogData {
    locale_tag: "ca",
    fallback: Some(&FALLBACK_CYCLE_B),
    bindings: &NO_FALLBACK_BINDINGS,
    shared_store: &EMPTY_SHARED_STORE,
});
static FALLBACK_CYCLE_B: Catalog = Catalog::new_trusted(CatalogData {
    locale_tag: "cb",
    fallback: Some(&FALLBACK_CYCLE_A),
    bindings: &NO_FALLBACK_BINDINGS,
    shared_store: &EMPTY_SHARED_STORE,
});
static UNSORTED_BINDINGS: [MessageBinding; 2] = [
    MessageBinding {
        message_id: 1,
        program_id: 0,
    },
    MessageBinding {
        message_id: 0,
        program_id: 0,
    },
];

#[test]
fn checked_catalogs_reject_incomplete_bindings_and_fallback_cycles() {
    let cycle = Catalog::try_new(CatalogData {
        locale_tag: "cr",
        fallback: Some(&FALLBACK_CYCLE_A),
        bindings: &NO_FALLBACK_BINDINGS,
        shared_store: &EMPTY_SHARED_STORE,
    });
    assert_eq!(cycle.err(), Some(RenderError::InvalidCatalog));

    let unsorted = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &UNSORTED_BINDINGS,
        shared_store: &FALLBACK_SHARED_STORE,
    });
    assert_eq!(unsorted.err(), Some(RenderError::InvalidCatalog));

    let unresolved = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: None,
        bindings: &NO_FALLBACK_BINDINGS,
        shared_store: &FALLBACK_SHARED_STORE,
    });
    assert_eq!(unresolved.err(), Some(RenderError::InvalidCatalog));
}

static INVALID_TAG_CATALOG: Catalog = Catalog::new_trusted(CatalogData {
    locale_tag: "en-u-ca",
    fallback: None,
    bindings: &NO_FALLBACK_BINDINGS,
    shared_store: &EMPTY_SHARED_STORE,
});

#[test]
fn checked_catalogs_reject_invalid_tags_throughout_fallback_chains() {
    let invalid_root = Catalog::try_new(CatalogData {
        locale_tag: "en--US",
        fallback: None,
        bindings: &NO_FALLBACK_BINDINGS,
        shared_store: &EMPTY_SHARED_STORE,
    });
    assert_eq!(invalid_root.err(), Some(RenderError::InvalidCatalog));

    let invalid_fallback = Catalog::try_new(CatalogData {
        locale_tag: "en",
        fallback: Some(&INVALID_TAG_CATALOG),
        bindings: &NO_FALLBACK_BINDINGS,
        shared_store: &EMPTY_SHARED_STORE,
    });
    assert_eq!(invalid_fallback.err(), Some(RenderError::InvalidCatalog));
}
