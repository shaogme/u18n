use alloc::string::String;
use core::{
    convert::TryFrom,
    error::Error,
    fmt::{Display, Error as FmtError, Formatter as FmtFormatter, Result as FmtResult, Write},
    ops::Range,
    ptr,
};

use crate::{
    PluralCategory, PluralKind, RenderError,
    catalog::{
        ArgumentValue, Catalog, CatalogData, Decimal, FormatNode, FormatOptions, FormatRequest,
        FormatValue, Instruction, Locale, MessageData, MessageProgram, MessageShardData,
        PluralBranchRecord, PluralDecimal, PluralNode, PluralSelector, SelectBranchRecord,
        SelectNode, SharedProgramStore,
    },
    plural::{PluralNumber, RuleEvaluator, RuleSet},
    validation::{CatalogValidator, InstructionRange, MAX_SELECTOR_NESTING},
};

/// A failure reported by a caller-provided locale formatter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatterError {
    /// The formatter cannot support this value or option combination.
    Unsupported,
    /// The formatter encountered an internal formatting failure.
    Failed,
}

impl Display for FormatterError {
    fn fmt(&self, formatter: &mut FmtFormatter<'_>) -> FmtResult {
        let message = match self {
            Self::Unsupported => "formatter does not support this request",
            Self::Failed => "formatter could not complete the request",
        };
        formatter.write_str(message)
    }
}

impl Error for FormatterError {}

/// Caller-owned locale formatting adapter. The runtime supplies the catalog's authored tag.
pub trait Formatter {
    /// Writes one fully typed request to the provided sink.
    fn format(
        &self,
        request: FormatRequest<'_>,
        output: &mut dyn Write,
    ) -> Result<(), FormatterError>;
}

#[derive(Clone, Copy)]
struct ProgramTables<'tables> {
    text_pool: &'tables str,
    instructions: &'tables [Instruction],
    plurals: &'tables [PluralNode],
    selects: &'tables [SelectNode],
    branches: &'tables [PluralBranchRecord],
    select_branches: &'tables [SelectBranchRecord],
    formats: &'tables [FormatNode],
    rule_sets: &'tables [RuleSet],
}

struct RenderContext<'catalog, 'arguments, 'value> {
    locale_tag: &'catalog str,
    tables: ProgramTables<'catalog>,
    arguments: &'arguments [ArgumentValue<'value>],
    formatter: Option<&'catalog dyn Formatter>,
}

impl<'tables> ProgramTables<'tables> {
    fn from_shared(store: &'tables SharedProgramStore) -> Self {
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

    fn from_shard(shard: &'tables MessageShardData) -> Self {
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
}

#[derive(Clone, Copy)]
struct ActivePluralValue {
    display: PluralDecimal,
    number: PluralNumber,
}

impl ActivePluralValue {
    fn from_decimal(value: PluralDecimal) -> Self {
        Self {
            display: value,
            number: PluralNumber {
                coefficient: value.coefficient() as u128,
                scale: value.scale(),
            },
        }
    }

    fn from_integer(value: u64) -> Result<Self, RenderError> {
        let coefficient = i128::from(value);
        let display = PluralDecimal::new(coefficient, 0).ok_or(RenderError::InvalidCatalog)?;
        Ok(Self::from_decimal(display))
    }
}

struct TrackingWriter<'a> {
    output: &'a mut dyn Write,
    failed: bool,
}

impl TrackingWriter<'_> {
    fn result(&self, result: Result<(), FmtError>) -> Result<(), RenderError> {
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

pub(crate) struct Renderer;

impl Renderer {
    pub(crate) fn validate_catalog(data: &CatalogData) -> Result<(), RenderError> {
        CatalogValidator::validate(data)
    }

    pub(crate) fn render(
        catalog: &Catalog,
        message_id: u32,
        arguments: &[ArgumentValue<'_>],
    ) -> Result<String, RenderError> {
        let mut output = String::new();
        Self::render_to(catalog, message_id, arguments, None, &mut output)?;
        Ok(output)
    }

    pub(crate) fn render_with(
        catalog: &Catalog,
        formatter: &dyn Formatter,
        message_id: u32,
        arguments: &[ArgumentValue<'_>],
    ) -> Result<String, RenderError> {
        let mut output = String::new();
        Self::render_to(catalog, message_id, arguments, Some(formatter), &mut output)?;
        Ok(output)
    }

    pub(crate) fn render_to(
        catalog: &Catalog,
        message_id: u32,
        arguments: &[ArgumentValue<'_>],
        formatter: Option<&dyn Formatter>,
        output: &mut dyn Write,
    ) -> Result<(), RenderError> {
        let data = catalog.data();
        let message_index =
            usize::try_from(message_id).map_err(|_| RenderError::MessageOutOfRange)?;
        let signature = data
            .shared_store
            .message_signatures
            .get(message_index)
            .ok_or(RenderError::MessageOutOfRange)?;
        let program = Self::lookup_program(catalog, message_id)?;
        Self::validate_arguments(signature.argument_count, arguments)?;
        let context = RenderContext {
            locale_tag: data.locale_tag,
            tables: ProgramTables::from_shared(data.shared_store),
            arguments,
            formatter,
        };
        let mut tracked = TrackingWriter {
            output,
            failed: false,
        };
        Self::render_program(&mut tracked, &context, program)
    }

    pub(crate) fn render_message(
        message: &MessageData,
        locale: &Locale,
        arguments: &[ArgumentValue<'_>],
    ) -> Result<String, RenderError> {
        let mut output = String::new();
        Self::render_message_to(message, locale, arguments, None, &mut output)?;
        Ok(output)
    }

    pub(crate) fn render_message_with(
        message: &MessageData,
        locale: &Locale,
        formatter: &dyn Formatter,
        arguments: &[ArgumentValue<'_>],
    ) -> Result<String, RenderError> {
        let mut output = String::new();
        Self::render_message_to(message, locale, arguments, Some(formatter), &mut output)?;
        Ok(output)
    }

    pub(crate) fn render_message_to(
        message: &MessageData,
        locale: &Locale,
        arguments: &[ArgumentValue<'_>],
        formatter: Option<&dyn Formatter>,
        output: &mut dyn Write,
    ) -> Result<(), RenderError> {
        let shard = message.shard();
        let locale_index =
            usize::try_from(locale.index()).map_err(|_| RenderError::LocaleOutOfRange)?;
        let selection = shard
            .locale_programs
            .get(locale_index)
            .ok_or(RenderError::LocaleOutOfRange)?;
        if selection.locale_tag != locale.as_str() {
            return Err(RenderError::LocaleMismatch);
        }
        let program_index =
            usize::try_from(selection.program_id).map_err(|_| RenderError::InvalidCatalog)?;
        let program = shard
            .programs
            .get(program_index)
            .ok_or(RenderError::InvalidCatalog)?;
        Self::validate_arguments(shard.argument_count, arguments)?;
        let context = RenderContext {
            locale_tag: locale.as_str(),
            tables: ProgramTables::from_shard(shard),
            arguments,
            formatter,
        };
        let mut tracked = TrackingWriter {
            output,
            failed: false,
        };
        Self::render_program(&mut tracked, &context, program)
    }

    fn render_program(
        output: &mut TrackingWriter<'_>,
        context: &RenderContext<'_, '_, '_>,
        program: &MessageProgram,
    ) -> Result<(), RenderError> {
        Self::render_range(
            output,
            context,
            InstructionRange {
                start: program.instruction_start,
                end: program.instruction_end,
            },
            None,
            0,
        )
    }

    fn validate_arguments(
        argument_count: u32,
        arguments: &[ArgumentValue<'_>],
    ) -> Result<(), RenderError> {
        if usize::try_from(argument_count).ok() != Some(arguments.len()) {
            return Err(RenderError::ArgumentCountMismatch);
        }
        Ok(())
    }

    fn lookup_program(catalog: &Catalog, message_id: u32) -> Result<&MessageProgram, RenderError> {
        let mut current = Some(catalog);
        let mut slow = Some(catalog);
        let mut fast = Some(catalog);
        while let Some(view) = current {
            if let Ok(binding_index) = view
                .data()
                .bindings
                .binary_search_by_key(&message_id, |binding| binding.message_id)
            {
                let binding = view
                    .data()
                    .bindings
                    .get(binding_index)
                    .ok_or(RenderError::InvalidCatalog)?;
                let program_index =
                    usize::try_from(binding.program_id).map_err(|_| RenderError::InvalidCatalog)?;
                return catalog
                    .data()
                    .shared_store
                    .programs
                    .get(program_index)
                    .ok_or(RenderError::InvalidCatalog);
            }
            current = view.data().fallback;
            slow = slow.and_then(|node| node.data().fallback);
            fast = fast
                .and_then(|node| node.data().fallback)
                .and_then(|node| node.data().fallback);
            if let (Some(slow_node), Some(fast_node)) = (slow, fast)
                && ptr::eq(slow_node, fast_node)
            {
                return Err(RenderError::InvalidCatalog);
            }
        }
        Err(RenderError::InvalidCatalog)
    }

    fn render_range(
        output: &mut TrackingWriter<'_>,
        context: &RenderContext<'_, '_, '_>,
        range: InstructionRange,
        active_count: Option<ActivePluralValue>,
        plural_depth: usize,
    ) -> Result<(), RenderError> {
        let instructions = context.tables.instructions;
        let positions = Self::checked_range(range, instructions.len())?;
        let selected = instructions
            .get(positions)
            .ok_or(RenderError::InvalidCatalog)?;
        for instruction in selected {
            match instruction {
                Instruction::Text { offset, length } => {
                    Self::append_text(output, context.tables.text_pool, *offset, *length)?;
                }
                Instruction::Argument { argument_index } => {
                    Self::append_argument(output, context, *argument_index)?;
                }
                Instruction::Plural { plural_index } => {
                    let next_depth = plural_depth
                        .checked_add(1)
                        .ok_or(RenderError::InvalidCatalog)?;
                    if next_depth > MAX_SELECTOR_NESTING {
                        return Err(RenderError::InvalidCatalog);
                    }
                    Self::render_plural(output, context, *plural_index, next_depth)?;
                }
                Instruction::Select { select_index } => {
                    let next_depth = plural_depth
                        .checked_add(1)
                        .ok_or(RenderError::InvalidCatalog)?;
                    if next_depth > MAX_SELECTOR_NESTING {
                        return Err(RenderError::InvalidCatalog);
                    }
                    Self::render_select(output, context, *select_index, active_count, next_depth)?;
                }
                Instruction::Format { format_index } => {
                    Self::render_format(output, context, *format_index)?;
                }
                Instruction::Pound => {
                    let count = active_count.ok_or(RenderError::PoundOutsidePlural)?;
                    let result = write!(output, "{}", count.display);
                    output.result(result)?;
                }
            }
        }
        Ok(())
    }

    fn append_text(
        output: &mut TrackingWriter<'_>,
        text_pool: &str,
        offset: u32,
        length: u32,
    ) -> Result<(), RenderError> {
        let start = usize::try_from(offset).map_err(|_| RenderError::InvalidCatalog)?;
        let size = usize::try_from(length).map_err(|_| RenderError::InvalidCatalog)?;
        let end = start.checked_add(size).ok_or(RenderError::InvalidCatalog)?;
        let text = text_pool
            .get(start..end)
            .ok_or(RenderError::InvalidCatalog)?;
        let result = output.write_str(text);
        output.result(result)
    }

    fn append_argument(
        output: &mut TrackingWriter<'_>,
        context: &RenderContext<'_, '_, '_>,
        argument_index: u32,
    ) -> Result<(), RenderError> {
        let index = usize::try_from(argument_index).map_err(|_| RenderError::ArgumentOutOfRange)?;
        let argument = context
            .arguments
            .get(index)
            .ok_or(RenderError::ArgumentOutOfRange)?;
        match argument {
            ArgumentValue::String(value) => {
                let result = output.write_str(value);
                return output.result(result);
            }
            ArgumentValue::Number(value) => {
                let result = write!(output, "{value}");
                output.result(result)?;
            }
            ArgumentValue::PluralCount(value) => {
                let result = write!(output, "{}", value.get());
                output.result(result)?;
            }
            ArgumentValue::OrdinalCount(value) => {
                let result = write!(output, "{}", value.get());
                output.result(result)?;
            }
            ArgumentValue::PluralDecimal(value) => {
                let result = write!(output, "{value}");
                output.result(result)?;
            }
            ArgumentValue::Decimal(_)
            | ArgumentValue::Money(_)
            | ArgumentValue::Date(_)
            | ArgumentValue::Time(_)
            | ArgumentValue::Instant(_) => return Err(RenderError::ArgumentTypeMismatch),
        }
        Ok(())
    }

    fn render_format(
        output: &mut TrackingWriter<'_>,
        context: &RenderContext<'_, '_, '_>,
        format_index: u32,
    ) -> Result<(), RenderError> {
        let index = usize::try_from(format_index).map_err(|_| RenderError::InvalidCatalog)?;
        let node = context
            .tables
            .formats
            .get(index)
            .ok_or(RenderError::InvalidCatalog)?;
        let argument_index =
            usize::try_from(node.argument_index).map_err(|_| RenderError::ArgumentOutOfRange)?;
        let argument = context
            .arguments
            .get(argument_index)
            .ok_or(RenderError::ArgumentOutOfRange)?;
        let value = Self::format_value(node, argument)?;
        let formatter = context.formatter.ok_or(RenderError::FormatterRequired)?;
        let request = FormatRequest {
            locale: context.locale_tag,
            value,
            options: node.options,
        };
        let mut adapter_output = TrackingWriter {
            output,
            failed: false,
        };
        let result = formatter.format(request, &mut adapter_output);
        if adapter_output.failed {
            return Err(RenderError::WriteFailed);
        }
        if result.is_err() {
            return Err(RenderError::FormatterFailed);
        }
        Ok(())
    }

    fn format_value<'value>(
        node: &FormatNode,
        argument: &ArgumentValue<'value>,
    ) -> Result<FormatValue<'value>, RenderError> {
        match (node.options, argument) {
            (FormatOptions::Decimal { .. }, ArgumentValue::Decimal(value)) => {
                Ok(FormatValue::Decimal(*value))
            }
            (FormatOptions::Decimal { .. }, ArgumentValue::PluralDecimal(value)) => Ok(
                FormatValue::Decimal(Decimal::new(value.coefficient(), value.scale())),
            ),
            (FormatOptions::Currency { .. }, ArgumentValue::Money(value)) => {
                Ok(FormatValue::Money(*value))
            }
            (FormatOptions::Date { .. }, ArgumentValue::Date(value)) => {
                Ok(FormatValue::Date(*value))
            }
            (FormatOptions::Time { .. }, ArgumentValue::Time(value)) => {
                Ok(FormatValue::Time(*value))
            }
            (FormatOptions::Instant { .. }, ArgumentValue::Instant(value)) => {
                Ok(FormatValue::Instant(*value))
            }
            _ => Err(RenderError::ArgumentTypeMismatch),
        }
    }

    fn render_plural(
        output: &mut TrackingWriter<'_>,
        context: &RenderContext<'_, '_, '_>,
        plural_index: u32,
        plural_depth: usize,
    ) -> Result<(), RenderError> {
        let index = usize::try_from(plural_index).map_err(|_| RenderError::InvalidCatalog)?;
        let plural = context
            .tables
            .plurals
            .get(index)
            .ok_or(RenderError::InvalidCatalog)?;
        let count = Self::plural_argument(context, plural)?;
        if let Some(exact) = Self::exact_integer(count.number)?
            && let Some(branch) = Self::find_exact_branch(context, plural, exact)?
        {
            return Self::render_range(
                output,
                context,
                InstructionRange {
                    start: branch.instruction_start,
                    end: branch.instruction_end,
                },
                Some(count),
                plural_depth,
            );
        }
        let rule_set = Self::rule_set(context, plural)?;
        let category = RuleEvaluator::select(rule_set, count.number)?;
        let branch = Self::find_branch(context, plural, category)?;
        Self::render_range(
            output,
            context,
            InstructionRange {
                start: branch.instruction_start,
                end: branch.instruction_end,
            },
            Some(count),
            plural_depth,
        )
    }

    fn render_select(
        output: &mut TrackingWriter<'_>,
        context: &RenderContext<'_, '_, '_>,
        select_index: u32,
        active_count: Option<ActivePluralValue>,
        selector_depth: usize,
    ) -> Result<(), RenderError> {
        let index = usize::try_from(select_index).map_err(|_| RenderError::InvalidCatalog)?;
        let node = context
            .tables
            .selects
            .get(index)
            .ok_or(RenderError::InvalidCatalog)?;
        let value = Self::select_argument(context, node)?;
        let branch = Self::find_select_branch(context, node, value)?;
        Self::render_range(
            output,
            context,
            InstructionRange {
                start: branch.instruction_start,
                end: branch.instruction_end,
            },
            active_count,
            selector_depth,
        )
    }

    fn select_argument<'value>(
        context: &RenderContext<'_, '_, 'value>,
        select: &SelectNode,
    ) -> Result<&'value str, RenderError> {
        let index =
            usize::try_from(select.argument_index).map_err(|_| RenderError::ArgumentOutOfRange)?;
        match context.arguments.get(index) {
            Some(ArgumentValue::String(value)) => Ok(value),
            Some(_) => Err(RenderError::ArgumentTypeMismatch),
            None => Err(RenderError::ArgumentOutOfRange),
        }
    }

    fn find_select_branch<'catalog>(
        context: &'catalog RenderContext<'_, '_, '_>,
        select: &SelectNode,
        value: &str,
    ) -> Result<&'catalog SelectBranchRecord, RenderError> {
        let positions = Self::checked_range(
            InstructionRange {
                start: select.branch_start,
                end: select.branch_end,
            },
            context.tables.select_branches.len(),
        )?;
        let branches = context
            .tables
            .select_branches
            .get(positions)
            .ok_or(RenderError::InvalidCatalog)?;
        let mut other = None;
        for branch in branches {
            let name = Self::text(
                context.tables.text_pool,
                branch.value_offset,
                branch.value_length,
            )?;
            if name == value {
                return Ok(branch);
            }
            if name == "other" {
                other = Some(branch);
            }
        }
        other.ok_or(RenderError::InvalidCatalog)
    }

    fn plural_argument(
        context: &RenderContext<'_, '_, '_>,
        plural: &PluralNode,
    ) -> Result<ActivePluralValue, RenderError> {
        let index =
            usize::try_from(plural.argument_index).map_err(|_| RenderError::ArgumentOutOfRange)?;
        match context.arguments.get(index) {
            Some(ArgumentValue::PluralCount(value)) if plural.kind == PluralKind::Cardinal => {
                ActivePluralValue::from_integer(value.get())
            }
            Some(ArgumentValue::PluralDecimal(value)) if plural.kind == PluralKind::Cardinal => {
                Ok(ActivePluralValue::from_decimal(*value))
            }
            Some(ArgumentValue::OrdinalCount(value)) if plural.kind == PluralKind::Ordinal => {
                ActivePluralValue::from_integer(value.get())
            }
            Some(_) => Err(RenderError::ArgumentTypeMismatch),
            None => Err(RenderError::ArgumentOutOfRange),
        }
    }

    fn rule_set<'catalog>(
        context: &'catalog RenderContext<'_, '_, '_>,
        plural: &PluralNode,
    ) -> Result<&'catalog RuleSet, RenderError> {
        let index =
            usize::try_from(plural.rule_set_index).map_err(|_| RenderError::InvalidCatalog)?;
        let rule_set = context
            .tables
            .rule_sets
            .get(index)
            .ok_or(RenderError::InvalidCatalog)?;
        if rule_set.kind != plural.kind {
            return Err(RenderError::InvalidCatalog);
        }
        Ok(rule_set)
    }

    fn find_branch<'catalog>(
        context: &'catalog RenderContext<'_, '_, '_>,
        plural: &PluralNode,
        category: PluralCategory,
    ) -> Result<&'catalog PluralBranchRecord, RenderError> {
        let positions = Self::checked_range(
            InstructionRange {
                start: plural.branch_start,
                end: plural.branch_end,
            },
            context.tables.branches.len(),
        )?;
        let branches = context
            .tables
            .branches
            .get(positions)
            .ok_or(RenderError::InvalidCatalog)?;
        let mut found = None;
        for branch in branches {
            if branch.selector == PluralSelector::Category(category) {
                if found.is_some() {
                    return Err(RenderError::InvalidCatalog);
                }
                found = Some(branch);
            }
        }
        found.ok_or(RenderError::InvalidCatalog)
    }

    fn find_exact_branch<'catalog>(
        context: &'catalog RenderContext<'_, '_, '_>,
        plural: &PluralNode,
        value: u64,
    ) -> Result<Option<&'catalog PluralBranchRecord>, RenderError> {
        let positions = Self::checked_range(
            InstructionRange {
                start: plural.branch_start,
                end: plural.branch_end,
            },
            context.tables.branches.len(),
        )?;
        let branches = context
            .tables
            .branches
            .get(positions)
            .ok_or(RenderError::InvalidCatalog)?;
        Ok(branches
            .iter()
            .find(|branch| branch.selector == PluralSelector::Exact(value)))
    }

    fn exact_integer(number: PluralNumber) -> Result<Option<u64>, RenderError> {
        let mut scale = 1u128;
        for _ in 0..number.scale {
            scale = scale.checked_mul(10).ok_or(RenderError::InvalidCatalog)?;
        }
        if !number.coefficient.is_multiple_of(scale) {
            return Ok(None);
        }
        Ok(u64::try_from(number.coefficient / scale).ok())
    }

    fn text(text_pool: &str, offset: u32, length: u32) -> Result<&str, RenderError> {
        let start = usize::try_from(offset).map_err(|_| RenderError::InvalidCatalog)?;
        let size = usize::try_from(length).map_err(|_| RenderError::InvalidCatalog)?;
        let end = start.checked_add(size).ok_or(RenderError::InvalidCatalog)?;
        text_pool.get(start..end).ok_or(RenderError::InvalidCatalog)
    }

    fn checked_range(range: InstructionRange, length: usize) -> Result<Range<usize>, RenderError> {
        let start = usize::try_from(range.start).map_err(|_| RenderError::InvalidCatalog)?;
        let end = usize::try_from(range.end).map_err(|_| RenderError::InvalidCatalog)?;
        if start > end || end > length {
            return Err(RenderError::InvalidCatalog);
        }
        Ok(start..end)
    }
}
