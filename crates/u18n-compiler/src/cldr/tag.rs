use std::collections::{BTreeMap, BTreeSet};
use u18n_locale_tag::ParsedTag;

pub(super) struct Registry {
    languages: BTreeSet<String>,
    language_ranges: Vec<(String, String)>,
    extlangs: BTreeMap<String, Vec<String>>,
    scripts: BTreeSet<String>,
    script_ranges: Vec<(String, String)>,
    regions: BTreeSet<String>,
    region_ranges: Vec<(String, String)>,
    variants: BTreeMap<String, Vec<String>>,
    grandfathered: BTreeSet<String>,
}

pub(crate) struct AliasData {
    pub(crate) language: BTreeMap<String, Vec<String>>,
    pub(crate) script: BTreeMap<String, String>,
    pub(crate) territory: BTreeMap<String, String>,
    pub(crate) variant: BTreeMap<String, String>,
}

pub(super) struct RegistryParser;

impl RegistryParser {
    pub(super) fn parse(source: &str) -> Result<Registry, String> {
        let mut registry = Registry {
            languages: BTreeSet::new(),
            language_ranges: Vec::new(),
            extlangs: BTreeMap::new(),
            scripts: BTreeSet::new(),
            script_ranges: Vec::new(),
            regions: BTreeSet::new(),
            region_ranges: Vec::new(),
            variants: BTreeMap::new(),
            grandfathered: BTreeSet::new(),
        };
        for record in source.split("%%") {
            let mut kind = "";
            let mut subtag = None;
            let mut tag = None;
            let mut prefixes = Vec::new();
            for line in record.lines() {
                if let Some((field, value)) = line.split_once(':') {
                    match field.trim() {
                        "Type" => kind = value.trim(),
                        "Subtag" => subtag = Some(value.trim().to_ascii_lowercase()),
                        "Tag" => tag = Some(value.trim().to_ascii_lowercase()),
                        "Prefix" => prefixes.push(value.trim().to_ascii_lowercase()),
                        _ => {}
                    }
                }
            }
            match kind {
                "language" => Self::insert_subtag(
                    &mut registry.languages,
                    &mut registry.language_ranges,
                    subtag,
                ),
                "extlang" => {
                    if let Some(value) = subtag {
                        registry.extlangs.insert(value, prefixes);
                    }
                }
                "script" => {
                    Self::insert_subtag(&mut registry.scripts, &mut registry.script_ranges, subtag)
                }
                "region" => {
                    Self::insert_subtag(&mut registry.regions, &mut registry.region_ranges, subtag)
                }
                "variant" => {
                    if let Some(value) = subtag {
                        registry.variants.insert(value, prefixes);
                    }
                }
                "grandfathered" => Self::insert(&mut registry.grandfathered, tag),
                _ => {}
            }
        }
        if registry.languages.is_empty() || registry.grandfathered.is_empty() {
            return Err("IANA registry snapshot could not be parsed".to_string());
        }
        Ok(registry)
    }

    fn insert(values: &mut BTreeSet<String>, value: Option<String>) {
        if let Some(value) = value {
            values.insert(value);
        }
    }

    fn insert_subtag(
        values: &mut BTreeSet<String>,
        ranges: &mut Vec<(String, String)>,
        value: Option<String>,
    ) {
        if let Some(value) = value {
            if let Some((start, end)) = value.split_once("..") {
                ranges.push((start.to_string(), end.to_string()));
            } else {
                values.insert(value);
            }
        }
    }

    fn registered(values: &BTreeSet<String>, ranges: &[(String, String)], candidate: &str) -> bool {
        values.contains(candidate)
            || ranges
                .iter()
                .any(|(start, end)| start.as_str() <= candidate && candidate <= end.as_str())
    }
}

pub(super) struct AliasParser;

impl AliasParser {
    pub(super) fn parse(source: &str) -> Result<AliasData, String> {
        let mut aliases = AliasData {
            language: BTreeMap::new(),
            script: BTreeMap::new(),
            territory: BTreeMap::new(),
            variant: BTreeMap::new(),
        };
        for (element, destination) in [
            ("languageAlias", 0u8),
            ("scriptAlias", 1),
            ("territoryAlias", 2),
            ("variantAlias", 3),
        ] {
            for tag in Self::elements(source, element) {
                let source_key = Self::attribute(tag, "type")
                    .ok_or_else(|| format!("missing type attribute on {element}"))?;
                let replacement = Self::attribute(tag, "replacement")
                    .ok_or_else(|| format!("missing replacement attribute on {element}"))?;
                let key = source_key.to_ascii_lowercase().replace('-', "_");
                let replacement_parts = replacement
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .split(['_', '-'])
                    .map(str::to_ascii_lowercase)
                    .collect::<Vec<_>>();
                match destination {
                    0 => {
                        aliases.language.insert(key, replacement_parts);
                    }
                    1 => {
                        if let Some(value) = replacement_parts.first() {
                            aliases.script.insert(key, value.clone());
                        }
                    }
                    2 => {
                        if let Some(value) = replacement_parts.first() {
                            aliases.territory.insert(key, value.clone());
                        }
                    }
                    _ => {
                        if let Some(value) = replacement_parts.first() {
                            aliases.variant.insert(key, value.clone());
                        }
                    }
                }
            }
        }
        Ok(aliases)
    }

    fn elements<'a>(source: &'a str, element: &str) -> Vec<&'a str> {
        let needle = format!("<{element} ");
        let closing = format!("</{element}>");
        let mut found = Vec::new();
        let mut offset = 0usize;
        while let Some(relative) = source.get(offset..).and_then(|tail| tail.find(&needle)) {
            let start = offset + relative;
            let Some(end_relative) = source.get(start..).and_then(|tail| tail.find('>')) else {
                break;
            };
            let end = start + end_relative + 1;
            found.push(source.get(start..end).unwrap_or(""));
            offset = end;
            if !closing.is_empty()
                && source
                    .get(offset..)
                    .is_some_and(|tail| tail.starts_with(&closing))
            {
                offset += closing.len();
            }
        }
        found
    }

    fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
        let marker = format!("{name}=\"");
        let start = tag.find(&marker)? + marker.len();
        let end = tag.get(start..)?.find('"')? + start;
        tag.get(start..end)
    }
}

pub(super) struct TagValidator;

impl TagValidator {
    pub(super) fn validate(source: &str, registry: &Registry) -> Result<Vec<String>, String> {
        ParsedTag::parse(source).map_err(|error| error.message().to_string())?;
        let subtags = source.split('-').collect::<Vec<_>>();
        if subtags.is_empty()
            || subtags.iter().any(|part| {
                part.is_empty()
                    || part.len() > 8
                    || !part.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
        {
            return Err("locale tag has an invalid BCP 47 subtag shape".to_string());
        }
        let lowercase = source.to_ascii_lowercase();
        if registry.grandfathered.contains(&lowercase) {
            Self::check_grandfathered_case(&subtags)?;
            return Ok(subtags
                .iter()
                .map(|part| part.to_ascii_lowercase())
                .collect());
        }
        if subtags
            .first()
            .is_some_and(|part| part.eq_ignore_ascii_case("x"))
        {
            if subtags.len() < 2 {
                return Err("private-use locale tags require at least one subtag".to_string());
            }
            Self::require_lowercase(&subtags, 0, subtags.len())?;
            return Ok(subtags
                .iter()
                .map(|part| part.to_ascii_lowercase())
                .collect());
        }
        Self::validate_language_tag(&subtags, registry)
    }

    fn validate_language_tag(subtags: &[&str], registry: &Registry) -> Result<Vec<String>, String> {
        let language = subtags
            .first()
            .ok_or_else(|| "missing language subtag".to_string())?;
        if !language.bytes().all(|byte| byte.is_ascii_alphabetic())
            || !(2..=8).contains(&language.len())
        {
            return Err("primary language subtag does not follow BCP 47 syntax".to_string());
        }
        Self::require_lowercase(subtags, 0, 1)?;
        if !RegistryParser::registered(
            &registry.languages,
            &registry.language_ranges,
            &language.to_ascii_lowercase(),
        ) {
            return Err(format!("unregistered language subtag `{language}`"));
        }
        let mut position = 1usize;
        let mut prefix = language.to_ascii_lowercase();
        if (2..=3).contains(&language.len()) {
            for _ in 0..3 {
                let Some(extlang) = subtags.get(position).copied() else {
                    break;
                };
                if extlang.len() != 3 || !extlang.bytes().all(|byte| byte.is_ascii_alphabetic()) {
                    break;
                }
                Self::require_lowercase(subtags, position, position + 1)?;
                let lower = extlang.to_ascii_lowercase();
                let prefixes = registry
                    .extlangs
                    .get(&lower)
                    .ok_or_else(|| format!("unregistered extlang subtag `{extlang}`"))?;
                if !prefixes.contains(&prefix) {
                    return Err(format!(
                        "extlang subtag `{extlang}` is not registered for prefix `{prefix}`"
                    ));
                }
                prefix.push('-');
                prefix.push_str(&lower);
                position += 1;
            }
        }
        if subtags.get(position).is_some_and(|part| {
            part.len() == 4 && part.bytes().all(|byte| byte.is_ascii_alphabetic())
        }) {
            let script = subtags.get(position).copied().unwrap_or("");
            if script != Self::titlecase(script) {
                return Err(format!("script subtag `{script}` must use title case"));
            }
            if !RegistryParser::registered(
                &registry.scripts,
                &registry.script_ranges,
                &script.to_ascii_lowercase(),
            ) {
                return Err(format!("unregistered script subtag `{script}`"));
            }
            position += 1;
        }
        if subtags
            .get(position)
            .is_some_and(|part| Self::is_region(part))
        {
            let region = subtags.get(position).copied().unwrap_or("");
            if region.bytes().any(|byte| byte.is_ascii_alphabetic())
                && region != region.to_ascii_uppercase()
            {
                return Err(format!(
                    "alphabetic region subtag `{region}` must be upper case"
                ));
            }
            if !RegistryParser::registered(
                &registry.regions,
                &registry.region_ranges,
                &region.to_ascii_lowercase(),
            ) {
                return Err(format!("unregistered region subtag `{region}`"));
            }
            position += 1;
        }
        let mut variants = BTreeSet::new();
        while subtags
            .get(position)
            .is_some_and(|part| Self::is_variant(part))
        {
            let variant = subtags.get(position).copied().unwrap_or("");
            Self::require_lowercase(subtags, position, position + 1)?;
            let lower = variant.to_ascii_lowercase();
            if !variants.insert(lower.clone()) {
                return Err(format!("variant subtag `{variant}` is duplicated"));
            }
            let prefixes = registry
                .variants
                .get(&lower)
                .ok_or_else(|| format!("unregistered variant subtag `{variant}`"))?;
            let preceding_subtags = subtags.get(..position).unwrap_or(&[]);
            if !prefixes.is_empty()
                && !prefixes
                    .iter()
                    .any(|prefix| Self::matches_variant_prefix(preceding_subtags, prefix))
            {
                return Err(format!(
                    "variant subtag `{variant}` does not match any registered IANA Prefix"
                ));
            }
            position += 1;
        }
        if position < subtags.len() {
            let next = subtags.get(position).copied().unwrap_or("");
            if next.eq_ignore_ascii_case("x") {
                let private_use_marker = position;
                position += 1;
                if position == subtags.len() {
                    return Err(
                        "private-use locale sequence requires at least one subtag".to_string()
                    );
                }
                Self::require_lowercase(subtags, private_use_marker, subtags.len())?;
                position = subtags.len();
            } else if next.len() == 1 {
                return Err(format!(
                    "BCP 47 extension sequence `{next}` is rejected by locale format version 1"
                ));
            } else {
                return Err(format!("subtag `{next}` is out of order or malformed"));
            }
        }
        if position != subtags.len() {
            return Err("locale tag contains an invalid trailing subtag".to_string());
        }
        Ok(subtags
            .iter()
            .map(|part| part.to_ascii_lowercase())
            .collect())
    }

    fn require_lowercase(subtags: &[&str], start: usize, end: usize) -> Result<(), String> {
        for part in subtags.get(start..end).unwrap_or(&[]) {
            if *part != part.to_ascii_lowercase() {
                return Err(format!("subtag `{part}` must use lower case"));
            }
        }
        Ok(())
    }

    fn is_region(value: &str) -> bool {
        (value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic()))
            || (value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_digit()))
    }

    fn is_variant(value: &str) -> bool {
        (5..=8).contains(&value.len())
            || (value.len() == 4 && value.as_bytes().first().is_some_and(u8::is_ascii_digit))
    }

    fn matches_variant_prefix(subtags: &[&str], prefix: &str) -> bool {
        if prefix.is_empty() {
            return false;
        }
        let mut subtags = subtags.iter();
        for expected in prefix.split('-') {
            let Some(actual) = subtags.next() else {
                return false;
            };
            if !actual.eq_ignore_ascii_case(expected) {
                return false;
            }
        }
        true
    }

    fn titlecase(value: &str) -> String {
        let mut chars = value.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
        })
    }

    fn check_grandfathered_case(subtags: &[&str]) -> Result<(), String> {
        for (position, part) in subtags.iter().enumerate() {
            let expected = if position > 0
                && part.len() == 2
                && part.bytes().all(|byte| byte.is_ascii_alphabetic())
            {
                part.to_ascii_uppercase()
            } else if position > 0
                && part.len() == 4
                && part.bytes().all(|byte| byte.is_ascii_alphabetic())
            {
                Self::titlecase(part)
            } else {
                part.to_ascii_lowercase()
            };
            if **part != expected {
                return Err(format!(
                    "grandfathered locale tag has non-recommended casing in `{part}`"
                ));
            }
        }
        Ok(())
    }
}
