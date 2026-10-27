use std::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};

use u18n_ast::{ArgumentType, LocaleAst, MessageDef, MessageKey, PluralKind, Span};

use crate::cldr::RuleSetData;

pub(crate) use linked_parts::{LinkedPatternPart, LinkedPluralBranch, LinkedSelectBranch};

pub(crate) struct LocaleInput {
    pub(crate) path: String,
    pub(crate) source: String,
    pub(crate) ast: LocaleAst,
}

pub(crate) struct LocaleParseStatus {
    pub(crate) had_errors: bool,
    pub(crate) unavailable_tags: BTreeSet<String>,
}

#[derive(Clone)]
pub(crate) struct LinkedPattern {
    pub(crate) parts: Vec<LinkedPatternPart>,
    pub(crate) span: Span,
}

#[derive(Clone)]
pub(crate) struct LinkedMessage {
    pub(crate) key: MessageKey,
    pub(crate) pattern: LinkedPattern,
    pub(crate) author_locale: String,
}

pub(crate) struct ResolvedMessage {
    pub(crate) key: MessageKey,
    pub(crate) message: Option<LinkedMessage>,
}

pub(crate) struct LinkedLocale {
    pub(crate) ast: LocaleAst,
    pub(crate) messages: Vec<ResolvedMessage>,
}

pub(crate) struct LinkedCatalog {
    pub(crate) messages: Vec<MessageDef>,
    pub(crate) locales: Vec<LinkedLocale>,
    pub(crate) rule_sets: Vec<RuleSetData>,
}

pub(super) struct ParameterBinding {
    pub(super) slot: u32,
    pub(super) argument_type: ArgumentType,
    pub(super) type_span: Span,
}

pub(super) struct LocaleWork<'a> {
    pub(super) input: &'a LocaleInput,
    pub(super) fallback: Option<String>,
    pub(super) fallback_span: Option<Span>,
    pub(super) rules: BTreeMap<PluralKind, RuleSetData>,
    pub(super) rule_set_indices: BTreeMap<PluralKind, u32>,
    pub(super) rule_errors: BTreeMap<PluralKind, String>,
    pub(super) messages: BTreeMap<String, LinkedMessage>,
}

mod linked_parts {
    use std::{string::String, vec::Vec};

    use u18n_ast::{FormatOptions, PluralCase, PluralKind, Span};

    use super::LinkedPattern;

    #[derive(Clone)]
    pub(crate) struct LinkedPluralBranch {
        pub(crate) case: PluralCase,
        pub(crate) pattern: LinkedPattern,
        pub(crate) span: Span,
    }

    #[derive(Clone)]
    pub(crate) struct LinkedSelectBranch {
        pub(crate) value: String,
        pub(crate) pattern: LinkedPattern,
        pub(crate) span: Span,
    }

    #[derive(Clone)]
    pub(crate) enum LinkedPatternPart {
        Text {
            value: String,
            span: Span,
        },
        Argument {
            slot: u32,
            span: Span,
        },
        Plural {
            slot: u32,
            kind: PluralKind,
            rule_set_index: u32,
            branches: Vec<LinkedPluralBranch>,
            span: Span,
        },
        Select {
            slot: u32,
            branches: Vec<LinkedSelectBranch>,
            span: Span,
        },
        Format {
            slot: u32,
            options: FormatOptions,
            span: Span,
        },
        Pound {
            span: Span,
        },
    }
}
