mod rules;
mod tag;

use std::collections::{BTreeMap, BTreeSet};

use crate::fingerprint::Fingerprint;

use rules::RulesParser;
pub(super) use rules::{
    ConjunctionData, DecimalData, OperandData, PredicateData, RangeData, RuleData, RuleSetData,
};
use tag::{AliasData, AliasParser, Registry, RegistryParser, TagValidator};
use u18n_ast::PluralKind;

#[cfg(test)]
use u18n_ast::PluralCategory;

const IANA_SHA256: &str = "755fad43283be7b41ebe3c89ad054b6eaf928f404f9c0edb74799e0eab74beb1";
const PLURALS_SHA256: &str = "69a30c57a621de9c52ec6ca95fe7bf9f06df6f8c13ca71a72e447b532c92cff7";
const ORDINALS_SHA256: &str = "6c62fa2250f4f0150a3c7e24fc93c20ae6722f65bd26b49aaad799b40f8014b3";
const METADATA_SHA256: &str = "b06614fc2c9aab3b9a9f7e8bf9328f4f7a95d89b2d2dbb300d0caff0f885c164";
const SUPPLEMENTAL_SHA256: &str =
    "745150ec1408d50e1a272b65654a5dd645f3e60f5cc2898b576f725b28538d1a";

pub(super) struct CldrData {
    registry: Registry,
    cardinal_rules: BTreeMap<String, RuleSetData>,
    ordinal_rules: BTreeMap<String, RuleSetData>,
    main_parents: BTreeMap<String, String>,
    plural_parents: BTreeMap<String, String>,
    aliases: AliasData,
    registry_digest: String,
    cardinal_digest: String,
    ordinal_digest: String,
    aliases_digest: String,
    supplemental_digest: String,
}

#[derive(Default)]
struct ParentLocaleMaps {
    main: BTreeMap<String, String>,
    plurals: BTreeMap<String, String>,
}

struct ParentLocaleParser;

impl CldrData {
    pub(super) fn load() -> Result<Self, String> {
        let registry_source = include_str!("../data/iana-language-subtag-registry.txt");
        let plural_source = include_str!("../data/cldr-49/plurals.xml");
        let ordinal_source = include_str!("../data/cldr-49/ordinals.xml");
        let metadata_source = include_str!("../data/cldr-49/supplementalMetadata.xml");
        let supplemental_source = include_str!("../data/cldr-49/supplementalData.xml");
        let registry_digest = Fingerprint::digest(registry_source.as_bytes());
        Self::verify_digest("IANA registry", &registry_digest, IANA_SHA256)?;
        let cardinal_digest = Fingerprint::digest(plural_source.as_bytes());
        Self::verify_digest("CLDR cardinal rules", &cardinal_digest, PLURALS_SHA256)?;
        let ordinal_digest = Fingerprint::digest(ordinal_source.as_bytes());
        Self::verify_digest("CLDR ordinal rules", &ordinal_digest, ORDINALS_SHA256)?;
        let aliases_digest = Fingerprint::digest(metadata_source.as_bytes());
        Self::verify_digest("CLDR aliases", &aliases_digest, METADATA_SHA256)?;
        let supplemental_digest = Fingerprint::digest(supplemental_source.as_bytes());
        Self::verify_digest(
            "CLDR supplemental data",
            &supplemental_digest,
            SUPPLEMENTAL_SHA256,
        )?;
        let registry = RegistryParser::parse(registry_source)?;
        let aliases = AliasParser::parse(metadata_source)?;
        let cardinal_rules = RulesParser::parse(plural_source, PluralKind::Cardinal)?;
        let ordinal_rules = RulesParser::parse(ordinal_source, PluralKind::Ordinal)?;
        let parent_locales = ParentLocaleParser::parse(supplemental_source)?;
        Ok(Self {
            registry,
            cardinal_rules,
            ordinal_rules,
            main_parents: parent_locales.main,
            plural_parents: parent_locales.plurals,
            aliases,
            registry_digest,
            cardinal_digest,
            ordinal_digest,
            aliases_digest,
            supplemental_digest,
        })
    }

    pub(super) fn resolve(
        &self,
        source_tag: &str,
        kind: PluralKind,
    ) -> Result<&RuleSetData, String> {
        let subtags = TagValidator::validate(source_tag, &self.registry)?;
        let locale = Self::canonicalize(subtags, &self.aliases);
        let rules = match kind {
            PluralKind::Cardinal => &self.cardinal_rules,
            PluralKind::Ordinal => &self.ordinal_rules,
        };
        Self::find_rule_set(locale, rules, &self.plural_parents).ok_or_else(|| {
            let name = match kind {
                PluralKind::Cardinal => "cardinal",
                PluralKind::Ordinal => "ordinal",
            };
            format!("locale `{source_tag}` does not resolve to a CLDR 49 {name} rule set")
        })
    }

    fn find_rule_set<'rules>(
        mut locale: Vec<String>,
        rules: &'rules BTreeMap<String, RuleSetData>,
        parents: &BTreeMap<String, String>,
    ) -> Option<&'rules RuleSetData> {
        let mut visited = BTreeSet::new();
        loop {
            let key = locale.join("_").to_ascii_lowercase();
            if !visited.insert(key.clone()) {
                return None;
            }
            if let Some(rule_set) = rules.get(&key) {
                return Some(rule_set);
            }
            if let Some(parent) = parents.get(&key) {
                locale = parent.split('_').map(str::to_string).collect();
                continue;
            }
            if locale.len() <= 1 {
                return None;
            }
            locale.pop();
        }
    }

    pub(super) fn registry_digest(&self) -> &str {
        &self.registry_digest
    }

    pub(super) fn ordinal_digest(&self) -> &str {
        &self.ordinal_digest
    }

    pub(super) fn cardinal_digest(&self) -> &str {
        &self.cardinal_digest
    }

    pub(super) fn aliases_digest(&self) -> &str {
        &self.aliases_digest
    }

    pub(super) fn supplemental_digest(&self) -> &str {
        &self.supplemental_digest
    }

    pub(super) fn main_parents(&self) -> &BTreeMap<String, String> {
        &self.main_parents
    }

    fn verify_digest(name: &str, actual: &str, expected: &str) -> Result<(), String> {
        if actual == expected {
            Ok(())
        } else {
            Err(format!(
                "pinned {name} checksum mismatch: expected {expected}, found {actual}"
            ))
        }
    }

    fn canonicalize(mut subtags: Vec<String>, aliases: &AliasData) -> Vec<String> {
        if let Some(private_use) = subtags.iter().position(|part| part == "x") {
            subtags.truncate(private_use);
        }
        if subtags.is_empty() {
            return subtags;
        }
        for _ in 0..4 {
            let mut changed = false;
            for size in (1..=subtags.len().min(3)).rev() {
                let key = subtags.get(..size).unwrap_or(&[]).join("_");
                if let Some(replacement) = aliases.language.get(&key) {
                    let mut next = replacement.clone();
                    next.extend(subtags.get(size..).unwrap_or(&[]).iter().cloned());
                    subtags = next;
                    changed = true;
                    break;
                }
            }
            for part in subtags.iter_mut().skip(1) {
                if let Some(replacement) = aliases.script.get(part) {
                    *part = replacement.clone();
                    changed = true;
                } else if let Some(replacement) = aliases.territory.get(part) {
                    *part = replacement.clone();
                    changed = true;
                } else if let Some(replacement) = aliases.variant.get(part) {
                    *part = replacement.clone();
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        subtags
    }
}

impl ParentLocaleParser {
    fn parse(source: &str) -> Result<ParentLocaleMaps, String> {
        let mut parents = ParentLocaleMaps {
            main: BTreeMap::new(),
            plurals: BTreeMap::new(),
        };
        let mut found_main_component = false;
        let mut found_plural_component = false;
        let mut offset = 0usize;
        while let Some(relative) = source
            .get(offset..)
            .and_then(|tail| tail.find("<parentLocales"))
        {
            let start = offset + relative;
            let tag_end = source
                .get(start..)
                .and_then(|tail| tail.find('>'))
                .map(|distance| start + distance + 1)
                .ok_or_else(|| "unterminated parentLocales element".to_string())?;
            let tag = source.get(start..tag_end).unwrap_or("");
            let close = source
                .get(tag_end..)
                .and_then(|tail| tail.find("</parentLocales>"))
                .map(|distance| tag_end + distance)
                .ok_or_else(|| "unterminated parentLocales element".to_string())?;
            let component = Self::attribute(tag, "component");
            let destination = match component {
                None => {
                    found_main_component = true;
                    &mut parents.main
                }
                Some(value) if value.split_ascii_whitespace().any(|part| part == "plurals") => {
                    found_plural_component = true;
                    &mut parents.plurals
                }
                Some(_) => {
                    offset = close + "</parentLocales>".len();
                    continue;
                }
            };
            let parent_kind = if component.is_none() {
                "main"
            } else {
                "plural"
            };
            {
                let body = source.get(tag_end..close).unwrap_or("");
                Self::parse_entries(body, destination, parent_kind)?;
            }
            offset = close + "</parentLocales>".len();
        }
        if !found_main_component {
            return Err("CLDR main parent-locale component could not be parsed".to_string());
        }
        if !found_plural_component {
            return Err("CLDR plural parent-locale component could not be parsed".to_string());
        }
        Self::validate_parent_chains(&parents.main, "main")?;
        Self::validate_parent_chains(&parents.plurals, "plural")?;
        Ok(parents)
    }

    fn parse_entries(
        body: &str,
        parents: &mut BTreeMap<String, String>,
        parent_kind: &str,
    ) -> Result<(), String> {
        let mut offset = 0usize;
        while let Some(relative) = body
            .get(offset..)
            .and_then(|tail| tail.find("<parentLocale"))
        {
            let start = offset + relative;
            let tag_end = body
                .get(start..)
                .and_then(|tail| tail.find('>'))
                .map(|distance| start + distance + 1)
                .ok_or_else(|| "unterminated parentLocale element".to_string())?;
            let tag = body.get(start..tag_end).unwrap_or("");
            let parent = Self::attribute(tag, "parent")
                .ok_or_else(|| "parentLocale element has no parent attribute".to_string())?
                .to_ascii_lowercase();
            let locales = Self::attribute(tag, "locales")
                .ok_or_else(|| "parentLocale element has no locales attribute".to_string())?;
            for locale in locales.split_ascii_whitespace() {
                if parents
                    .insert(locale.to_ascii_lowercase(), parent.clone())
                    .is_some()
                {
                    return Err(format!(
                        "duplicate CLDR {parent_kind} parent locale `{locale}`"
                    ));
                }
            }
            offset = tag_end;
        }
        Ok(())
    }

    fn validate_parent_chains(
        parents: &BTreeMap<String, String>,
        parent_kind: &str,
    ) -> Result<(), String> {
        for child in parents.keys() {
            let mut candidate = child.as_str();
            let mut visited = BTreeSet::new();
            loop {
                if !visited.insert(candidate) {
                    return Err(format!(
                        "CLDR {parent_kind} parent-locale chain contains a cycle at `{candidate}`"
                    ));
                }
                if let Some(parent) = parents.get(candidate) {
                    candidate = parent;
                } else if let Some(separator) = candidate.rfind('_') {
                    candidate = candidate.get(..separator).unwrap_or("");
                } else {
                    break;
                }
            }
        }
        Ok(())
    }

    fn attribute<'source>(tag: &'source str, name: &str) -> Option<&'source str> {
        let marker = format!("{name}=\"");
        let start = tag.find(&marker)? + marker.len();
        let end = tag.get(start..)?.find('"')? + start;
        tag.get(start..end)
    }
}

#[cfg(test)]
#[test]
fn separates_main_plural_and_unrelated_parent_locales() {
    let source = r#"
        <parentLocales>
            <parentLocale parent="main" locales="aa_BB zz"/>
        </parentLocales>
        <parentLocales component="segmentations grammaticalFeatures plurals">
            <parentLocale parent="aa" locales="aa_BB cc_DD"/>
        </parentLocales>
        <parentLocales component="collations">
            <parentLocale parent="other" locales="ee_FF"/>
        </parentLocales>
    "#;
    let parents = ParentLocaleParser::parse(source).unwrap_or_default();

    assert_eq!(parents.main.get("aa_bb").map(String::as_str), Some("main"));
    assert_eq!(parents.main.get("zz").map(String::as_str), Some("main"));
    assert_eq!(parents.plurals.get("aa_bb").map(String::as_str), Some("aa"));
    assert_eq!(parents.plurals.get("cc_dd").map(String::as_str), Some("aa"));
    assert!(!parents.main.contains_key("ee_ff"));
    assert!(!parents.plurals.contains_key("ee_ff"));
}

#[cfg(test)]
#[test]
fn rejects_parent_chains_that_cycle_through_truncation() {
    let source = r#"
        <parentLocales>
            <parentLocale parent="aa_BB" locales="aa"/>
        </parentLocales>
        <parentLocales component="plurals"></parentLocales>
    "#;

    let error = ParentLocaleParser::parse(source).err().unwrap_or_default();
    assert!(error.contains("main parent-locale chain contains a cycle"));
}

#[cfg(test)]
#[test]
fn rejects_duplicate_children_within_each_parent_map() {
    let source = r#"
        <parentLocales>
            <parentLocale parent="aa" locales="bb"/>
            <parentLocale parent="cc" locales="bb"/>
        </parentLocales>
        <parentLocales component="plurals"></parentLocales>
    "#;

    let error = ParentLocaleParser::parse(source).err().unwrap_or_default();
    assert!(error.contains("duplicate CLDR main parent locale `bb`"));
}

#[cfg(test)]
#[test]
fn plural_parent_mapping_precedes_subtag_truncation() {
    let regional_categories = vec![PluralCategory::One, PluralCategory::Other];
    let root_categories = vec![PluralCategory::Other];
    let mut rules = BTreeMap::new();
    rules.insert(
        "az".to_string(),
        RuleSetData {
            kind: PluralKind::Cardinal,
            categories: regional_categories,
            rules: Vec::new(),
        },
    );
    rules.insert(
        "root".to_string(),
        RuleSetData {
            kind: PluralKind::Cardinal,
            categories: root_categories.clone(),
            rules: Vec::new(),
        },
    );
    let parents = BTreeMap::from([("az_arab".to_string(), "root".to_string())]);
    let locale = vec!["az".to_string(), "Arab".to_string()];
    let categories = CldrData::find_rule_set(locale, &rules, &parents)
        .map(|rule_set| rule_set.categories.as_slice());

    assert_eq!(categories, Some(root_categories.as_slice()));
}

#[cfg(test)]
#[test]
fn main_parent_mapping_does_not_change_plural_rule_lookup() {
    let data = CldrData::load().ok();
    let main_parent = data
        .as_ref()
        .and_then(|data| data.main_parents().get("az_arab"))
        .map(String::as_str);
    let cardinal_categories = data
        .as_ref()
        .and_then(|data| data.resolve("az-Arab", PluralKind::Cardinal).ok())
        .map(|rule_set| rule_set.categories.clone());

    assert_eq!(main_parent, Some("root"));
    assert_eq!(
        cardinal_categories,
        Some(vec![PluralCategory::One, PluralCategory::Other])
    );
}
