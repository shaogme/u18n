use alloc::{vec, vec::Vec};
use core::{convert::TryFrom, ops::Range, ptr};
use u18n_locale_tag::ParsedTag;

use crate::{
    PluralCategory, RenderError,
    catalog::{
        Catalog, CatalogData, CurrencyPrecision, FormatNode, FormatOptions, Instruction,
        PluralBranchRecord, PluralSelector, Precision, SharedProgramStore,
    },
    plural::{RuleEvaluator, RuleSet},
};

pub(crate) const MAX_SELECTOR_NESTING: usize = 32;

pub(crate) struct InstructionRange {
    pub(crate) start: u32,
    pub(crate) end: u32,
}

pub(crate) struct CatalogValidator<'catalog> {
    store: &'catalog SharedProgramStore,
    states: Vec<u8>,
    depths: Vec<u8>,
    message_visits: Vec<usize>,
    select_states: Vec<u8>,
    select_depths: Vec<u8>,
    select_message_visits: Vec<[usize; 2]>,
}

impl<'catalog> CatalogValidator<'catalog> {
    pub(crate) fn validate(data: &'catalog CatalogData) -> Result<(), RenderError> {
        let mut chain: Vec<&CatalogData> = Vec::new();
        let mut current = Some(data);
        while let Some(view) = current {
            if ParsedTag::parse(view.locale_tag).is_err()
                || !ptr::eq(view.shared_store, data.shared_store)
                || chain.iter().any(|seen| ptr::eq(*seen, view))
            {
                return Err(RenderError::InvalidCatalog);
            }
            Self::validate_bindings(view, data.shared_store)?;
            chain.push(view);
            current = view.fallback.map(Catalog::data);
        }

        for start in 0..chain.len() {
            for message_id in 0..data.shared_store.message_signatures.len() {
                if !chain
                    .get(start..)
                    .ok_or(RenderError::InvalidCatalog)?
                    .iter()
                    .any(|view| {
                        u32::try_from(message_id).ok().is_some_and(|message_id| {
                            view.bindings
                                .binary_search_by_key(&message_id, |binding| binding.message_id)
                                .is_ok()
                        })
                    })
                {
                    return Err(RenderError::InvalidCatalog);
                }
            }
        }

        let store = data.shared_store;
        for format in store.formats {
            Self::validate_format(format)?;
        }
        let mut validator = Self {
            store,
            states: vec![0; store.plurals.len()],
            depths: vec![0; store.plurals.len()],
            message_visits: vec![usize::MAX; store.plurals.len()],
            select_states: vec![0; store.selects.len()],
            select_depths: vec![0; store.selects.len()],
            select_message_visits: vec![[usize::MAX; 2]; store.selects.len()],
        };
        for program in store.programs {
            let positions = Self::checked_range(
                InstructionRange {
                    start: program.instruction_start,
                    end: program.instruction_end,
                },
                store.instructions.len(),
            )?;
            for position in positions {
                match store.instructions.get(position).copied() {
                    Some(Instruction::Plural { plural_index }) => {
                        validator.validate_node(
                            usize::try_from(plural_index)
                                .map_err(|_| RenderError::InvalidCatalog)?,
                            1,
                        )?;
                    }
                    Some(Instruction::Select { select_index }) => {
                        validator.validate_select_node(
                            usize::try_from(select_index)
                                .map_err(|_| RenderError::InvalidCatalog)?,
                            1,
                        )?;
                    }
                    Some(Instruction::Format { format_index }) => {
                        Self::format_node(store, format_index)?;
                    }
                    _ => {}
                }
            }
        }
        for index in 0..store.plurals.len() {
            validator.validate_node(index, 1)?;
        }
        for index in 0..store.selects.len() {
            validator.validate_select_node(index, 1)?;
        }
        for rule_set in store.rule_sets {
            RuleEvaluator::validate(rule_set)?;
        }
        for view in &chain {
            for binding in view.bindings {
                let message_index =
                    usize::try_from(binding.message_id).map_err(|_| RenderError::InvalidCatalog)?;
                let signature = store
                    .message_signatures
                    .get(message_index)
                    .ok_or(RenderError::InvalidCatalog)?;
                let program_index =
                    usize::try_from(binding.program_id).map_err(|_| RenderError::InvalidCatalog)?;
                let program = store
                    .programs
                    .get(program_index)
                    .ok_or(RenderError::InvalidCatalog)?;
                validator.validate_message_range(
                    InstructionRange {
                        start: program.instruction_start,
                        end: program.instruction_end,
                    },
                    signature.argument_count,
                    false,
                    message_index,
                )?;
            }
        }
        Ok(())
    }

    fn validate_bindings(
        view: &CatalogData,
        store: &SharedProgramStore,
    ) -> Result<(), RenderError> {
        let mut previous = None;
        for binding in view.bindings {
            let message_index =
                usize::try_from(binding.message_id).map_err(|_| RenderError::InvalidCatalog)?;
            let program_index =
                usize::try_from(binding.program_id).map_err(|_| RenderError::InvalidCatalog)?;
            if previous.is_some_and(|message_id| message_id >= binding.message_id)
                || store.message_signatures.get(message_index).is_none()
                || store.programs.get(program_index).is_none()
            {
                return Err(RenderError::InvalidCatalog);
            }
            previous = Some(binding.message_id);
        }
        Ok(())
    }

    fn validate_node(&mut self, index: usize, current_depth: usize) -> Result<u8, RenderError> {
        if current_depth > MAX_SELECTOR_NESTING {
            return Err(RenderError::InvalidCatalog);
        }
        match self.states.get(index).copied() {
            Some(1) => return Err(RenderError::InvalidCatalog),
            Some(2) => {
                let depth = self
                    .depths
                    .get(index)
                    .copied()
                    .ok_or(RenderError::InvalidCatalog)?;
                if current_depth + usize::from(depth) - 1 > MAX_SELECTOR_NESTING {
                    return Err(RenderError::InvalidCatalog);
                }
                return Ok(depth);
            }
            Some(0) => {}
            _ => return Err(RenderError::InvalidCatalog),
        }
        let node = self
            .store
            .plurals
            .get(index)
            .copied()
            .ok_or(RenderError::InvalidCatalog)?;
        let rule_set_index =
            usize::try_from(node.rule_set_index).map_err(|_| RenderError::InvalidCatalog)?;
        let rule_set = self
            .store
            .rule_sets
            .get(rule_set_index)
            .ok_or(RenderError::InvalidCatalog)?;
        if rule_set.kind != node.kind {
            return Err(RenderError::InvalidCatalog);
        }
        let state = self
            .states
            .get_mut(index)
            .ok_or(RenderError::InvalidCatalog)?;
        *state = 1;

        let branch_positions = Self::checked_range(
            InstructionRange {
                start: node.branch_start,
                end: node.branch_end,
            },
            self.store.branches.len(),
        )?;
        let branches = self
            .store
            .branches
            .get(branch_positions.clone())
            .ok_or(RenderError::InvalidCatalog)?;
        Self::validate_branch_categories(branches, rule_set)?;
        let mut deepest_child = 0u8;
        for branch_position in branch_positions {
            let branch = self
                .store
                .branches
                .get(branch_position)
                .copied()
                .ok_or(RenderError::InvalidCatalog)?;
            let instruction_positions = Self::checked_range(
                InstructionRange {
                    start: branch.instruction_start,
                    end: branch.instruction_end,
                },
                self.store.instructions.len(),
            )?;
            for instruction_position in instruction_positions {
                match self
                    .store
                    .instructions
                    .get(instruction_position)
                    .copied()
                    .ok_or(RenderError::InvalidCatalog)?
                {
                    Instruction::Text { offset, length } => {
                        Self::validate_text(self.store, offset, length)?;
                    }
                    Instruction::Plural { plural_index } => {
                        let child_index = usize::try_from(plural_index)
                            .map_err(|_| RenderError::InvalidCatalog)?;
                        deepest_child =
                            deepest_child.max(self.validate_node(child_index, current_depth + 1)?);
                    }
                    Instruction::Select { select_index } => {
                        let child_index = usize::try_from(select_index)
                            .map_err(|_| RenderError::InvalidCatalog)?;
                        deepest_child = deepest_child
                            .max(self.validate_select_node(child_index, current_depth + 1)?);
                    }
                    Instruction::Format { format_index } => {
                        Self::format_node(self.store, format_index)?;
                    }
                    Instruction::Argument { .. } | Instruction::LineBreak | Instruction::Pound => {}
                }
            }
        }
        let depth = deepest_child
            .checked_add(1)
            .ok_or(RenderError::InvalidCatalog)?;
        if usize::from(depth) > MAX_SELECTOR_NESTING {
            return Err(RenderError::InvalidCatalog);
        }
        let stored_depth = self
            .depths
            .get_mut(index)
            .ok_or(RenderError::InvalidCatalog)?;
        *stored_depth = depth;
        let state = self
            .states
            .get_mut(index)
            .ok_or(RenderError::InvalidCatalog)?;
        *state = 2;
        Ok(depth)
    }

    fn validate_select_node(
        &mut self,
        index: usize,
        current_depth: usize,
    ) -> Result<u8, RenderError> {
        if current_depth > MAX_SELECTOR_NESTING {
            return Err(RenderError::InvalidCatalog);
        }
        match self.select_states.get(index).copied() {
            Some(1) => return Err(RenderError::InvalidCatalog),
            Some(2) => {
                let depth = self
                    .select_depths
                    .get(index)
                    .copied()
                    .ok_or(RenderError::InvalidCatalog)?;
                if current_depth + usize::from(depth) - 1 > MAX_SELECTOR_NESTING {
                    return Err(RenderError::InvalidCatalog);
                }
                return Ok(depth);
            }
            Some(0) => {}
            _ => return Err(RenderError::InvalidCatalog),
        }
        let node = self
            .store
            .selects
            .get(index)
            .copied()
            .ok_or(RenderError::InvalidCatalog)?;
        let state = self
            .select_states
            .get_mut(index)
            .ok_or(RenderError::InvalidCatalog)?;
        *state = 1;
        let positions = Self::checked_range(
            InstructionRange {
                start: node.branch_start,
                end: node.branch_end,
            },
            self.store.select_branches.len(),
        )?;
        let branches = self
            .store
            .select_branches
            .get(positions.clone())
            .ok_or(RenderError::InvalidCatalog)?;
        let mut has_other = false;
        for (offset, branch) in branches.iter().enumerate() {
            let value = Self::text(self.store, branch.value_offset, branch.value_length)?;
            has_other |= value == "other";
            if branches
                .get(offset + 1..)
                .ok_or(RenderError::InvalidCatalog)?
                .iter()
                .any(|later| {
                    Self::text(self.store, later.value_offset, later.value_length).ok()
                        == Some(value)
                })
            {
                return Err(RenderError::InvalidCatalog);
            }
        }
        if !has_other {
            return Err(RenderError::InvalidCatalog);
        }
        let mut deepest_child = 0u8;
        for branch in branches {
            let instruction_positions = Self::checked_range(
                InstructionRange {
                    start: branch.instruction_start,
                    end: branch.instruction_end,
                },
                self.store.instructions.len(),
            )?;
            for instruction_position in instruction_positions {
                match self
                    .store
                    .instructions
                    .get(instruction_position)
                    .copied()
                    .ok_or(RenderError::InvalidCatalog)?
                {
                    Instruction::Text { offset, length } => {
                        Self::validate_text(self.store, offset, length)?;
                    }
                    Instruction::Plural { plural_index } => {
                        let child_index = usize::try_from(plural_index)
                            .map_err(|_| RenderError::InvalidCatalog)?;
                        deepest_child =
                            deepest_child.max(self.validate_node(child_index, current_depth + 1)?);
                    }
                    Instruction::Select { select_index } => {
                        let child_index = usize::try_from(select_index)
                            .map_err(|_| RenderError::InvalidCatalog)?;
                        deepest_child = deepest_child
                            .max(self.validate_select_node(child_index, current_depth + 1)?);
                    }
                    Instruction::Format { format_index } => {
                        Self::format_node(self.store, format_index)?;
                    }
                    Instruction::Argument { .. } | Instruction::LineBreak | Instruction::Pound => {}
                }
            }
        }
        let depth = deepest_child
            .checked_add(1)
            .ok_or(RenderError::InvalidCatalog)?;
        if usize::from(depth) > MAX_SELECTOR_NESTING {
            return Err(RenderError::InvalidCatalog);
        }
        *self
            .select_depths
            .get_mut(index)
            .ok_or(RenderError::InvalidCatalog)? = depth;
        *self
            .select_states
            .get_mut(index)
            .ok_or(RenderError::InvalidCatalog)? = 2;
        Ok(depth)
    }

    fn validate_branch_categories(
        branches: &[PluralBranchRecord],
        rule_set: &RuleSet,
    ) -> Result<(), RenderError> {
        if !branches
            .iter()
            .any(|branch| branch.selector == PluralSelector::Category(PluralCategory::Other))
        {
            return Err(RenderError::InvalidCatalog);
        }
        for (position, branch) in branches.iter().enumerate() {
            if branches
                .get(position + 1..)
                .ok_or(RenderError::InvalidCatalog)?
                .iter()
                .any(|later| later.selector == branch.selector)
            {
                return Err(RenderError::InvalidCatalog);
            }
        }
        if rule_set.rules.iter().any(|rule| {
            !branches
                .iter()
                .any(|branch| branch.selector == PluralSelector::Category(rule.category))
        }) || branches.iter().any(|branch| {
            let PluralSelector::Category(category) = branch.selector else {
                return false;
            };
            category != PluralCategory::Other
                && !rule_set.rules.iter().any(|rule| rule.category == category)
        }) {
            return Err(RenderError::InvalidCatalog);
        }
        Ok(())
    }

    fn validate_message_range(
        &mut self,
        range: InstructionRange,
        argument_count: u32,
        inside_plural: bool,
        message_index: usize,
    ) -> Result<(), RenderError> {
        let positions = Self::checked_range(range, self.store.instructions.len())?;
        for position in positions {
            match self
                .store
                .instructions
                .get(position)
                .copied()
                .ok_or(RenderError::InvalidCatalog)?
            {
                Instruction::Text { offset, length } => {
                    Self::validate_text(self.store, offset, length)?;
                }
                Instruction::Argument { argument_index } => {
                    if argument_index >= argument_count {
                        return Err(RenderError::InvalidCatalog);
                    }
                }
                Instruction::Plural { plural_index } => {
                    self.validate_message_node(plural_index, argument_count, message_index)?;
                }
                Instruction::Select { select_index } => {
                    self.validate_message_select(
                        select_index,
                        argument_count,
                        inside_plural,
                        message_index,
                    )?;
                }
                Instruction::Format { format_index } => {
                    let format = Self::format_node(self.store, format_index)?;
                    if format.argument_index >= argument_count {
                        return Err(RenderError::InvalidCatalog);
                    }
                }
                Instruction::Pound if !inside_plural => {
                    return Err(RenderError::InvalidCatalog);
                }
                Instruction::Pound => {}
                Instruction::LineBreak => {}
            }
        }
        Ok(())
    }

    fn validate_message_node(
        &mut self,
        plural_index: u32,
        argument_count: u32,
        message_index: usize,
    ) -> Result<(), RenderError> {
        let index = usize::try_from(plural_index).map_err(|_| RenderError::InvalidCatalog)?;
        let node = self
            .store
            .plurals
            .get(index)
            .copied()
            .ok_or(RenderError::InvalidCatalog)?;
        if node.argument_index >= argument_count {
            return Err(RenderError::InvalidCatalog);
        }
        if self.message_visits.get(index).copied() == Some(message_index) {
            return Ok(());
        }
        let visit = self
            .message_visits
            .get_mut(index)
            .ok_or(RenderError::InvalidCatalog)?;
        *visit = message_index;
        let branch_positions = Self::checked_range(
            InstructionRange {
                start: node.branch_start,
                end: node.branch_end,
            },
            self.store.branches.len(),
        )?;
        for branch_position in branch_positions {
            let branch = self
                .store
                .branches
                .get(branch_position)
                .copied()
                .ok_or(RenderError::InvalidCatalog)?;
            self.validate_message_range(
                InstructionRange {
                    start: branch.instruction_start,
                    end: branch.instruction_end,
                },
                argument_count,
                true,
                message_index,
            )?;
        }
        Ok(())
    }

    fn validate_message_select(
        &mut self,
        select_index: u32,
        argument_count: u32,
        inside_plural: bool,
        message_index: usize,
    ) -> Result<(), RenderError> {
        let index = usize::try_from(select_index).map_err(|_| RenderError::InvalidCatalog)?;
        let node = self
            .store
            .selects
            .get(index)
            .copied()
            .ok_or(RenderError::InvalidCatalog)?;
        if node.argument_index >= argument_count {
            return Err(RenderError::InvalidCatalog);
        }
        let context_index = usize::from(inside_plural);
        if self
            .select_message_visits
            .get(index)
            .and_then(|visits| visits.get(context_index))
            .copied()
            == Some(message_index)
        {
            return Ok(());
        }
        *self
            .select_message_visits
            .get_mut(index)
            .and_then(|visits| visits.get_mut(context_index))
            .ok_or(RenderError::InvalidCatalog)? = message_index;
        let positions = Self::checked_range(
            InstructionRange {
                start: node.branch_start,
                end: node.branch_end,
            },
            self.store.select_branches.len(),
        )?;
        for position in positions {
            let branch = self
                .store
                .select_branches
                .get(position)
                .copied()
                .ok_or(RenderError::InvalidCatalog)?;
            self.validate_message_range(
                InstructionRange {
                    start: branch.instruction_start,
                    end: branch.instruction_end,
                },
                argument_count,
                inside_plural,
                message_index,
            )?;
        }
        Ok(())
    }

    fn validate_text(
        data: &SharedProgramStore,
        offset: u32,
        length: u32,
    ) -> Result<(), RenderError> {
        Self::text(data, offset, length).map(|_| ())
    }

    fn format_node(data: &SharedProgramStore, index: u32) -> Result<&FormatNode, RenderError> {
        let index = usize::try_from(index).map_err(|_| RenderError::InvalidCatalog)?;
        data.formats.get(index).ok_or(RenderError::InvalidCatalog)
    }

    fn validate_format(format: &FormatNode) -> Result<(), RenderError> {
        let valid = match format.options {
            FormatOptions::Decimal { precision, .. } => match precision {
                Precision::Input => true,
                Precision::Fixed(digits) => digits <= 9,
            },
            FormatOptions::Currency { precision, .. } => match precision {
                CurrencyPrecision::CurrencyDefault => true,
                CurrencyPrecision::Fixed(digits) => digits <= 9,
            },
            FormatOptions::Date { .. } | FormatOptions::Time { .. } => true,
            FormatOptions::Instant { time_zone, .. } => Self::valid_time_zone(time_zone),
        };
        if valid {
            Ok(())
        } else {
            Err(RenderError::InvalidCatalog)
        }
    }

    fn valid_time_zone(time_zone: &str) -> bool {
        !time_zone.is_empty()
            && time_zone.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+' | b'/' | b'.')
            })
    }

    fn text(data: &SharedProgramStore, offset: u32, length: u32) -> Result<&str, RenderError> {
        let start = usize::try_from(offset).map_err(|_| RenderError::InvalidCatalog)?;
        let size = usize::try_from(length).map_err(|_| RenderError::InvalidCatalog)?;
        let end = start.checked_add(size).ok_or(RenderError::InvalidCatalog)?;
        data.text_pool
            .get(start..end)
            .ok_or(RenderError::InvalidCatalog)
    }
}

impl CatalogValidator<'_> {
    fn checked_range(range: InstructionRange, length: usize) -> Result<Range<usize>, RenderError> {
        let start = usize::try_from(range.start).map_err(|_| RenderError::InvalidCatalog)?;
        let end = usize::try_from(range.end).map_err(|_| RenderError::InvalidCatalog)?;
        if start > end || end > length {
            return Err(RenderError::InvalidCatalog);
        }
        Ok(start..end)
    }
}
