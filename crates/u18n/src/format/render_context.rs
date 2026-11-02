use core::fmt::{Error as FmtError, Result as FmtResult, Write};

use crate::{
    RenderError,
    catalog::{
        ArgumentValue, FormatNode, Instruction, MessageShardData, PluralBranchRecord,
        PluralDecimal, PluralNode, SelectBranchRecord, SelectNode, SharedProgramStore,
    },
    plural::{PluralNumber, RuleSet},
};

use super::Formatter;

#[derive(Clone, Copy)]
pub(super) struct ProgramTables<'tables> {
    text_pool: &'tables str,
    instructions: &'tables [Instruction],
    plurals: &'tables [PluralNode],
    selects: &'tables [SelectNode],
    branches: &'tables [PluralBranchRecord],
    select_branches: &'tables [SelectBranchRecord],
    formats: &'tables [FormatNode],
    rule_sets: &'tables [RuleSet],
}

pub(super) struct RenderContext<'catalog, 'arguments, 'value> {
    pub(super) locale_tag: &'catalog str,
    pub(super) tables: ProgramTables<'catalog>,
    pub(super) arguments: &'arguments [ArgumentValue<'value>],
    pub(super) formatter: Option<&'catalog dyn Formatter>,
}

#[derive(Clone, Copy)]
pub(super) struct ActivePluralValue {
    display: PluralDecimal,
    number: PluralNumber,
}

pub(super) struct TrackingWriter<'a> {
    output: &'a mut dyn Write,
    failed: bool,
}

impl<'tables> ProgramTables<'tables> {
    pub(super) fn from_shared(store: &'tables SharedProgramStore) -> Self {
        Self {
            text_pool: store.text_pool,
            instructions: store.instructions,
            plurals: store.plurals,
            selects: store.selects,
            branches: store.branches,
            select_branches: store.select_branches,
            formats: store.formats,
            rule_sets: store.rule_sets,
        }
    }

    pub(super) fn from_shard(shard: &'tables MessageShardData) -> Self {
        Self {
            text_pool: shard.text_pool,
            instructions: shard.instructions,
            plurals: shard.plurals,
            selects: shard.selects,
            branches: shard.branches,
            select_branches: shard.select_branches,
            formats: shard.formats,
            rule_sets: shard.rule_sets,
        }
    }

    pub(super) fn text_pool(&self) -> &'tables str {
        self.text_pool
    }

    pub(super) fn instructions(&self) -> &'tables [Instruction] {
        self.instructions
    }

    pub(super) fn plurals(&self) -> &'tables [PluralNode] {
        self.plurals
    }

    pub(super) fn selects(&self) -> &'tables [SelectNode] {
        self.selects
    }

    pub(super) fn branches(&self) -> &'tables [PluralBranchRecord] {
        self.branches
    }

    pub(super) fn select_branches(&self) -> &'tables [SelectBranchRecord] {
        self.select_branches
    }

    pub(super) fn formats(&self) -> &'tables [FormatNode] {
        self.formats
    }

    pub(super) fn rule_sets(&self) -> &'tables [RuleSet] {
        self.rule_sets
    }
}

impl ActivePluralValue {
    pub(super) fn from_decimal(value: PluralDecimal) -> Self {
        Self {
            display: value,
            number: PluralNumber {
                coefficient: value.coefficient() as u128,
                scale: value.scale(),
            },
        }
    }

    pub(super) fn from_integer(value: u64) -> Result<Self, RenderError> {
        let coefficient = i128::from(value);
        let display = PluralDecimal::new(coefficient, 0).ok_or(RenderError::InvalidCatalog)?;
        Ok(Self::from_decimal(display))
    }

    pub(super) fn display(self) -> PluralDecimal {
        self.display
    }

    pub(super) fn number(self) -> PluralNumber {
        self.number
    }
}

impl TrackingWriter<'_> {
    pub(super) fn new(output: &mut dyn Write) -> TrackingWriter<'_> {
        TrackingWriter {
            output,
            failed: false,
        }
    }

    pub(super) fn result(&self, result: Result<(), FmtError>) -> Result<(), RenderError> {
        if result.is_err() && self.failed {
            Err(RenderError::WriteFailed)
        } else if result.is_err() {
            Err(RenderError::FormattingFailed)
        } else if self.failed {
            Err(RenderError::WriteFailed)
        } else {
            Ok(())
        }
    }

    pub(super) fn has_failed_write(&self) -> bool {
        self.failed
    }
}

impl Write for TrackingWriter<'_> {
    fn write_str(&mut self, value: &str) -> FmtResult {
        match self.output.write_str(value) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }
}
