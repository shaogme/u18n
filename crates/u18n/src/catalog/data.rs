use super::{Catalog, FormatNode};
use crate::{
    PluralCategory,
    plural::{PluralKind, RuleSet},
};

/// A text instruction referencing a byte range in the shared UTF-8 pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Instruction {
    /// Append `length` bytes from the pool starting at `offset`.
    Text { offset: u32, length: u32 },
    /// Append one logical line break as U+000A.
    LineBreak,
    /// Format the argument at its schema-defined slot.
    Argument { argument_index: u32 },
    /// Evaluate a plural node by index.
    Plural { plural_index: u32 },
    /// Select a named string case by index.
    Select { select_index: u32 },
    /// Apply the format operation at the given index.
    Format { format_index: u32 },
    /// Append the active plural count.
    Pound,
}

/// The number of arguments accepted by one schema message ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageSignature {
    /// Number of arguments expected in schema order.
    pub argument_count: u32,
}

/// A message program's half-open range in the shared instruction table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageProgram {
    /// First instruction in the shared instruction table.
    pub instruction_start: u32,
    /// Instruction table position immediately after this program.
    pub instruction_end: u32,
}

/// A locale-authored mapping from schema message ID to shared program ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageBinding {
    /// Schema message ID, in sorted order within the locale.
    pub message_id: u32,
    /// Index into [`SharedProgramStore::programs`].
    pub program_id: u32,
}

/// A branch selector for an exact count or cardinal category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluralSelector {
    /// Match this integer before applying cardinal rules.
    Exact(u64),
    /// Match the selected CLDR cardinal category.
    Category(PluralCategory),
}

/// A plural branch's category and half-open instruction range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluralBranchRecord {
    /// Exact integer or CLDR category selecting this branch.
    pub selector: PluralSelector,
    /// First instruction in the branch.
    pub instruction_start: u32,
    /// Instruction table position immediately after the branch.
    pub instruction_end: u32,
}

/// A string selection expression bound to one schema parameter slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectNode {
    /// Schema argument slot containing the selected string.
    pub argument_index: u32,
    /// First case record in the catalog select-case table.
    pub branch_start: u32,
    /// Case table position immediately after this select's branches.
    pub branch_end: u32,
}

/// A named select case and its instruction range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectBranchRecord {
    /// Byte range of the exact case name in the shared UTF-8 pool.
    pub value_offset: u32,
    /// Length of the case name in bytes.
    pub value_length: u32,
    /// First instruction in the case.
    pub instruction_start: u32,
    /// Instruction table position immediately after the case.
    pub instruction_end: u32,
}

/// A plural expression bound to a count argument and compiled rule set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluralNode {
    /// Schema argument slot used as the selector.
    pub argument_index: u32,
    /// Whether this selector uses cardinal or ordinal rules.
    pub kind: PluralKind,
    /// Index into [`SharedProgramStore::rule_sets`].
    pub rule_set_index: u32,
    /// First branch record in the catalog branch table.
    pub branch_start: u32,
    /// Branch table position immediately after this plural's branches.
    pub branch_end: u32,
}

/// Immutable program tables shared by every generated locale view.
///
/// The fields are public so generated source can define static data without runtime
/// registration or heap allocation.
pub struct SharedProgramStore {
    /// Shared UTF-8 storage for all literal fragments.
    pub text_pool: &'static str,
    /// Argument counts indexed by schema message ID.
    pub message_signatures: &'static [MessageSignature],
    /// Unique lowered message programs.
    pub programs: &'static [MessageProgram],
    /// Flattened instructions for all programs and selector branches.
    pub instructions: &'static [Instruction],
    /// Plural nodes referenced by plural instructions.
    pub plurals: &'static [PluralNode],
    /// Select nodes referenced by select instructions.
    pub selects: &'static [SelectNode],
    /// Branch records referenced by plural nodes.
    pub branches: &'static [PluralBranchRecord],
    /// Named cases referenced by select nodes.
    pub select_branches: &'static [SelectBranchRecord],
    /// Typed format operations referenced by format instructions.
    pub formats: &'static [FormatNode],
    /// Compiled cardinal and ordinal rule sets used by plural nodes.
    pub rule_sets: &'static [RuleSet],
}

/// A program choice for one locale in a keyed message shard.
pub struct LocaleProgram {
    /// Exact source tag for the locale position represented by this entry.
    pub locale_tag: &'static str,
    /// Index into [`MessageShardData::programs`].
    pub program_id: u32,
}

/// Immutable data owned by one generated message key.
///
/// Fields are public so generated source can construct static shards without registration or
/// allocation. A shard must contain only data for its one message key.
pub struct MessageShardData {
    /// Number of arguments expected in schema order.
    pub argument_count: u32,
    /// Program selection in the generated locale-index order.
    pub locale_programs: &'static [LocaleProgram],
    /// UTF-8 storage for this message's literal fragments and select labels.
    pub text_pool: &'static str,
    /// Lowered message programs used by this key across generated locales.
    pub programs: &'static [MessageProgram],
    /// Flattened instructions for this message's programs and selector branches.
    pub instructions: &'static [Instruction],
    /// Plural nodes referenced by this message's plural instructions.
    pub plurals: &'static [PluralNode],
    /// Select nodes referenced by this message's select instructions.
    pub selects: &'static [SelectNode],
    /// Branch records referenced by this message's plural nodes.
    pub branches: &'static [PluralBranchRecord],
    /// Named cases referenced by this message's select nodes.
    pub select_branches: &'static [SelectBranchRecord],
    /// Typed format operations referenced by this message's format instructions.
    pub formats: &'static [FormatNode],
    /// Compiled plural rule sets used only by this message's programs.
    pub rule_sets: &'static [RuleSet],
}

/// One immutable locale view over a shared program store.
///
/// Bindings must be strictly sorted by message ID and contain only messages authored in this
/// locale. Missing IDs are looked up through `fallback`, which points directly to the locale
/// named by the source header.
pub struct CatalogData {
    /// Authored source tag used by caller-provided formatters.
    pub locale_tag: &'static str,
    /// Explicit fallback locale, when this source locale declares one.
    pub fallback: Option<&'static Catalog>,
    /// Sorted message-to-program bindings authored in this locale.
    pub bindings: &'static [MessageBinding],
    /// Shared immutable program tables.
    pub shared_store: &'static SharedProgramStore,
}
