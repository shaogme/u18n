use std::collections::{BTreeMap, VecDeque};

use u18n_ast::{FormatOptions, PluralCase, PluralCategory, PluralKind};

use crate::{
    cldr::RuleSetData,
    semantic::{LinkedPattern, LinkedPatternPart, LinkedPluralBranch, LinkedSelectBranch},
};

use super::pool::StringPool;

pub(super) struct MessageSignatureData {
    pub(super) argument_count: u32,
}

pub(super) struct MessageProgramData {
    pub(super) instruction_start: u32,
    pub(super) instruction_end: u32,
}

pub(super) struct MessageBindingData {
    pub(super) message_id: u32,
    pub(super) program_id: u32,
}

pub(super) enum InstructionData {
    Text { offset: u32, length: u32 },
    Argument { index: u32 },
    Plural { index: u32 },
    Select { index: u32 },
    Format { index: u32 },
    Pound,
}

pub(super) struct PluralNodeData {
    pub(super) argument_index: u32,
    pub(super) kind: PluralKind,
    pub(super) rule_set_index: u32,
    pub(super) branch_start: u32,
    pub(super) branch_end: u32,
}

pub(super) struct FormatNodeData {
    pub(super) argument_index: u32,
    pub(super) options: FormatOptions,
}

pub(super) struct BranchData {
    pub(super) selector: PluralCase,
    pub(super) instruction_start: u32,
    pub(super) instruction_end: u32,
}

pub(super) struct SelectNodeData {
    pub(super) argument_index: u32,
    pub(super) branch_start: u32,
    pub(super) branch_end: u32,
}

pub(super) struct SelectBranchData {
    pub(super) value_offset: u32,
    pub(super) value_length: u32,
    pub(super) instruction_start: u32,
    pub(super) instruction_end: u32,
}

pub(super) struct LoweredProgramStore {
    pub(super) message_signatures: Vec<MessageSignatureData>,
    pub(super) programs: Vec<MessageProgramData>,
    pub(super) instructions: Vec<InstructionData>,
    pub(super) plurals: Vec<PluralNodeData>,
    pub(super) selects: Vec<SelectNodeData>,
    pub(super) branches: Vec<BranchData>,
    pub(super) select_branches: Vec<SelectBranchData>,
    pub(super) formats: Vec<FormatNodeData>,
    pub(super) rules: Vec<RuleSetData>,
}

pub(super) struct LoweredLocale {
    pub(super) locale_index: u32,
    pub(super) module: String,
    pub(super) locale_tag: String,
    pub(super) fallback_module: Option<String>,
    pub(super) bindings: Vec<MessageBindingData>,
}

pub(super) struct LocaleProgramData {
    pub(super) locale_tag: String,
    pub(super) program_id: u32,
}

pub(super) struct LoweredMessageShard {
    pub(super) message_id: u32,
    pub(super) argument_count: u32,
    pub(super) locale_programs: Vec<LocaleProgramData>,
    pub(super) store: LoweredProgramStore,
    pub(super) text_pool: String,
    pub(super) literal_fragments: usize,
    pub(super) overlap_optimization_enabled: bool,
}

enum SelectorTask<'ast> {
    Plural(usize, Vec<&'ast LinkedPluralBranch>),
    Select(usize, Vec<&'ast LinkedSelectBranch>),
}

pub(super) struct LowerBuilder<'ast> {
    rule_set_indices: BTreeMap<u32, u32>,
    instructions: Vec<InstructionData>,
    plurals: Vec<PluralNodeData>,
    selects: Vec<SelectNodeData>,
    branches: Vec<BranchData>,
    select_branches: Vec<SelectBranchData>,
    formats: Vec<FormatNodeData>,
    tasks: VecDeque<SelectorTask<'ast>>,
}

impl<'ast> LowerBuilder<'ast> {
    pub(super) fn new(rule_set_indices: BTreeMap<u32, u32>) -> Self {
        Self {
            rule_set_indices,
            instructions: Vec::new(),
            plurals: Vec::new(),
            selects: Vec::new(),
            branches: Vec::new(),
            select_branches: Vec::new(),
            formats: Vec::new(),
            tasks: VecDeque::new(),
        }
    }

    pub(super) fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    pub(super) fn finish(
        self,
        message_signatures: Vec<MessageSignatureData>,
        programs: Vec<MessageProgramData>,
        rules: Vec<RuleSetData>,
    ) -> LoweredProgramStore {
        LoweredProgramStore {
            message_signatures,
            programs,
            instructions: self.instructions,
            plurals: self.plurals,
            selects: self.selects,
            branches: self.branches,
            select_branches: self.select_branches,
            formats: self.formats,
            rules,
        }
    }

    pub(super) fn compile_pattern(
        &mut self,
        pattern: &'ast LinkedPattern,
        pool: &StringPool,
    ) -> Result<(), String> {
        let _pattern_span = pattern.span;
        for part in &pattern.parts {
            match part {
                LinkedPatternPart::Text { value, span } => {
                    let _source_span = span;
                    if let Some((offset, length)) = pool.reference(value) {
                        self.instructions
                            .push(InstructionData::Text { offset, length });
                    }
                }
                LinkedPatternPart::Argument { slot, span } => {
                    let _source_span = span;
                    self.instructions
                        .push(InstructionData::Argument { index: *slot });
                }
                LinkedPatternPart::Pound { span } => {
                    let _source_span = span;
                    self.instructions.push(InstructionData::Pound);
                }
                LinkedPatternPart::Plural {
                    slot,
                    kind,
                    rule_set_index,
                    branches,
                    span,
                } => {
                    let _source_span = span;
                    let local_rule_set_index = match self.rule_set_indices.get(rule_set_index) {
                        Some(index) => *index,
                        None => {
                            return Err(
                                "linked plural references a rule set not used by this catalog"
                                    .to_string(),
                            );
                        }
                    };
                    let plural_index = self.plurals.len();
                    self.plurals.push(PluralNodeData {
                        argument_index: *slot,
                        kind: *kind,
                        rule_set_index: local_rule_set_index,
                        branch_start: 0,
                        branch_end: 0,
                    });
                    self.instructions.push(InstructionData::Plural {
                        index: Self::checked_index(plural_index)?,
                    });
                    let mut ordered = branches.iter().collect::<Vec<_>>();
                    ordered.sort_by_key(|branch| Self::case_rank(branch.case));
                    self.tasks
                        .push_back(SelectorTask::Plural(plural_index, ordered));
                }
                LinkedPatternPart::Select {
                    slot,
                    branches,
                    span,
                } => {
                    let _source_span = span;
                    let select_index = self.selects.len();
                    self.selects.push(SelectNodeData {
                        argument_index: *slot,
                        branch_start: 0,
                        branch_end: 0,
                    });
                    self.instructions.push(InstructionData::Select {
                        index: Self::checked_index(select_index)?,
                    });
                    let mut ordered = branches.iter().collect::<Vec<_>>();
                    ordered.sort_by(|left, right| left.value.cmp(&right.value));
                    self.tasks
                        .push_back(SelectorTask::Select(select_index, ordered));
                }
                LinkedPatternPart::Format {
                    slot,
                    options,
                    span,
                } => {
                    let _source_span = span;
                    let format_index = self.formats.len();
                    self.formats.push(FormatNodeData {
                        argument_index: *slot,
                        options: options.clone(),
                    });
                    self.instructions.push(InstructionData::Format {
                        index: Self::checked_index(format_index)?,
                    });
                }
            }
        }
        Ok(())
    }

    pub(super) fn drain_selector_tasks(&mut self, pool: &StringPool) -> Result<(), String> {
        while let Some(task) = self.tasks.pop_front() {
            match task {
                SelectorTask::Plural(index, branches) => {
                    let branch_start = Self::checked_index(self.branches.len())?;
                    for branch in branches {
                        let _source_span = branch.span;
                        let instruction_start = Self::checked_index(self.instructions.len())?;
                        self.compile_pattern(&branch.pattern, pool)?;
                        let instruction_end = Self::checked_index(self.instructions.len())?;
                        self.branches.push(BranchData {
                            selector: branch.case,
                            instruction_start,
                            instruction_end,
                        });
                    }
                    let branch_end = Self::checked_index(self.branches.len())?;
                    let node = self
                        .plurals
                        .get_mut(index)
                        .ok_or_else(|| "plural lowering node disappeared".to_string())?;
                    node.branch_start = branch_start;
                    node.branch_end = branch_end;
                }
                SelectorTask::Select(index, branches) => {
                    let branch_start = Self::checked_index(self.select_branches.len())?;
                    for branch in branches {
                        let _source_span = branch.span;
                        let (value_offset, value_length) =
                            pool.reference(&branch.value).ok_or_else(|| {
                                format!(
                                    "select case `{}` is absent from the string pool",
                                    branch.value
                                )
                            })?;
                        let instruction_start = Self::checked_index(self.instructions.len())?;
                        self.compile_pattern(&branch.pattern, pool)?;
                        let instruction_end = Self::checked_index(self.instructions.len())?;
                        self.select_branches.push(SelectBranchData {
                            value_offset,
                            value_length,
                            instruction_start,
                            instruction_end,
                        });
                    }
                    let branch_end = Self::checked_index(self.select_branches.len())?;
                    let node = self
                        .selects
                        .get_mut(index)
                        .ok_or_else(|| "select lowering node disappeared".to_string())?;
                    node.branch_start = branch_start;
                    node.branch_end = branch_end;
                }
            }
        }
        Ok(())
    }

    fn checked_index(value: usize) -> Result<u32, String> {
        u32::try_from(value).map_err(|_| "generated table exceeds the u32 index limit".to_string())
    }

    fn case_rank(case: PluralCase) -> (u8, u64) {
        match case {
            PluralCase::Exact(value) => (0, value),
            PluralCase::Category(category) => (1 + Self::category_rank(category), 0),
        }
    }

    fn category_rank(category: PluralCategory) -> u8 {
        match category {
            PluralCategory::Zero => 0,
            PluralCategory::One => 1,
            PluralCategory::Two => 2,
            PluralCategory::Few => 3,
            PluralCategory::Many => 4,
            PluralCategory::Other => 5,
        }
    }
}
