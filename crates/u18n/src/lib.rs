#![no_std]

extern crate alloc;

mod catalog;
mod format;
mod plural;
mod validation;

#[doc(hidden)]
pub use u18n_macros::{
    dispatch_translation as __dispatch_translation, invoke_message as __invoke_message,
};

pub use catalog::{
    ArgumentValue, Catalog, CatalogData, CivilDate, CivilTime, CurrencyPrecision, Decimal,
    FormatNode, FormatOptions, FormatRequest, FormatStyle, FormatValue, Grouping, Instant,
    Instruction, Locale, LocaleProgram, MessageBinding, MessageData, MessageProgram,
    MessageShardData, MessageSignature, Money, OrdinalCount, PluralBranchRecord, PluralCount,
    PluralDecimal, PluralNode, PluralSelector, Precision, RenderError, Rounding,
    SelectBranchRecord, SelectNode, SharedProgramStore,
};
pub use format::{Formatter, FormatterError};
pub use plural::{
    PluralCategory, PluralConjunction, PluralKind, PluralNumber, PluralOperand, PluralPredicate,
    PluralRange, PluralRule, RuleSet,
};

#[doc(hidden)]
pub mod __private {
    pub use u18n_locale_tag::{ParsedTag, TagError};
}
