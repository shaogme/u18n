use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{Error as FmtError, Write},
};

use u18n_locale_tag::ParsedTag;

use crate::codegen::lower::LoweredLocale;

struct LocaleIndexRecord {
    tag: String,
    normalized_tag: String,
    variant: String,
}

struct ParentIndexRecord {
    key: u64,
}

struct ParentLocaleVariant {
    name: String,
    tag: String,
    discriminant: u8,
}

struct ParentLocaleSpec {
    name: &'static str,
    tag: &'static str,
    discriminant: u8,
}

struct PreparedLocaleTags {
    variants: Vec<String>,
    locale_index: Vec<LocaleIndexRecord>,
    parent_variants: Vec<ParentLocaleVariant>,
    parent_index: Vec<ParentIndexRecord>,
    default_variant: String,
}

const CLDR_MAIN_PARENT_CHILD_COUNT: usize = 198;
const PARENT_LOCALE_BITS: u32 = 3;
const PARENT_LOCALE_MASK: u64 = (1 << PARENT_LOCALE_BITS) - 1;
const PARENT_LOCALE_SPECS: [ParentLocaleSpec; 8] = [
    ParentLocaleSpec {
        name: "Root",
        tag: "root",
        discriminant: 0,
    },
    ParentLocaleSpec {
        name: "En001",
        tag: "en-001",
        discriminant: 1,
    },
    ParentLocaleSpec {
        name: "En150",
        tag: "en-150",
        discriminant: 2,
    },
    ParentLocaleSpec {
        name: "Es419",
        tag: "es-419",
        discriminant: 3,
    },
    ParentLocaleSpec {
        name: "No",
        tag: "no",
        discriminant: 4,
    },
    ParentLocaleSpec {
        name: "PtPt",
        tag: "pt-PT",
        discriminant: 5,
    },
    ParentLocaleSpec {
        name: "ZhHantHk",
        tag: "zh-Hant-HK",
        discriminant: 6,
    },
    ParentLocaleSpec {
        name: "EnIn",
        tag: "en-IN",
        discriminant: 7,
    },
];

pub(super) struct LocaleTagEmitter;

impl LocaleTagEmitter {
    pub(super) fn emit(
        output: &mut String,
        locales: &[LoweredLocale],
        main_parents: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        let prepared = Self::prepare(locales, main_parents)?;
        Self::emit_prepared(output, locales, &prepared)
            .map_err(|_| "could not format generated locale tag source".to_string())
    }

    fn prepare(
        locales: &[LoweredLocale],
        main_parents: &BTreeMap<String, String>,
    ) -> Result<PreparedLocaleTags, String> {
        let variants = Self::locale_variants(locales)?;
        let locale_index = Self::locale_index(locales, &variants)?;
        Self::validate_main_parent_data(main_parents)?;
        let (parent_variants, parent_index) = Self::parent_index(main_parents)?;
        let default_variant = Self::default_variant(locales, &variants)
            .ok_or_else(|| "generated locale set is empty".to_string())?
            .to_string();
        Ok(PreparedLocaleTags {
            variants,
            locale_index,
            parent_variants,
            parent_index,
            default_variant,
        })
    }

    fn emit_prepared(
        output: &mut String,
        locales: &[LoweredLocale],
        prepared: &PreparedLocaleTags,
    ) -> Result<(), FmtError> {
        writeln!(output)?;
        writeln!(output, "#[derive(Clone, Copy, Debug, Eq, PartialEq)]")?;
        writeln!(
            output,
            "/// A locale included in the generated catalog set."
        )?;
        writeln!(output, "pub enum LocaleTag {{")?;
        for variant in &prepared.variants {
            writeln!(output, "    /// The `{}` locale.", variant)?;
            writeln!(output, "    {variant},")?;
        }
        writeln!(output, "}}")?;
        writeln!(output)?;
        writeln!(
            output,
            "const PARENT_LOCALE_BITS: u32 = {PARENT_LOCALE_BITS};"
        )?;
        writeln!(
            output,
            "const PARENT_LOCALE_MASK: u64 = (1 << PARENT_LOCALE_BITS) - 1;"
        )?;
        writeln!(output)?;
        writeln!(output, "struct LocaleIndexEntry {{")?;
        writeln!(output, "    tag: &'static str,")?;
        writeln!(output, "    locale: LocaleTag,")?;
        writeln!(output, "}}")?;
        writeln!(output)?;
        Self::emit_parent_locale(output, &prepared.parent_variants)?;
        writeln!(output)?;
        writeln!(output, "struct ParentIndexEntry {{")?;
        writeln!(output, "    key: u64,")?;
        writeln!(output, "}}")?;
        writeln!(output)?;
        Self::emit_locale_index(output, &prepared.locale_index)?;
        Self::emit_parent_index(output, &prepared.parent_index)?;
        writeln!(output)?;
        Self::emit_locale_tag_methods(
            output,
            locales,
            &prepared.variants,
            &prepared.default_variant,
        )?;
        writeln!(output)?;
        Self::emit_locale_tag_parser(output)?;
        Ok(())
    }

    fn emit_parent_locale(
        output: &mut String,
        variants: &[ParentLocaleVariant],
    ) -> Result<(), FmtError> {
        writeln!(output, "#[derive(Clone, Copy, Debug, Eq, PartialEq)]")?;
        writeln!(output, "#[repr(u8)]")?;
        writeln!(output, "enum ParentLocale {{")?;
        for variant in variants {
            writeln!(output, "    {} = {},", variant.name, variant.discriminant)?;
        }
        writeln!(output, "}}")?;
        writeln!(output)?;
        writeln!(output, "impl ParentLocale {{")?;
        writeln!(output, "    fn as_str(&self) -> &'static str {{")?;
        writeln!(output, "        match self {{")?;
        for variant in variants {
            writeln!(
                output,
                "            Self::{} => {:?},",
                variant.name, variant.tag
            )?;
        }
        writeln!(output, "        }}")?;
        writeln!(output, "    }}")?;
        writeln!(output)?;
        writeln!(
            output,
            "    fn from_discriminant(discriminant: u64) -> Option<Self> {{"
        )?;
        writeln!(output, "        match discriminant {{")?;
        for variant in variants {
            writeln!(
                output,
                "            {} => Some(Self::{}),",
                variant.discriminant, variant.name
            )?;
        }
        writeln!(output, "            _ => None,")?;
        writeln!(output, "        }}")?;
        writeln!(output, "    }}")?;
        writeln!(output, "}}")?;
        Ok(())
    }

    fn locale_variants(locales: &[LoweredLocale]) -> Result<Vec<String>, String> {
        let mut seen = BTreeMap::new();
        let mut variants = Vec::with_capacity(locales.len());
        for locale in locales {
            let variant = Self::variant_name(&locale.locale_tag)?;
            if let Some(original) = seen.insert(variant.clone(), locale.locale_tag.clone()) {
                return Err(format!(
                    "generated LocaleTag variant `{variant}` collides for `{original}` and `{}`",
                    locale.locale_tag
                ));
            }
            variants.push(variant);
        }
        Ok(variants)
    }

    fn variant_name(tag: &str) -> Result<String, String> {
        let mut variant = String::new();
        for subtag in tag.split('-') {
            if subtag.is_empty()
                || subtag.len() > 8
                || !subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
            {
                return Err(format!(
                    "cannot name a generated variant for locale tag `{tag}`"
                ));
            }
            for (position, byte) in subtag.bytes().enumerate() {
                let normalized = if position == 0 {
                    byte.to_ascii_uppercase()
                } else {
                    byte.to_ascii_lowercase()
                };
                variant.push(char::from(normalized));
            }
        }
        Ok(variant)
    }

    fn locale_index(
        locales: &[LoweredLocale],
        variants: &[String],
    ) -> Result<Vec<LocaleIndexRecord>, String> {
        if locales.len() != variants.len() {
            return Err("generated locale variants do not match the locale set".to_string());
        }
        let mut entries = locales
            .iter()
            .zip(variants)
            .map(|(locale, variant)| {
                ParsedTag::parse(&locale.locale_tag).map_err(|_| {
                    format!(
                        "cannot index invalid generated locale tag `{}`",
                        locale.locale_tag
                    )
                })?;
                let normalized_tag = locale.locale_tag.to_ascii_lowercase();
                Ok(LocaleIndexRecord {
                    tag: locale.locale_tag.clone(),
                    normalized_tag,
                    variant: variant.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        entries.sort_by(|left, right| left.normalized_tag.cmp(&right.normalized_tag));
        if entries.windows(2).any(|pair| {
            pair.first()
                .zip(pair.get(1))
                .is_some_and(|(left, right)| left.normalized_tag == right.normalized_tag)
        }) {
            return Err("generated locale tags have duplicate normalized lookup tags".to_string());
        }
        Ok(entries)
    }

    fn parent_index(
        main_parents: &BTreeMap<String, String>,
    ) -> Result<(Vec<ParentLocaleVariant>, Vec<ParentIndexRecord>), String> {
        let variants = PARENT_LOCALE_SPECS
            .iter()
            .map(|spec| {
                if spec.discriminant == 0 {
                    if spec.tag != "root" {
                        return Err(format!(
                            "fixed root parent tag must be `root`, found `{}`",
                            spec.tag
                        ));
                    }
                } else {
                    ParsedTag::parse(spec.tag)
                        .ok()
                        .and_then(|tag| tag.semantic_code())
                        .ok_or_else(|| {
                            format!(
                                "fixed CLDR parent tag `{}` cannot be encoded as a semantic code",
                                spec.tag
                            )
                        })?;
                };
                Ok(ParentLocaleVariant {
                    name: spec.name.to_string(),
                    tag: spec.tag.to_string(),
                    discriminant: spec.discriminant,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let variant_by_tag = variants
            .iter()
            .map(|variant| (Self::normalize_cldr_tag(&variant.tag), variant))
            .collect::<BTreeMap<_, _>>();
        let mut entries = main_parents
            .iter()
            .map(|(child, parent)| {
                let child_tag = Self::normalize_cldr_tag(child);
                let parent_tag = Self::normalize_cldr_tag(parent);
                let parent_variant = variant_by_tag
                    .get(&parent_tag)
                    .ok_or_else(|| format!("unknown CLDR main-parent value `{parent_tag}`"))?;
                let child_key = Self::semantic_cldr_key(&child_tag, "child")?;
                Ok(ParentIndexRecord {
                    key: Self::parent_index_key(child_key, parent_variant.discriminant)
                        .ok_or_else(|| {
                            format!(
                                "CLDR main-parent child `{child_tag}` and parent `{parent_tag}` do not fit the packed key"
                            )
                        })?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        entries.sort_by_key(|entry| entry.key);
        if entries.windows(2).any(|pair| {
            pair.first().zip(pair.get(1)).is_some_and(|(left, right)| {
                left.key >> PARENT_LOCALE_BITS == right.key >> PARENT_LOCALE_BITS
            })
        }) {
            return Err("CLDR main-parent data has duplicate semantic child codes".to_string());
        }
        Ok((variants, entries))
    }

    fn validate_main_parent_data(main_parents: &BTreeMap<String, String>) -> Result<(), String> {
        if main_parents.len() != CLDR_MAIN_PARENT_CHILD_COUNT {
            return Err(format!(
                "pinned CLDR main-parent map has {} child tags; expected {CLDR_MAIN_PARENT_CHILD_COUNT}",
                main_parents.len()
            ));
        }
        let parent_tags = main_parents
            .values()
            .map(|tag| Self::normalize_cldr_tag(tag))
            .collect::<BTreeSet<_>>();
        let expected_parent_tags = PARENT_LOCALE_SPECS
            .iter()
            .map(|spec| Self::normalize_cldr_tag(spec.tag))
            .collect::<BTreeSet<_>>();
        if parent_tags != expected_parent_tags {
            return Err(
                "pinned CLDR main-parent map does not contain the fixed eight parent tags"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn default_variant<'a>(locales: &[LoweredLocale], variants: &'a [String]) -> Option<&'a str> {
        if locales.len() != variants.len() {
            return None;
        }
        let index = locales
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.locale_tag.cmp(&right.locale_tag))
            .map(|(index, _)| index)?;
        variants.get(index).map(String::as_str)
    }

    fn normalize_cldr_tag(tag: &str) -> String {
        tag.replace('_', "-").to_ascii_lowercase()
    }

    fn semantic_cldr_key(tag: &str, role: &str) -> Result<u64, String> {
        ParsedTag::parse(tag)
            .ok()
            .and_then(|parsed| parsed.semantic_code())
            .ok_or_else(|| {
                format!(
                    "CLDR main-parent {role} tag `{tag}` is outside the semantic language/script/region profile"
                )
            })
    }

    fn parent_index_key(child_key: u64, parent_discriminant: u8) -> Option<u64> {
        let parent_discriminant = u64::from(parent_discriminant);
        if parent_discriminant > PARENT_LOCALE_MASK {
            return None;
        }
        child_key
            .checked_mul(1 << PARENT_LOCALE_BITS)?
            .checked_add(parent_discriminant)
    }

    fn emit_locale_index(
        output: &mut String,
        entries: &[LocaleIndexRecord],
    ) -> Result<(), FmtError> {
        writeln!(output, "static LOCALE_INDEX: &[LocaleIndexEntry] = &[")?;
        for entry in entries {
            writeln!(
                output,
                "    LocaleIndexEntry {{ tag: {:?}, locale: LocaleTag::{} }},",
                entry.tag, entry.variant
            )?;
        }
        writeln!(output, "];")?;
        Ok(())
    }

    fn emit_parent_index(
        output: &mut String,
        entries: &[ParentIndexRecord],
    ) -> Result<(), FmtError> {
        writeln!(output, "static PARENT_INDEX: &[ParentIndexEntry] = &[")?;
        for entry in entries {
            writeln!(output, "    ParentIndexEntry {{ key: {}u64 }},", entry.key)?;
        }
        writeln!(output, "];")?;
        Ok(())
    }

    fn emit_locale_tag_methods(
        output: &mut String,
        locales: &[LoweredLocale],
        variants: &[String],
        default_variant: &str,
    ) -> Result<(), FmtError> {
        if locales.len() != variants.len() {
            return Err(FmtError);
        }
        writeln!(output, "impl LocaleTag {{")?;
        writeln!(
            output,
            "    /// Returns this locale's canonical language tag."
        )?;
        writeln!(output, "    pub const fn as_str(&self) -> &'static str {{")?;
        writeln!(output, "        match self {{")?;
        for (locale, variant) in locales.iter().zip(variants) {
            writeln!(
                output,
                "            Self::{variant} => {:?},",
                locale.locale_tag
            )?;
        }
        writeln!(output, "        }}")?;
        writeln!(output, "    }}")?;
        writeln!(output)?;
        writeln!(
            output,
            "    /// Returns this locale's generated runtime handle."
        )?;
        writeln!(
            output,
            "    pub const fn as_locale(&self) -> &'static Locale {{"
        )?;
        writeln!(output, "        match self {{")?;
        for (locale, variant) in locales.iter().zip(variants) {
            writeln!(
                output,
                "            Self::{variant} => &catalogs::{}::LOCALE,",
                locale.module
            )?;
        }
        writeln!(output, "        }}")?;
        writeln!(output, "    }}")?;
        writeln!(output)?;
        writeln!(
            output,
            "    /// Resolves a locale tag from one of the generated catalogs."
        )?;
        writeln!(
            output,
            "    pub fn from_catalog(catalog: &'static Catalog) -> Result<Self, &'static str> {{"
        )?;
        for (locale, variant) in locales.iter().zip(variants) {
            writeln!(
                output,
                "        if ptr::eq(catalog, &catalogs::{}::CATALOG) {{",
                locale.module
            )?;
            writeln!(output, "            return Ok(Self::{variant});")?;
            writeln!(output, "        }}")?;
        }
        writeln!(
            output,
            "        Err(\"catalog is not part of this generated locale set\")"
        )?;
        writeln!(output, "    }}")?;
        writeln!(output)?;
        writeln!(
            output,
            "    /// Returns this locale's complete generated catalog."
        )?;
        writeln!(
            output,
            "    pub const fn as_catalog(&self) -> &'static Catalog {{"
        )?;
        writeln!(output, "        match self {{")?;
        for (locale, variant) in locales.iter().zip(variants) {
            writeln!(
                output,
                "            Self::{variant} => &catalogs::{}::CATALOG,",
                locale.module
            )?;
        }
        writeln!(output, "        }}")?;
        writeln!(output, "    }}")?;
        writeln!(output)?;
        Self::emit_lookup_methods(output, default_variant)?;
        writeln!(output, "}}")?;
        Ok(())
    }

    fn emit_lookup_methods(output: &mut String, default_variant: &str) -> Result<(), FmtError> {
        writeln!(
            output,
            "    fn find_source_locale(tag: &ParsedTag<'_>) -> Option<Self> {{"
        )?;
        writeln!(
            output,
            "        LOCALE_INDEX.binary_search_by(|entry| tag.cmp_ignore_ascii_case(entry.tag).reverse())"
        )?;
        writeln!(output, "            .ok()")?;
        writeln!(
            output,
            "            .and_then(|position| LOCALE_INDEX.get(position))"
        )?;
        writeln!(output, "            .map(|entry| entry.locale)")?;
        writeln!(output, "    }}")?;
        writeln!(output)?;
        writeln!(
            output,
            "    fn cldr_parent(tag: &ParsedTag<'_>) -> Option<ParentLocale> {{"
        )?;
        writeln!(output, "        let child_key = tag.semantic_code()?;")?;
        writeln!(output, "        let position = PARENT_INDEX")?;
        writeln!(
            output,
            "            .binary_search_by_key(&child_key, |entry| entry.key >> PARENT_LOCALE_BITS)"
        )?;
        writeln!(output, "            .ok()?;")?;
        writeln!(output, "        let entry = PARENT_INDEX.get(position)?;")?;
        writeln!(
            output,
            "        ParentLocale::from_discriminant(entry.key & PARENT_LOCALE_MASK)"
        )?;
        writeln!(output, "    }}")?;
        writeln!(output)?;
        writeln!(
            output,
            "    /// Resolves an unsupported or regional tag through generated locale fallbacks."
        )?;
        writeln!(
            output,
            "    pub fn fallback(locale_tag: &str) -> Result<Self, &'static str> {{"
        )?;
        writeln!(
            output,
            "        let mut candidate = ParsedTag::parse(locale_tag)"
        )?;
        writeln!(
            output,
            "            .map_err(|_| \"invalid or policy-rejected BCP 47 locale tag\")?;"
        )?;
        writeln!(output, "        loop {{")?;
        writeln!(
            output,
            "            if let Some(locale) = Self::find_source_locale(&candidate) {{"
        )?;
        writeln!(output, "                return Ok(locale);")?;
        writeln!(output, "            }}")?;
        writeln!(output, "            if candidate.is_grandfathered() {{")?;
        writeln!(output, "                break;")?;
        writeln!(output, "            }}")?;
        writeln!(
            output,
            "            if let Some(parent) = Self::cldr_parent(&candidate).and_then(|parent| ParsedTag::parse(parent.as_str()).ok()) {{"
        )?;
        writeln!(output, "                candidate = parent;")?;
        writeln!(output, "                continue;")?;
        writeln!(output, "            }}")?;
        writeln!(
            output,
            "            if let Some(prefix) = candidate.truncate().and_then(|prefix| ParsedTag::parse(prefix).ok()) {{"
        )?;
        writeln!(output, "                candidate = prefix;")?;
        writeln!(output, "                continue;")?;
        writeln!(output, "            }}")?;
        writeln!(output, "            break;")?;
        writeln!(output, "        }}")?;
        writeln!(output, "        Ok(Self::{default_variant})")?;
        writeln!(output, "    }}")?;
        Ok(())
    }

    fn emit_locale_tag_parser(output: &mut String) -> Result<(), FmtError> {
        writeln!(output, "impl FromStr for LocaleTag {{")?;
        writeln!(output, "    type Err = &'static str;")?;
        writeln!(
            output,
            "    fn from_str(locale_tag: &str) -> Result<Self, Self::Err> {{"
        )?;
        writeln!(output, "        let tag = ParsedTag::parse(locale_tag)")?;
        writeln!(
            output,
            "            .map_err(|_| \"invalid or policy-rejected BCP 47 locale tag\")?;"
        )?;
        writeln!(
            output,
            "        Self::find_source_locale(&tag).ok_or(\"unsupported locale tag\")"
        )?;
        writeln!(output, "    }}")?;
        writeln!(output, "}}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use u18n_locale_tag::ParsedTag;

    use crate::codegen::lower::{LoweredLocale, MessageBindingData};

    use super::LocaleTagEmitter;

    fn lowered_locale(locale_tag: &str) -> LoweredLocale {
        LoweredLocale {
            locale_index: 0,
            module: String::new(),
            locale_tag: locale_tag.to_string(),
            fallback_module: None,
            bindings: Vec::<MessageBindingData>::new(),
        }
    }

    #[test]
    fn variant_names_concatenate_normalized_subtags() {
        assert_eq!(LocaleTagEmitter::variant_name("en"), Ok("En".to_string()));
        assert_eq!(
            LocaleTagEmitter::variant_name("es-419"),
            Ok("Es419".to_string())
        );
        assert_eq!(
            LocaleTagEmitter::variant_name("en-x-private"),
            Ok("EnXPrivate".to_string())
        );
        assert_eq!(
            LocaleTagEmitter::variant_name("zh-Hant-HK"),
            Ok("ZhHantHk".to_string())
        );
    }

    #[test]
    fn locale_variant_collisions_fail_with_the_source_tags() {
        let locales = [lowered_locale("en-1-2"), lowered_locale("en-12")];
        let error = LocaleTagEmitter::locale_variants(&locales)
            .expect_err("ambiguous variant names must be rejected");

        assert_eq!(
            error,
            "generated LocaleTag variant `En12` collides for `en-1-2` and `en-12`"
        );
    }

    #[test]
    fn parent_index_uses_the_fixed_parent_variants() {
        let parents = BTreeMap::from([
            ("en".to_string(), "root".to_string()),
            ("fa".to_string(), "en-001".to_string()),
        ]);
        let (variants, entries) = LocaleTagEmitter::parent_index(&parents)
            .expect("fixed parent variants should be represented");

        assert_eq!(variants.len(), 8);
        assert_eq!(
            variants.first().map(|variant| variant.name.as_str()),
            Some("Root")
        );
        assert_eq!(variants.get(1).map(|variant| variant.discriminant), Some(1));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries.first().map(|entry| entry.key & 7), Some(1));
        assert_eq!(entries.get(1).map(|entry| entry.key & 7), Some(0));
    }

    #[test]
    fn unknown_parent_values_are_rejected() {
        let parents = BTreeMap::from([("en".to_string(), "fr".to_string())]);
        let error = LocaleTagEmitter::parent_index(&parents)
            .err()
            .expect("unknown parent values must be rejected");

        assert!(error.contains("unknown CLDR main-parent value `fr`"));
    }

    #[test]
    fn unrepresentable_child_tags_are_rejected() {
        let parents = BTreeMap::from([("en-x-private".to_string(), "root".to_string())]);
        let error = LocaleTagEmitter::parent_index(&parents)
            .err()
            .expect("private-use child tags are outside the semantic profile");

        assert!(error.contains("outside the semantic language/script/region profile"));
    }

    #[test]
    fn duplicate_semantic_child_codes_are_rejected() {
        let parents = BTreeMap::from([
            ("en".to_string(), "root".to_string()),
            ("EN".to_string(), "root".to_string()),
        ]);
        let error = LocaleTagEmitter::parent_index(&parents)
            .err()
            .expect("case variants must collide as semantic child codes");

        assert!(error.contains("duplicate semantic child codes"));
    }

    #[test]
    fn parent_index_sorts_children_by_numeric_semantic_code_and_packs_parents() {
        let parents = BTreeMap::from([
            ("en".to_string(), "root".to_string()),
            ("fa".to_string(), "root".to_string()),
        ]);
        let (_, entries) = LocaleTagEmitter::parent_index(&parents)
            .expect("valid main-parent data should be packed");

        assert_eq!(
            entries.first().map(|entry| entry.key >> 3),
            ParsedTag::parse("fa")
                .ok()
                .and_then(|tag| tag.semantic_code())
        );
        assert_eq!(
            entries.get(1).map(|entry| entry.key >> 3),
            ParsedTag::parse("en")
                .ok()
                .and_then(|tag| tag.semantic_code())
        );
    }
}
