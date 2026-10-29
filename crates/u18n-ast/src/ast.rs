use alloc::{string::String, vec::Vec};

pub use pattern::{
    CurrencyPrecision, FormatOptions, FormatStyle, Grouping, Pattern, PatternPart, PluralBranch,
    PluralCase, PluralCategory, PluralKind, Precision, Rounding, SelectBranch,
};

/// Identifies one source file within a compilation input set.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileId(pub u32);

/// A half-open UTF-8 byte range in a source file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Span {
    pub file_id: FileId,
    pub start: u32,
    pub end: u32,
}

/// A source-spanned identifier used by schema and locale nodes.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Identifier {
    pub text: String,
    pub span: Span,
}

/// A dotted message key composed of source-spanned segments.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MessageKey {
    pub segments: Vec<Identifier>,
    pub span: Span,
}

/// The value class accepted by a schema parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ArgumentType {
    PluralCount,
    PluralDecimal,
    OrdinalCount,
    String,
    Number,
    Decimal,
    Money,
    Date,
    Time,
    Instant,
}

/// The supported schema language version.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SchemaVersion {
    V1,
}

/// A translator-facing documentation comment attached to a schema declaration.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Documentation {
    pub text: String,
    pub span: Span,
}

/// A parameter declaration with separate name, type, and declaration spans.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ParameterDef {
    pub name: Identifier,
    pub argument_type: ArgumentType,
    pub type_span: Span,
    pub documentation: Vec<Documentation>,
    pub span: Span,
}

/// A message declaration and its ordered parameter signature.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MessageDef {
    pub key: MessageKey,
    pub parameters: Vec<ParameterDef>,
    pub documentation: Vec<Documentation>,
    pub span: Span,
}

/// A parsed schema containing message signatures in source order.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SchemaAst {
    pub version: SchemaVersion,
    pub messages: Vec<MessageDef>,
    pub span: Span,
}

/// A locale's authored BCP 47 tag and its source location.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LocaleId {
    pub source_tag: String,
    pub span: Span,
}

/// A localized message pattern associated with its schema key.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LocalizedMessage {
    pub key: MessageKey,
    pub pattern: Pattern,
    pub span: Span,
}

/// A parsed locale catalog for one authored language tag.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LocaleAst {
    pub locale: LocaleId,
    pub fallback: Option<LocaleId>,
    pub locale_span: Span,
    pub messages: Vec<LocalizedMessage>,
    pub span: Span,
}

mod pattern {
    use alloc::vec::Vec;

    use super::Span;

    pub use nodes::{
        CurrencyPrecision, FormatOptions, FormatStyle, Grouping, PatternPart, PluralBranch,
        PluralCase, PluralCategory, PluralKind, Precision, Rounding, SelectBranch,
    };

    /// An ordered sequence of typed tokens in a message pattern.
    #[derive(Clone, Debug, Eq, Hash, PartialEq)]
    pub struct Pattern {
        pub parts: Vec<PatternPart>,
        pub span: Span,
    }

    mod nodes {
        use alloc::{string::String, vec::Vec};

        use super::Pattern;
        use crate::ast::{Identifier, Span};

        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Grouping {
            None,
            Auto,
        }

        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Precision {
            Input,
            Fixed(u8),
        }

        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum CurrencyPrecision {
            CurrencyDefault,
            Fixed(u8),
        }

        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Rounding {
            HalfEven,
            HalfUp,
            TowardZero,
            AwayFromZero,
        }

        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum FormatStyle {
            Short,
            Medium,
            Long,
            Full,
        }

        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum PluralCategory {
            Zero,
            One,
            Two,
            Few,
            Many,
            Other,
        }

        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum PluralKind {
            Cardinal,
            Ordinal,
        }

        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
                time_zone: String,
            },
        }

        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum PluralCase {
            Exact(u64),
            Category(PluralCategory),
        }

        /// One exact value or category and its recursively parsed pattern.
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub struct PluralBranch {
            pub case: PluralCase,
            pub case_span: Span,
            pub pattern: Pattern,
            pub span: Span,
        }

        /// One named string case and recursively parsed pattern.
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub struct SelectBranch {
            pub name: Identifier,
            pub pattern: Pattern,
            pub span: Span,
        }

        /// A literal, hard line break, interpolation, selector, or active count marker.
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub enum PatternPart {
            Text {
                value: String,
                span: Span,
            },
            /// A logical line break rendered as U+000A.
            LineBreak {
                span: Span,
            },
            Argument {
                name: Identifier,
                span: Span,
            },
            Plural {
                name: Identifier,
                kind: PluralKind,
                branches: Vec<PluralBranch>,
                span: Span,
            },
            Select {
                name: Identifier,
                branches: Vec<SelectBranch>,
                span: Span,
            },
            Format {
                name: Identifier,
                options: FormatOptions,
                span: Span,
            },
            Pound {
                span: Span,
            },
        }
    }
}
