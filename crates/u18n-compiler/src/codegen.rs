mod emit;
mod lower;
mod pool;

use std::collections::{BTreeMap, BTreeSet};

use u18n_ast::{
    CurrencyPrecision, FormatOptions, FormatStyle, Grouping, LocaleAst, MessageKey, PluralCase,
    PluralCategory, PluralKind, Precision, Rounding,
};

use crate::{
    cldr::{ConjunctionData, OperandData, PredicateData, RangeData, RuleData, RuleSetData},
    semantic::{LinkedCatalog, LinkedLocale, LinkedPattern, LinkedPatternPart},
};

use emit::{EmissionInput, SourceEmitter};
use lower::{
    LocaleProgramData, LowerBuilder, LoweredLocale, LoweredMessageShard, LoweredProgramStore,
    MessageBindingData, MessageProgramData, MessageSignatureData,
};
use pool::StringPool;

pub(super) struct CodeGenerator;

pub(crate) struct GeneratedSource {
    pub(crate) source: String,
    pub(crate) literal_fragments: usize,
    pub(crate) pool_bytes: usize,
    pub(crate) overlap_optimization_enabled: bool,
    pub(crate) keyed_literal_fragments: usize,
    pub(crate) keyed_pool_bytes: usize,
    pub(crate) keyed_shard_count: usize,
    pub(crate) keyed_optimized_shards: usize,
}

#[derive(Clone, Copy)]
pub(super) struct GenerationConfig<'a> {
    pub(super) pool_limit: usize,
    pub(super) atomic_locale: bool,
    pub(super) final_fallback_locale: Option<&'a str>,
}

pub(super) struct CodegenInput<'a> {
    pub(super) catalog: &'a LinkedCatalog,
    pub(super) fingerprint: &'a str,
    pub(super) registry_digest: &'a str,
    pub(super) cardinal_digest: &'a str,
    pub(super) ordinal_digest: &'a str,
    pub(super) aliases_digest: &'a str,
    pub(super) supplemental_digest: &'a str,
    pub(super) main_parents: &'a BTreeMap<String, String>,
    pub(super) generation: GenerationConfig<'a>,
}

struct InternedPrograms {
    patterns: BTreeMap<String, LinkedPattern>,
    program_ids: BTreeMap<String, u32>,
    rules: Vec<RuleSetData>,
    rule_indices: BTreeMap<u32, u32>,
    source_rules: Vec<RuleSetData>,
}

impl CodeGenerator {
    pub(super) fn generate(input: CodegenInput<'_>) -> Result<GeneratedSource, String> {
        let CodegenInput {
            catalog,
            fingerprint,
            registry_digest,
            cardinal_digest,
            ordinal_digest,
            aliases_digest,
            supplemental_digest,
            main_parents,
            generation,
        } = input;
        let fragments = Self::collect_fragments(catalog);
        let formatted_keys = catalog
            .locales
            .iter()
            .flat_map(|locale| locale.messages.iter())
            .filter_map(|resolved| resolved.message.as_ref())
            .filter(|message| Self::has_format(&message.pattern))
            .map(|message| Self::key_text(&message.key))
            .collect::<BTreeSet<_>>();
        let pool = StringPool::build(fragments, generation.pool_limit)?;
        let interned = Self::intern_programs(catalog)?;
        let signatures = Self::message_signatures(catalog)?;
        let store = Self::lower_programs(&interned, &pool, signatures)?;
        let message_shards = Self::lower_message_shards(catalog, generation.pool_limit)?;
        let mut locales = Vec::new();
        for (locale_index, locale) in catalog.locales.iter().enumerate() {
            locales.push(Self::lower_locale(
                locale,
                &interned,
                Self::checked_index(locale_index)?,
            )?);
        }
        let final_fallback = generation
            .final_fallback_locale
            .map(|tag| {
                locales
                    .iter()
                    .find(|locale| locale.locale_tag == tag)
                    .ok_or_else(|| {
                        format!("validated final fallback locale `{tag}` was not lowered")
                    })
            })
            .transpose()?;
        let output = SourceEmitter::emit(EmissionInput {
            catalog,
            fingerprint,
            registry_digest,
            cardinal_digest,
            ordinal_digest,
            aliases_digest,
            supplemental_digest,
            main_parents,
            generation,
            locales: &locales,
            store: &store,
            message_shards: &message_shards,
            final_fallback,
            text_pool: pool.value(),
            formatted_keys: &formatted_keys,
        })?;
        Ok(GeneratedSource {
            source: output,
            literal_fragments: pool.fragment_count(),
            pool_bytes: pool.value().len(),
            overlap_optimization_enabled: pool.overlap_optimization_enabled(),
            keyed_literal_fragments: message_shards
                .iter()
                .map(|shard| shard.literal_fragments)
                .sum(),
            keyed_pool_bytes: message_shards
                .iter()
                .map(|shard| shard.text_pool.len())
                .sum(),
            keyed_shard_count: message_shards.len(),
            keyed_optimized_shards: message_shards
                .iter()
                .filter(|shard| shard.overlap_optimization_enabled)
                .count(),
        })
    }

    fn intern_programs(catalog: &LinkedCatalog) -> Result<InternedPrograms, String> {
        Self::intern_patterns(
            catalog
                .locales
                .iter()
                .flat_map(|locale| locale.messages.iter())
                .filter_map(|resolved| resolved.message.as_ref())
                .map(|message| &message.pattern),
            &catalog.rule_sets,
        )
    }

    fn intern_patterns<'a>(
        patterns: impl IntoIterator<Item = &'a LinkedPattern>,
        source_rules: &[RuleSetData],
    ) -> Result<InternedPrograms, String> {
        let mut interned_patterns = BTreeMap::new();
        for pattern in patterns {
            let key = Self::pattern_key(pattern, source_rules)?;
            interned_patterns
                .entry(key)
                .or_insert_with(|| pattern.clone());
        }
        let mut referenced_rules = BTreeSet::new();
        for pattern in interned_patterns.values() {
            Self::collect_rule_sets(pattern, &mut referenced_rules);
        }
        let mut canonical_rules = BTreeMap::new();
        let mut source_rule_keys = BTreeMap::new();
        for source_index in &referenced_rules {
            let position = usize::try_from(*source_index)
                .map_err(|_| "plural rule-set index exceeds the host index limit".to_string())?;
            let rule_set = source_rules
                .get(position)
                .ok_or_else(|| "linked plural references an unavailable rule set".to_string())?;
            let key = Self::rule_set_key(rule_set);
            canonical_rules
                .entry(key.clone())
                .or_insert_with(|| rule_set.clone());
            source_rule_keys.insert(*source_index, key);
        }
        let rule_ids = canonical_rules
            .keys()
            .enumerate()
            .map(|(index, key)| Self::checked_index(index).map(|id| (key.clone(), id)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let rule_indices = source_rule_keys
            .into_iter()
            .map(|(source_index, key)| {
                rule_ids
                    .get(&key)
                    .copied()
                    .map(|index| (source_index, index))
                    .ok_or_else(|| "interned plural rule set is missing".to_string())
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let program_ids = interned_patterns
            .keys()
            .enumerate()
            .map(|(index, key)| Self::checked_index(index).map(|id| (key.clone(), id)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(InternedPrograms {
            patterns: interned_patterns,
            program_ids,
            rules: canonical_rules.into_values().collect(),
            rule_indices,
            source_rules: source_rules.to_vec(),
        })
    }

    fn lower_programs(
        interned: &InternedPrograms,
        pool: &StringPool,
        signatures: Vec<MessageSignatureData>,
    ) -> Result<LoweredProgramStore, String> {
        let mut builder = LowerBuilder::new(interned.rule_indices.clone());
        let mut programs = Vec::new();
        for pattern in interned.patterns.values() {
            let start = Self::checked_index(builder.instruction_count())?;
            builder.compile_pattern(pattern, pool)?;
            let end = Self::checked_index(builder.instruction_count())?;
            programs.push(MessageProgramData {
                instruction_start: start,
                instruction_end: end,
            });
            builder.drain_selector_tasks(pool)?;
        }
        Ok(builder.finish(signatures, programs, interned.rules.clone()))
    }

    fn message_signatures(catalog: &LinkedCatalog) -> Result<Vec<MessageSignatureData>, String> {
        catalog
            .messages
            .iter()
            .map(|message| {
                Ok(MessageSignatureData {
                    argument_count: Self::checked_index(message.parameters.len())?,
                })
            })
            .collect()
    }

    fn lower_message_shards(
        catalog: &LinkedCatalog,
        pool_limit: usize,
    ) -> Result<Vec<LoweredMessageShard>, String> {
        let mut shards = Vec::with_capacity(catalog.messages.len());
        for (message_id, definition) in catalog.messages.iter().enumerate() {
            let resolved = catalog
                .locales
                .iter()
                .map(|locale| {
                    let message = locale
                        .messages
                        .get(message_id)
                        .and_then(|resolved| resolved.message.as_ref())
                        .ok_or_else(|| {
                            format!(
                                "resolved message `{}` is missing from locale `{}`",
                                Self::key_text(&definition.key),
                                locale.ast.locale.source_tag
                            )
                        })?;
                    if Self::key_text(&message.key) != Self::key_text(&definition.key) {
                        return Err(format!(
                            "resolved message order differs for locale `{}`",
                            locale.ast.locale.source_tag
                        ));
                    }
                    Ok(message)
                })
                .collect::<Result<Vec<_>, String>>()?;
            let interned = Self::intern_patterns(
                resolved.iter().map(|message| &message.pattern),
                &catalog.rule_sets,
            )?;
            let mut fragments = BTreeSet::new();
            for message in &resolved {
                Self::collect_pattern_fragments(&message.pattern, &mut fragments);
            }
            fragments.remove("");
            let pool = StringPool::build(fragments, pool_limit)?;
            let argument_count = Self::checked_index(definition.parameters.len())?;
            let signature = MessageSignatureData { argument_count };
            let store = Self::lower_programs(&interned, &pool, vec![signature])?;
            let locale_programs = catalog
                .locales
                .iter()
                .zip(&resolved)
                .map(|(locale, message)| {
                    let key = Self::pattern_key(&message.pattern, &interned.source_rules)?;
                    let program_id = interned
                        .program_ids
                        .get(&key)
                        .copied()
                        .ok_or_else(|| "resolved message has no keyed program".to_string())?;
                    Ok(LocaleProgramData {
                        locale_tag: locale.ast.locale.source_tag.clone(),
                        program_id,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            shards.push(LoweredMessageShard {
                message_id: Self::checked_index(message_id)?,
                argument_count,
                locale_programs,
                store,
                text_pool: pool.value().to_string(),
                literal_fragments: pool.fragment_count(),
                overlap_optimization_enabled: pool.overlap_optimization_enabled(),
            });
        }
        Ok(shards)
    }

    fn lower_locale(
        locale: &LinkedLocale,
        interned: &InternedPrograms,
        locale_index: u32,
    ) -> Result<LoweredLocale, String> {
        let locale_tag = locale.ast.locale.source_tag.clone();
        let mut bindings = Vec::new();
        for (message_id, resolved) in locale.messages.iter().enumerate() {
            let Some(message) = &resolved.message else {
                continue;
            };
            if message.author_locale != locale_tag {
                continue;
            }
            let key = Self::pattern_key(&message.pattern, &interned.source_rules)?;
            let program_id = interned
                .program_ids
                .get(&key)
                .copied()
                .ok_or_else(|| "authored message has no interned program".to_string())?;
            bindings.push(MessageBindingData {
                message_id: Self::checked_index(message_id)?,
                program_id,
            });
        }
        let fallback_module = locale.ast.fallback.as_ref().map(|fallback| {
            format!(
                "locale_{}",
                fallback.source_tag.to_ascii_lowercase().replace('-', "_")
            )
        });
        Ok(LoweredLocale {
            locale_index,
            module: Self::locale_module(&locale.ast),
            locale_tag,
            fallback_module,
            bindings,
        })
    }

    fn pattern_key(pattern: &LinkedPattern, rules: &[RuleSetData]) -> Result<String, String> {
        let mut output = String::new();
        Self::encode_pattern(pattern, rules, &mut output)?;
        Ok(output)
    }

    fn encode_pattern(
        pattern: &LinkedPattern,
        rules: &[RuleSetData],
        output: &mut String,
    ) -> Result<(), String> {
        output.push('P');
        Self::push_number(output, pattern.parts.len());
        for part in &pattern.parts {
            match part {
                LinkedPatternPart::Text { value, .. } => {
                    output.push('T');
                    Self::push_atom(output, value);
                }
                LinkedPatternPart::LineBreak { .. } => output.push('B'),
                LinkedPatternPart::Argument { slot, .. } => {
                    output.push('A');
                    Self::push_number(output, *slot);
                }
                LinkedPatternPart::Pound { .. } => output.push('H'),
                LinkedPatternPart::Plural {
                    slot,
                    kind,
                    rule_set_index,
                    branches,
                    ..
                } => {
                    output.push('L');
                    Self::push_number(output, *slot);
                    output.push(Self::plural_kind_code(*kind));
                    let rule_index = usize::try_from(*rule_set_index)
                        .map_err(|_| "plural rule-set index exceeds host limit".to_string())?;
                    let rule_set = rules.get(rule_index).ok_or_else(|| {
                        "linked plural references an unavailable rule set".to_string()
                    })?;
                    Self::push_atom(output, &Self::rule_set_key(rule_set));
                    let mut ordered = branches.iter().collect::<Vec<_>>();
                    ordered.sort_by_key(|branch| Self::case_rank(branch.case));
                    Self::push_number(output, ordered.len());
                    for branch in ordered {
                        Self::push_case(output, branch.case);
                        Self::encode_pattern(&branch.pattern, rules, output)?;
                    }
                }
                LinkedPatternPart::Select { slot, branches, .. } => {
                    output.push('S');
                    Self::push_number(output, *slot);
                    let mut ordered = branches.iter().collect::<Vec<_>>();
                    ordered.sort_by(|left, right| left.value.cmp(&right.value));
                    Self::push_number(output, ordered.len());
                    for branch in ordered {
                        Self::push_atom(output, &branch.value);
                        Self::encode_pattern(&branch.pattern, rules, output)?;
                    }
                }
                LinkedPatternPart::Format { slot, options, .. } => {
                    output.push('F');
                    Self::push_number(output, *slot);
                    Self::push_atom(output, &Self::format_options_key(options));
                }
            }
        }
        Ok(())
    }

    fn rule_set_key(rule_set: &RuleSetData) -> String {
        let mut output = String::new();
        output.push(Self::plural_kind_code(rule_set.kind));
        let mut categories = rule_set.categories.to_vec();
        categories.sort_by_key(|category| Self::category_code(*category));
        Self::push_number(&mut output, categories.len());
        for category in categories {
            output.push(char::from(b'0' + Self::category_code(category)));
        }
        let mut rules = rule_set
            .rules
            .iter()
            .map(|rule| (Self::category_code(rule.category), Self::rule_key(rule)))
            .collect::<Vec<_>>();
        rules.sort();
        Self::push_number(&mut output, rules.len());
        for (category, key) in rules {
            output.push(char::from(b'0' + category));
            Self::push_atom(&mut output, &key);
        }
        output
    }

    fn rule_key(rule: &RuleData) -> String {
        let mut alternatives = rule
            .alternatives
            .iter()
            .map(Self::conjunction_key)
            .collect::<Vec<_>>();
        alternatives.sort();
        let mut output = String::new();
        Self::push_number(&mut output, alternatives.len());
        for alternative in alternatives {
            Self::push_atom(&mut output, &alternative);
        }
        output
    }

    fn conjunction_key(conjunction: &ConjunctionData) -> String {
        let mut predicates = conjunction
            .predicates
            .iter()
            .map(Self::predicate_key)
            .collect::<Vec<_>>();
        predicates.sort();
        let mut output = String::new();
        Self::push_number(&mut output, predicates.len());
        for predicate in predicates {
            Self::push_atom(&mut output, &predicate);
        }
        output
    }

    fn predicate_key(predicate: &PredicateData) -> String {
        let mut ranges = predicate
            .ranges
            .iter()
            .map(Self::range_key)
            .collect::<Vec<_>>();
        ranges.sort();
        let mut output = String::new();
        output.push(Self::operand_code(predicate.operand));
        match predicate.modulus {
            Some(value) => {
                output.push('M');
                Self::push_number(&mut output, value);
            }
            None => output.push('N'),
        }
        output.push(if predicate.negated { '1' } else { '0' });
        output.push(if predicate.within { '1' } else { '0' });
        Self::push_number(&mut output, ranges.len());
        for range in ranges {
            Self::push_atom(&mut output, &range);
        }
        output
    }

    fn range_key(range: &RangeData) -> String {
        format!(
            "{}:{}:{}:{}",
            range.start.coefficient, range.start.scale, range.end.coefficient, range.end.scale
        )
    }

    fn collect_rule_sets(pattern: &LinkedPattern, referenced: &mut BTreeSet<u32>) {
        for part in &pattern.parts {
            match part {
                LinkedPatternPart::Plural {
                    rule_set_index,
                    branches,
                    ..
                } => {
                    referenced.insert(*rule_set_index);
                    for branch in branches {
                        Self::collect_rule_sets(&branch.pattern, referenced);
                    }
                }
                LinkedPatternPart::Select { branches, .. } => {
                    for branch in branches {
                        Self::collect_rule_sets(&branch.pattern, referenced);
                    }
                }
                LinkedPatternPart::Text { .. }
                | LinkedPatternPart::LineBreak { .. }
                | LinkedPatternPart::Argument { .. }
                | LinkedPatternPart::Format { .. }
                | LinkedPatternPart::Pound { .. } => {}
            }
        }
    }

    fn collect_fragments(catalog: &LinkedCatalog) -> BTreeSet<String> {
        let mut fragments = BTreeSet::new();
        for locale in &catalog.locales {
            for message in &locale.messages {
                if let Some(message) = &message.message {
                    Self::collect_pattern_fragments(&message.pattern, &mut fragments);
                }
            }
        }
        fragments.remove("");
        fragments
    }

    fn collect_pattern_fragments(pattern: &LinkedPattern, fragments: &mut BTreeSet<String>) {
        for part in &pattern.parts {
            match part {
                LinkedPatternPart::Text { value, .. } => {
                    if !value.is_empty() {
                        fragments.insert(value.clone());
                    }
                }
                LinkedPatternPart::Plural { branches, .. } => {
                    for branch in branches {
                        Self::collect_pattern_fragments(&branch.pattern, fragments);
                    }
                }
                LinkedPatternPart::Select { branches, .. } => {
                    for branch in branches {
                        if !branch.value.is_empty() {
                            fragments.insert(branch.value.clone());
                        }
                        Self::collect_pattern_fragments(&branch.pattern, fragments);
                    }
                }
                LinkedPatternPart::Argument { .. }
                | LinkedPatternPart::LineBreak { .. }
                | LinkedPatternPart::Format { .. }
                | LinkedPatternPart::Pound { .. } => {}
            }
        }
    }

    fn has_format(pattern: &LinkedPattern) -> bool {
        pattern.parts.iter().any(|part| match part {
            LinkedPatternPart::Format { .. } => true,
            LinkedPatternPart::Plural { branches, .. } => branches
                .iter()
                .any(|branch| Self::has_format(&branch.pattern)),
            LinkedPatternPart::Select { branches, .. } => branches
                .iter()
                .any(|branch| Self::has_format(&branch.pattern)),
            LinkedPatternPart::Text { .. }
            | LinkedPatternPart::LineBreak { .. }
            | LinkedPatternPart::Argument { .. }
            | LinkedPatternPart::Pound { .. } => false,
        })
    }

    fn format_options_key(options: &FormatOptions) -> String {
        match options {
            FormatOptions::Decimal {
                grouping,
                precision,
                rounding,
            } => format!(
                "D{}:{}:{}",
                match grouping {
                    Grouping::None => 'N',
                    Grouping::Auto => 'A',
                },
                match precision {
                    Precision::Input => "I".to_string(),
                    Precision::Fixed(digits) => format!("F{digits}"),
                },
                Self::rounding_code(*rounding)
            ),
            FormatOptions::Currency {
                precision,
                rounding,
            } => format!(
                "C{}:{}",
                match precision {
                    CurrencyPrecision::CurrencyDefault => "C".to_string(),
                    CurrencyPrecision::Fixed(digits) => format!("F{digits}"),
                },
                Self::rounding_code(*rounding)
            ),
            FormatOptions::Date { style } => format!("d{}", Self::style_code(*style)),
            FormatOptions::Time { style } => format!("t{}", Self::style_code(*style)),
            FormatOptions::Instant { style, time_zone } => {
                let mut key = format!("i{}", Self::style_code(*style));
                Self::push_atom(&mut key, time_zone);
                key
            }
        }
    }

    fn case_rank(case: PluralCase) -> (u8, u64) {
        match case {
            PluralCase::Exact(value) => (0, value),
            PluralCase::Category(category) => (1 + Self::category_code(category), 0),
        }
    }

    fn push_case(output: &mut String, case: PluralCase) {
        match case {
            PluralCase::Exact(value) => {
                output.push('E');
                Self::push_number(output, value);
            }
            PluralCase::Category(category) => {
                output.push('C');
                output.push(char::from(b'0' + Self::category_code(category)));
            }
        }
    }

    fn plural_kind_code(kind: PluralKind) -> char {
        match kind {
            PluralKind::Cardinal => 'C',
            PluralKind::Ordinal => 'O',
        }
    }

    fn category_code(category: PluralCategory) -> u8 {
        match category {
            PluralCategory::Zero => 0,
            PluralCategory::One => 1,
            PluralCategory::Two => 2,
            PluralCategory::Few => 3,
            PluralCategory::Many => 4,
            PluralCategory::Other => 5,
        }
    }

    fn operand_code(operand: OperandData) -> char {
        match operand {
            OperandData::N => 'n',
            OperandData::I => 'i',
            OperandData::V => 'v',
            OperandData::W => 'w',
            OperandData::F => 'f',
            OperandData::T => 't',
            OperandData::C => 'c',
            OperandData::E => 'e',
        }
    }

    fn rounding_code(rounding: Rounding) -> char {
        match rounding {
            Rounding::HalfEven => 'e',
            Rounding::HalfUp => 'u',
            Rounding::TowardZero => 't',
            Rounding::AwayFromZero => 'a',
        }
    }

    fn style_code(style: FormatStyle) -> char {
        match style {
            FormatStyle::Short => 's',
            FormatStyle::Medium => 'm',
            FormatStyle::Long => 'l',
            FormatStyle::Full => 'f',
        }
    }

    fn push_atom(output: &mut String, value: &str) {
        output.push_str(&value.len().to_string());
        output.push(':');
        output.push_str(value);
    }

    fn push_number(output: &mut String, value: impl ToString) {
        Self::push_atom(output, &value.to_string());
    }

    fn checked_index(value: usize) -> Result<u32, String> {
        u32::try_from(value).map_err(|_| "generated table exceeds the u32 index limit".to_string())
    }

    fn locale_module(locale: &LocaleAst) -> String {
        format!(
            "locale_{}",
            locale
                .locale
                .source_tag
                .to_ascii_lowercase()
                .replace('-', "_")
        )
    }

    fn key_text(key: &MessageKey) -> String {
        key.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}
