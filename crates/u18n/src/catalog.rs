mod data;

pub use data::{
    CatalogData, Instruction, LocaleProgram, MessageBinding, MessageProgram, MessageShardData,
    MessageSignature, PluralBranchRecord, PluralNode, PluralSelector, SelectBranchRecord,
    SelectNode, SharedProgramStore,
};

use alloc::string::String;
use core::{
    error::Error,
    fmt::{Display, Error as FmtError, Formatter as FmtFormatter, Result as FmtResult, Write},
};

use crate::format::{Formatter, Renderer};

/// A non-negative integer count used by cardinal plural selectors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PluralCount {
    value: u64,
}

/// A non-negative integer rank used by ordinal plural selectors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OrdinalCount {
    value: u64,
}

/// A signed decimal coefficient paired with its authored scale.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Decimal {
    coefficient: i128,
    scale: u8,
}

/// A non-negative exact decimal quantity for cardinal plural rules.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PluralDecimal {
    coefficient: i128,
    scale: u8,
}

/// A decimal amount paired with a three-letter currency code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Money<'a> {
    amount: Decimal,
    currency: &'a str,
}

/// A validated Gregorian civil date.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CivilDate {
    year: i32,
    month: u8,
    day: u8,
}

/// A wall-clock time without a date or time zone.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CivilTime {
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
}

/// An absolute instant represented as seconds and nanoseconds from the Unix epoch.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Instant {
    unix_seconds: i64,
    nanosecond: u32,
}

/// Grouping behavior requested from the caller's formatter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Grouping {
    None,
    Auto,
}

/// Decimal precision requested from the caller's formatter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Precision {
    Input,
    Fixed(u8),
}

/// Currency precision requested from the caller's formatter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CurrencyPrecision {
    CurrencyDefault,
    Fixed(u8),
}

/// Rounding behavior requested from the caller's formatter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Rounding {
    HalfEven,
    HalfUp,
    TowardZero,
    AwayFromZero,
}

/// Date and time style requested from the caller's formatter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FormatStyle {
    Short,
    Medium,
    Long,
    Full,
}

/// A closed formatting operation and its explicit options.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FormatOptions {
    Decimal {
        grouping: Grouping,
        precision: Precision,
        rounding: Rounding,
    },
    Currency {
        precision: CurrencyPrecision,
        rounding: Rounding,
    },
    Date {
        style: FormatStyle,
    },
    Time {
        style: FormatStyle,
    },
    Instant {
        style: FormatStyle,
        time_zone: &'static str,
    },
}

/// One typed value supplied to a formatting adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FormatValue<'a> {
    Decimal(Decimal),
    Money(Money<'a>),
    Date(CivilDate),
    Time(CivilTime),
    Instant(Instant),
}

/// A request containing the selected catalog's locale and typed options.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FormatRequest<'a> {
    pub locale: &'a str,
    pub value: FormatValue<'a>,
    pub options: FormatOptions,
}

/// A static format operation bound to one schema argument.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FormatNode {
    pub argument_index: u32,
    pub options: FormatOptions,
}

/// A runtime argument bound to one schema parameter slot.
#[derive(Clone, Copy)]
pub enum ArgumentValue<'a> {
    /// A borrowed UTF-8 string.
    String(&'a str),
    /// A caller-formatted number.
    Number(&'a dyn Display),
    /// An integer count usable by interpolation and plural selectors.
    PluralCount(PluralCount),
    /// An exact decimal quantity usable by interpolation and cardinal selectors.
    PluralDecimal(PluralDecimal),
    /// An integer rank usable by ordinal selectors and interpolation.
    OrdinalCount(OrdinalCount),
    /// A decimal whose scale is preserved for locale-aware formatting.
    Decimal(Decimal),
    /// A currency amount and its ISO code.
    Money(Money<'a>),
    /// A Gregorian date without time-zone semantics.
    Date(CivilDate),
    /// A wall-clock time without date or time-zone semantics.
    Time(CivilTime),
    /// An absolute instant.
    Instant(Instant),
}

/// A generated locale identity that does not retain catalog tables.
pub struct Locale {
    index: u32,
    locale_tag: &'static str,
}

/// A handle to one generated message shard.
pub struct MessageData {
    shard: MessageShardData,
}

/// An opaque handle to one generated locale catalog.
pub struct Catalog {
    data: CatalogData,
}

/// An error encountered while reading a generated catalog or rendering a message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderError {
    /// The numeric message ID is not present in the catalog.
    MessageOutOfRange,
    /// The selected generated locale index is absent from this message shard.
    LocaleOutOfRange,
    /// The selected locale tag does not match this message shard's locale index.
    LocaleMismatch,
    /// The supplied argument count differs from the message signature.
    ArgumentCountMismatch,
    /// An instruction references an argument slot outside the supplied list.
    ArgumentOutOfRange,
    /// A plural selector references an argument that is not a plural count.
    ArgumentTypeMismatch,
    /// A table range or reference is invalid or internally inconsistent.
    InvalidCatalog,
    /// A pound instruction appeared without an active plural selector.
    PoundOutsidePlural,
    /// A caller's `Display` implementation returned a formatting error.
    FormattingFailed,
    /// A formatted operation was rendered without a formatter.
    FormatterRequired,
    /// The caller-provided formatter rejected a request.
    FormatterFailed,
    /// The caller-owned output sink rejected a write.
    WriteFailed,
}

impl PluralCount {
    /// Creates a count from an unsigned integer.
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    /// Returns the underlying integer value.
    pub const fn get(self) -> u64 {
        self.value
    }
}

impl OrdinalCount {
    /// Creates an ordinal count from an unsigned integer.
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    /// Returns the underlying rank value.
    pub const fn get(self) -> u64 {
        self.value
    }
}

impl Decimal {
    /// Creates a decimal from its signed coefficient and decimal scale.
    pub const fn new(coefficient: i128, scale: u8) -> Self {
        Self { coefficient, scale }
    }

    /// Returns the signed integer coefficient.
    pub const fn coefficient(self) -> i128 {
        self.coefficient
    }

    /// Returns the number of visible fractional digits.
    pub const fn scale(self) -> u8 {
        self.scale
    }
}

impl PluralDecimal {
    /// Creates a non-negative decimal quantity with up to 38 visible fractional digits.
    pub const fn new(coefficient: i128, scale: u8) -> Option<Self> {
        if coefficient < 0 || scale > 38 {
            return None;
        }
        Some(Self { coefficient, scale })
    }

    /// Returns the non-negative integer coefficient.
    pub const fn coefficient(self) -> i128 {
        self.coefficient
    }

    /// Returns the number of visible fractional digits.
    pub const fn scale(self) -> u8 {
        self.scale
    }
}

impl<'a> Money<'a> {
    /// Creates a money value if `currency` has the shape of a three-letter ISO code.
    ///
    /// This checks the code's shape only; callers must supply a code assigned by ISO 4217.
    pub fn new(amount: Decimal, currency: &'a str) -> Option<Self> {
        let bytes = currency.as_bytes();
        if bytes.len() != 3 {
            return None;
        }
        if !bytes.iter().all(u8::is_ascii_uppercase) {
            return None;
        }
        Some(Money { amount, currency })
    }

    /// Returns the exact decimal amount.
    pub const fn amount(self) -> Decimal {
        self.amount
    }

    /// Returns the currency code supplied to [`Money::new`].
    pub const fn currency(self) -> &'a str {
        self.currency
    }
}

impl CivilDate {
    /// Creates a valid proleptic Gregorian date.
    pub const fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if month == 0 || month > 12 || day == 0 {
            return None;
        }
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        if day > days {
            return None;
        }
        Some(Self { year, month, day })
    }

    /// Returns the year component.
    pub const fn year(self) -> i32 {
        self.year
    }

    /// Returns the month component.
    pub const fn month(self) -> u8 {
        self.month
    }

    /// Returns the day component.
    pub const fn day(self) -> u8 {
        self.day
    }
}

impl CivilTime {
    /// Creates a valid wall-clock time with nanosecond precision.
    pub const fn new(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Option<Self> {
        if hour > 23 || minute > 59 || second > 59 || nanosecond >= 1_000_000_000 {
            return None;
        }
        Some(Self {
            hour,
            minute,
            second,
            nanosecond,
        })
    }

    /// Returns the hour component.
    pub const fn hour(self) -> u8 {
        self.hour
    }

    /// Returns the minute component.
    pub const fn minute(self) -> u8 {
        self.minute
    }

    /// Returns the second component.
    pub const fn second(self) -> u8 {
        self.second
    }

    /// Returns the nanosecond component.
    pub const fn nanosecond(self) -> u32 {
        self.nanosecond
    }
}

impl Instant {
    /// Creates an instant with a normalized subsecond component.
    pub const fn new(unix_seconds: i64, nanosecond: u32) -> Option<Self> {
        if nanosecond >= 1_000_000_000 {
            return None;
        }
        Some(Self {
            unix_seconds,
            nanosecond,
        })
    }

    /// Returns seconds from the Unix epoch.
    pub const fn unix_seconds(self) -> i64 {
        self.unix_seconds
    }

    /// Returns the nanosecond component.
    pub const fn nanosecond(self) -> u32 {
        self.nanosecond
    }
}

impl Locale {
    /// Creates a trusted locale handle for generated static data.
    pub const fn new_trusted(index: u32, locale_tag: &'static str) -> Self {
        Self { index, locale_tag }
    }

    /// Returns this locale's stable index in the generated locale set.
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Returns this locale's exact authored source tag.
    pub const fn as_str(&self) -> &'static str {
        self.locale_tag
    }
}

impl MessageData {
    /// Creates a trusted handle for one compiler-generated message shard.
    ///
    /// The caller must ensure that each locale selection and all table references are valid,
    /// including plural rule sets and their category branches. Rendering trusts those invariants.
    pub const fn new_trusted(shard: MessageShardData) -> Self {
        Self { shard }
    }

    /// Renders this message using the selected generated locale.
    pub fn render(
        &self,
        locale: &Locale,
        arguments: &[ArgumentValue<'_>],
    ) -> Result<String, RenderError> {
        Renderer::render_message(self, locale, arguments)
    }

    /// Renders this message to a caller-owned sink without allocating an output string.
    pub fn render_to(
        &self,
        locale: &Locale,
        arguments: &[ArgumentValue<'_>],
        output: &mut dyn Write,
    ) -> Result<(), RenderError> {
        Renderer::render_message_to(self, locale, arguments, None, output)
    }

    /// Renders this message with caller-owned locale formatting.
    pub fn render_with(
        &self,
        locale: &Locale,
        formatter: &dyn Formatter,
        arguments: &[ArgumentValue<'_>],
    ) -> Result<String, RenderError> {
        Renderer::render_message_with(self, locale, formatter, arguments)
    }

    /// Renders this message with caller-owned locale formatting into a caller-owned sink.
    pub fn render_with_to(
        &self,
        locale: &Locale,
        formatter: &dyn Formatter,
        arguments: &[ArgumentValue<'_>],
        output: &mut dyn Write,
    ) -> Result<(), RenderError> {
        Renderer::render_message_to(self, locale, arguments, Some(formatter), output)
    }

    pub(crate) fn shard(&self) -> &MessageShardData {
        &self.shard
    }
}

impl From<u64> for PluralCount {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl Display for PluralCount {
    fn fmt(&self, formatter: &mut FmtFormatter<'_>) -> FmtResult {
        Display::fmt(&self.value, formatter)
    }
}

impl Display for OrdinalCount {
    fn fmt(&self, formatter: &mut FmtFormatter<'_>) -> FmtResult {
        Display::fmt(&self.value, formatter)
    }
}

impl Display for PluralDecimal {
    fn fmt(&self, formatter: &mut FmtFormatter<'_>) -> FmtResult {
        let mut divisor = 1i128;
        for _ in 0..self.scale {
            divisor = divisor.checked_mul(10).ok_or(FmtError)?;
        }
        let integer = self.coefficient / divisor;
        let fraction = self.coefficient % divisor;
        Display::fmt(&integer, formatter)?;
        if self.scale > 0 {
            formatter.write_char('.')?;
            write!(
                formatter,
                "{fraction:0width$}",
                width = usize::from(self.scale)
            )?;
        }
        Ok(())
    }
}

impl Catalog {
    /// Creates a trusted handle for immutable tables produced by the compiler.
    ///
    /// The caller must ensure that every table reference and catalog invariant is valid,
    /// including plural rule sets and their category branches. Rendering trusts those invariants.
    pub const fn new_trusted(data: CatalogData) -> Self {
        Self { data }
    }

    /// Validates caller-provided tables and the reachable fallback chain once.
    ///
    /// Validation checks that all views share one program store, bindings are sorted and valid,
    /// every message resolves from every reachable locale, fallback links are acyclic, plural
    /// rule sets are valid, and each plural node has complete category branches.
    pub fn try_new(data: CatalogData) -> Result<Self, RenderError> {
        Renderer::validate_catalog(&data)?;
        Ok(Self { data })
    }

    /// Renders the message addressed by `message_id` using schema-order arguments.
    ///
    /// If the selected locale has no local binding, rendering searches its explicit fallback
    /// chain for the message's shared program.
    pub fn render(
        &self,
        message_id: u32,
        arguments: &[ArgumentValue<'_>],
    ) -> Result<String, RenderError> {
        Renderer::render(self, message_id, arguments)
    }

    /// Renders to a caller-owned sink without allocating an output string.
    ///
    /// A formatted message requires [`Self::render_with_to`]. A failed write can leave a partial
    /// prefix in `output`.
    pub fn render_to(
        &self,
        message_id: u32,
        arguments: &[ArgumentValue<'_>],
        output: &mut dyn Write,
    ) -> Result<(), RenderError> {
        Renderer::render_to(self, message_id, arguments, None, output)
    }

    /// Renders with caller-owned locale formatting into a caller-owned sink.
    ///
    /// A formatter or sink failure can leave a partial prefix in `output`.
    pub fn render_with_to(
        &self,
        formatter: &dyn Formatter,
        message_id: u32,
        arguments: &[ArgumentValue<'_>],
        output: &mut dyn Write,
    ) -> Result<(), RenderError> {
        Renderer::render_to(self, message_id, arguments, Some(formatter), output)
    }

    /// Renders a message using a caller-provided formatting adapter.
    pub fn render_with(
        &self,
        formatter: &dyn Formatter,
        message_id: u32,
        arguments: &[ArgumentValue<'_>],
    ) -> Result<String, RenderError> {
        Renderer::render_with(self, formatter, message_id, arguments)
    }

    pub(crate) fn data(&self) -> &CatalogData {
        &self.data
    }
}

impl RenderError {
    fn message(self) -> &'static str {
        match self {
            Self::MessageOutOfRange => "message ID is outside the catalog",
            Self::LocaleOutOfRange => "locale index is outside the message shard",
            Self::LocaleMismatch => "locale does not belong to the message shard",
            Self::ArgumentCountMismatch => "argument count does not match the message signature",
            Self::ArgumentOutOfRange => "instruction references an absent argument",
            Self::ArgumentTypeMismatch => "plural selector argument has an incompatible type",
            Self::InvalidCatalog => "catalog contains an invalid table reference",
            Self::PoundOutsidePlural => "pound instruction has no active plural selector",
            Self::FormattingFailed => "argument formatting failed",
            Self::FormatterRequired => "formatted operation requires a formatter",
            Self::FormatterFailed => "caller-provided formatter failed",
            Self::WriteFailed => "output sink rejected a write",
        }
    }
}

impl Display for RenderError {
    fn fmt(&self, formatter: &mut FmtFormatter<'_>) -> FmtResult {
        formatter.write_str(self.message())
    }
}

impl Error for RenderError {}
