#![no_std]

use core::{cmp::Ordering, iter::Peekable};

/// Maximum number of ASCII bytes that fit in a packed locale key.
pub const MAX_PACKED_TAG_BYTES: usize = 16;
const GRANDFATHERED_TAGS: [&str; 25] = [
    "art-lojban",
    "cel-gaulish",
    "en-gb-oed",
    "i-ami",
    "i-bnn",
    "i-default",
    "i-enochian",
    "i-hak",
    "i-klingon",
    "i-lux",
    "i-mingo",
    "i-navajo",
    "i-pwn",
    "i-tao",
    "i-tay",
    "i-tsu",
    "no-bok",
    "no-nyn",
    "sgn-be-fr",
    "sgn-be-nl",
    "sgn-ch-de",
    "zh-guoyu",
    "zh-hakka",
    "zh-min",
    "zh-min-nan",
];
const LANGUAGE_SLOT_BITS: u32 = 5;
const SCRIPT_SHIFT: u32 = 15;
const SCRIPT_SLOT_BITS: u32 = 5;
const REGION_SHIFT: u32 = 35;
const REGION_SLOT_BITS: u32 = 6;

/// A syntax or policy failure found while parsing a locale tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TagError {
    /// The input contains no characters.
    Empty,
    /// The input contains a non-ASCII character.
    NonAscii,
    /// A subtag is empty, too long, or contains a non-alphanumeric byte.
    InvalidSubtag,
    /// Subtags do not fit the BCP 47 component order.
    InvalidStructure,
    /// The tag contains an extension sequence rejected by format version 1.
    ExtensionRejected,
    /// An extension singleton has no following extension subtag.
    ExtensionMissingSubtag,
    /// A private-use marker has no following private-use subtag.
    PrivateUseMissingSubtag,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TagKind {
    Regular,
    Grandfathered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// A borrowed tag that passed the shared BCP 47 syntax and extension-policy checks.
///
/// Parsing checks syntax only. IANA registration and source casing are compiler checks.
pub struct ParsedTag<'a> {
    source: &'a str,
    kind: TagKind,
    private_use_start: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Subtag<'a> {
    value: &'a str,
    start: usize,
}

struct Subtags<'a> {
    source: &'a str,
    next_start: usize,
}

impl TagError {
    /// Returns a stable human-readable description of the parse failure.
    pub const fn message(self) -> &'static str {
        match self {
            Self::Empty => "locale tag is empty",
            Self::NonAscii => "locale tags may contain only ASCII characters",
            Self::InvalidSubtag => "locale subtags must contain 1 to 8 ASCII letters or digits",
            Self::InvalidStructure => "locale tag does not follow BCP 47 syntax",
            Self::ExtensionRejected => {
                "BCP 47 extension sequences are rejected by locale format version 1"
            }
            Self::ExtensionMissingSubtag => {
                "BCP 47 extension singleton requires one or more subtags"
            }
            Self::PrivateUseMissingSubtag => "private-use locale tags require at least one subtag",
        }
    }
}

impl<'a> ParsedTag<'a> {
    /// Parses a complete BCP 47 tag under the version 1 extension policy.
    pub fn parse(source: &'a str) -> Result<ParsedTag<'a>, TagError> {
        if source.is_empty() {
            return Err(TagError::Empty);
        }
        if !source.is_ascii() {
            return Err(TagError::NonAscii);
        }
        Self::validate_subtag_shape(source)?;
        if GRANDFATHERED_TAGS
            .iter()
            .any(|tag| source.eq_ignore_ascii_case(tag))
        {
            return Ok(ParsedTag {
                source,
                kind: TagKind::Grandfathered,
                private_use_start: None,
            });
        }
        Self::validate_language_tag(source)
    }

    /// Returns the authored request spelling without normalization.
    pub const fn as_str(&self) -> &'a str {
        self.source
    }

    /// Compares this parsed tag with another ASCII tag without allocating.
    pub fn cmp_ignore_ascii_case(&self, other: &str) -> Ordering {
        self.source
            .bytes()
            .map(|byte| byte.to_ascii_lowercase())
            .cmp(other.bytes().map(|byte| byte.to_ascii_lowercase()))
    }

    /// Reports whether the complete tag is in the registered grandfathered set.
    pub const fn is_grandfathered(&self) -> bool {
        matches!(self.kind, TagKind::Grandfathered)
    }

    /// Returns the normalized packed lookup key when the tag fits in 16 bytes.
    pub fn packed_key(&self) -> Option<u128> {
        if !self.source.is_ascii() || self.source.len() > MAX_PACKED_TAG_BYTES {
            return None;
        }

        let mut bytes = [0; MAX_PACKED_TAG_BYTES];
        for (slot, byte) in bytes.iter_mut().zip(self.source.bytes()) {
            *slot = byte.to_ascii_lowercase();
        }
        Some(u128::from_le_bytes(bytes))
    }

    /// Encodes the bounded language, script, and region form as a semantic `u64` code.
    ///
    /// Bits above the language, script, and region components are zero. Tags with components
    /// outside this layout do not have a semantic code, even when they are valid BCP 47 tags.
    pub fn semantic_code(&self) -> Option<u64> {
        if self.is_grandfathered() {
            return None;
        }

        let mut parts = Subtags::new(self.source);
        let language = parts.next()?.value;
        if !(2..=3).contains(&language.len())
            || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
        {
            return None;
        }

        let mut code = encode_letter_slots(language, LANGUAGE_SLOT_BITS)?;
        if let Some(part) = parts.next() {
            if part.value.len() == 4 && part.value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
                code |= encode_letter_slots(part.value, SCRIPT_SLOT_BITS)? << SCRIPT_SHIFT;
            } else if is_region(part.value) {
                code |= encode_region_slots(part.value)? << REGION_SHIFT;
                if parts.next().is_some() {
                    return None;
                }
            } else {
                return None;
            }
        }

        if let Some(part) = parts.next() {
            if !is_region(part.value) || parts.next().is_some() {
                return None;
            }
            code |= encode_region_slots(part.value)? << REGION_SHIFT;
        }

        Some(code)
    }

    /// Binary-searches entries sorted by their numeric packed keys.
    /// Returns `Err(entries.len())` when this tag is longer than 16 bytes.
    pub fn binary_search_by<T, F>(&self, entries: &[T], key: F) -> Result<usize, usize>
    where
        F: FnMut(&T) -> u128,
    {
        let Some(packed_key) = self.packed_key() else {
            return Err(entries.len());
        };
        entries.binary_search_by_key(&packed_key, key)
    }

    /// Returns the next valid truncation candidate, if one exists.
    ///
    /// A private-use tail is removed as one component; grandfathered tags are atomic.
    pub fn truncate(&self) -> Option<&'a str> {
        if self.is_grandfathered() {
            return None;
        }
        if let Some(private_use_start) = self.private_use_start {
            return private_use_start
                .checked_sub(1)
                .and_then(|end| self.source.get(..end));
        }
        self.source
            .rfind('-')
            .and_then(|separator| self.source.get(..separator))
    }

    fn validate_subtag_shape(source: &str) -> Result<(), TagError> {
        for part in source.split('-') {
            if part.is_empty()
                || part.len() > 8
                || !part.bytes().all(|byte| byte.is_ascii_alphanumeric())
            {
                return Err(TagError::InvalidSubtag);
            }
        }
        Ok(())
    }

    fn validate_language_tag(source: &'a str) -> Result<ParsedTag<'a>, TagError> {
        let mut parts = Subtags::new(source).peekable();
        let language = parts.next().ok_or(TagError::InvalidStructure)?;
        if !(2..=8).contains(&language.value.len())
            || !language
                .value
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic())
        {
            if language.value.eq_ignore_ascii_case("x") {
                return Self::validate_private_use(source, parts, 0);
            }
            return Err(TagError::InvalidStructure);
        }

        if language.value.len() <= 3 {
            let mut extlang_count = 0usize;
            while extlang_count < 3
                && parts.peek().is_some_and(|part| {
                    part.value.len() == 3
                        && part.value.bytes().all(|byte| byte.is_ascii_alphabetic())
                })
            {
                parts.next();
                extlang_count += 1;
            }
        }

        if parts.peek().is_some_and(|part| {
            part.value.len() == 4 && part.value.bytes().all(|byte| byte.is_ascii_alphabetic())
        }) {
            parts.next();
        }
        if parts.peek().is_some_and(|part| is_region(part.value)) {
            parts.next();
        }

        let variants_start = parts.peek().map_or(source.len(), |part| part.start);
        while parts.peek().is_some_and(|part| is_variant(part.value)) {
            let variant = parts.next().ok_or(TagError::InvalidStructure)?;
            let prior_variants = source.get(variants_start..variant.start).unwrap_or("");
            if Subtags::new(prior_variants).any(|part| {
                is_variant(part.value) && part.value.eq_ignore_ascii_case(variant.value)
            }) {
                return Err(TagError::InvalidStructure);
            }
        }

        let mut private_use_start = None;
        if let Some(next) = parts.next() {
            if next.value.eq_ignore_ascii_case("x") {
                private_use_start = Some(next.start);
                if parts.next().is_none() {
                    return Err(TagError::PrivateUseMissingSubtag);
                }
                if parts.any(|part| part.value.is_empty()) {
                    return Err(TagError::InvalidSubtag);
                }
            } else if next.value.len() == 1 {
                if parts.next().is_none() {
                    return Err(TagError::ExtensionMissingSubtag);
                }
                return Err(TagError::ExtensionRejected);
            } else {
                return Err(TagError::InvalidStructure);
            }
        }

        Ok(ParsedTag {
            source,
            kind: TagKind::Regular,
            private_use_start,
        })
    }

    fn validate_private_use(
        source: &'a str,
        mut parts: Peekable<Subtags<'a>>,
        private_use_start: usize,
    ) -> Result<ParsedTag<'a>, TagError> {
        if parts.next().is_none() {
            return Err(TagError::PrivateUseMissingSubtag);
        }
        Ok(ParsedTag {
            source,
            kind: TagKind::Regular,
            private_use_start: Some(private_use_start),
        })
    }
}

impl<'a> Subtags<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            next_start: 0,
        }
    }
}

impl<'a> Iterator for Subtags<'a> {
    type Item = Subtag<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_start >= self.source.len() {
            return None;
        }
        let start = self.next_start;
        let tail = self.source.get(start..)?;
        let end = tail
            .find('-')
            .map_or(self.source.len(), |offset| start + offset);
        let value = self.source.get(start..end)?;
        self.next_start = end.saturating_add(1);
        Some(Subtag { value, start })
    }
}

fn is_region(value: &str) -> bool {
    (value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic()))
        || (value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_variant(value: &str) -> bool {
    (5..=8).contains(&value.len())
        || (value.len() == 4 && value.as_bytes().first().is_some_and(u8::is_ascii_digit))
}

fn encode_letter_slots(value: &str, slot_bits: u32) -> Option<u64> {
    let mut code = 0u64;
    for (slot, byte) in value.bytes().enumerate() {
        if !byte.is_ascii_alphabetic() {
            return None;
        }
        let normalized = byte.to_ascii_lowercase();
        let value = u64::from(normalized.checked_sub(b'a')?.checked_add(1)?);
        code |= value.checked_shl(slot_bits.checked_mul(u32::try_from(slot).ok()?)?)?;
    }
    Some(code)
}

fn encode_region_slots(value: &str) -> Option<u64> {
    let mut code = 0u64;
    for (slot, byte) in value.bytes().enumerate() {
        let value = if byte.is_ascii_alphabetic() {
            u64::from(
                byte.to_ascii_lowercase()
                    .checked_sub(b'a')?
                    .checked_add(1)?,
            )
        } else if byte.is_ascii_digit() {
            u64::from(byte.checked_sub(b'0')?.checked_add(27)?)
        } else {
            return None;
        };
        let shift = REGION_SLOT_BITS.checked_mul(u32::try_from(slot).ok()?)?;
        code |= value.checked_shl(shift)?;
    }
    Some(code)
}
