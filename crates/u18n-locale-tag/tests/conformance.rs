use core::cmp::Ordering;

use u18n_locale_tag::{MAX_PACKED_TAG_BYTES, ParsedTag, TagError};

struct KeyRecord {
    key: u128,
}

struct SemanticCase {
    tag: &'static str,
    uppercase: &'static str,
}

fn key_record(tag: &str) -> Option<KeyRecord> {
    let parsed = ParsedTag::parse(tag).ok()?;
    Some(KeyRecord {
        key: parsed.packed_key()?,
    })
}

#[test]
fn accepts_regular_private_use_and_grandfathered_tags() {
    for tag in [
        "en",
        "zh-cmn-Hans-CN",
        "es-419",
        "de-CH-1901",
        "en-x-customer-a",
        "x-internal-build",
        "i-klingon",
        "sgn-BE-FR",
    ] {
        assert!(ParsedTag::parse(tag).is_ok(), "{tag}");
    }
}

#[test]
fn rejects_malformed_shape_duplicate_variants_and_extensions() {
    for tag in ["", "-en", "en-", "en--US", "en_US", "en-abcdefghi"] {
        assert!(ParsedTag::parse(tag).is_err(), "{tag}");
    }
    assert_eq!(
        ParsedTag::parse("en-u-ca"),
        Err(TagError::ExtensionRejected)
    );
    assert_eq!(
        ParsedTag::parse("en-a"),
        Err(TagError::ExtensionMissingSubtag)
    );
    assert!(ParsedTag::parse("de-1901-1901").is_err());
}

#[test]
fn packed_keys_are_ascii_case_insensitive_little_endian_and_bounded() {
    let parsed = ParsedTag::parse("ES-419").expect("well-formed tag");
    let expected = u128::from_le_bytes([
        b'e', b's', b'-', b'4', b'1', b'9', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    assert_eq!(parsed.packed_key(), Some(expected));
    assert!(
        ParsedTag::parse("en-x-abcdefg-abc")
            .ok()
            .and_then(|tag| tag.packed_key())
            .is_some()
    );
    assert_eq!(MAX_PACKED_TAG_BYTES, 16);
    assert_eq!(
        ParsedTag::parse("en-x-abcdefgh-abc")
            .ok()
            .and_then(|tag| tag.packed_key()),
        None
    );
    assert_eq!(ParsedTag::parse("français"), Err(TagError::NonAscii));
}

#[test]
fn packed_binary_search_uses_numeric_key_order() {
    let mut entries = [
        key_record("en").expect("valid packed tag"),
        key_record("fa").expect("valid packed tag"),
    ];
    entries.sort_by_key(|entry| entry.key);
    let fa = ParsedTag::parse("fa").expect("tag").packed_key();
    let en = ParsedTag::parse("en").expect("tag").packed_key();
    assert!(fa < en);
    assert_eq!(
        ParsedTag::parse("EN")
            .expect("tag")
            .binary_search_by(&entries, |entry| entry.key),
        Ok(1)
    );
    assert_eq!(
        ParsedTag::parse("fa")
            .expect("tag")
            .binary_search_by(&entries, |entry| entry.key),
        Ok(0)
    );
}

#[test]
fn binary_search_handles_empty_singleton_boundaries_and_larger_indexes() {
    let empty: Vec<KeyRecord> = Vec::new();
    let query = ParsedTag::parse("en").expect("tag");
    assert_eq!(query.binary_search_by(&empty, |entry| entry.key), Err(0));

    let singleton = [key_record("en").expect("valid packed tag")];
    assert_eq!(query.binary_search_by(&singleton, |entry| entry.key), Ok(0));
    assert_eq!(
        ParsedTag::parse("fr")
            .expect("tag")
            .binary_search_by(&singleton, |entry| entry.key),
        Err(1)
    );

    let mut boundaries = [
        key_record("en").expect("valid packed tag"),
        key_record("fr").expect("valid packed tag"),
    ];
    boundaries.sort_by_key(|entry| entry.key);
    assert_eq!(
        ParsedTag::parse("en")
            .expect("tag")
            .binary_search_by(&boundaries, |entry| entry.key),
        Ok(0)
    );
    assert_eq!(
        ParsedTag::parse("fr")
            .expect("tag")
            .binary_search_by(&boundaries, |entry| entry.key),
        Ok(1)
    );

    let tags = (0..64)
        .map(|index| format!("en-x-v{index}"))
        .collect::<Vec<_>>();
    let mut larger = tags
        .iter()
        .map(|tag| key_record(tag).expect("generated test tag is packable"))
        .collect::<Vec<_>>();
    larger.sort_by_key(|entry| entry.key);
    for position in [0usize, 31, 63] {
        if let Some(tag) = tags.get(position) {
            let parsed = ParsedTag::parse(tag).expect("tag");
            assert!(parsed.binary_search_by(&larger, |entry| entry.key).is_ok());
        }
    }
    assert!(
        ParsedTag::parse("en-x-missing")
            .expect("tag")
            .binary_search_by(&larger, |entry| entry.key)
            .is_err()
    );
}

#[test]
fn long_well_formed_requests_remain_parseable_but_unpacked() {
    let full_length = ParsedTag::parse("en-x-abcdefg-abc").expect("16-byte tag");
    assert!(full_length.packed_key().is_some());

    let too_long = ParsedTag::parse("en-x-abcdefgh-abc").expect("17-byte tag");
    assert_eq!(too_long.packed_key(), None);
    assert_eq!(too_long.truncate(), Some("en"));
    assert_eq!(
        too_long.binary_search_by(&[], |entry: &KeyRecord| entry.key),
        Err(0)
    );
}

#[test]
fn truncation_keeps_only_valid_candidates() {
    assert_eq!(
        ParsedTag::parse("en-Latn-US-x-private-tail")
            .expect("tag")
            .truncate(),
        Some("en-Latn-US")
    );
    assert_eq!(
        ParsedTag::parse("en-US").expect("tag").truncate(),
        Some("en")
    );
    assert_eq!(ParsedTag::parse("x-private").expect("tag").truncate(), None);
    assert_eq!(ParsedTag::parse("i-klingon").expect("tag").truncate(), None);
}

#[test]
fn packed_comparison_matches_exact_normalized_keys() {
    let entries = [key_record("en").expect("valid packed tag")];
    assert_eq!(
        ParsedTag::parse("EN")
            .expect("tag")
            .binary_search_by(&entries, |entry| entry.key),
        Ok(0)
    );
    assert_eq!(
        ParsedTag::parse("fr")
            .expect("tag")
            .packed_key()
            .map(|key| key.cmp(&entries[0].key)),
        Some(Ordering::Greater)
    );
}

#[test]
fn semantic_codes_have_stable_component_layouts() {
    for (tag, expected) in [
        ("en", 453),
        ("es-419", 5_129_187_383_837_285),
        ("zh-Hant-HK", 24_486_079_627_546),
        ("en-001", 4_000_951_014_785_477),
        ("pt-PT", 44_530_220_925_584),
        ("no", 494),
    ] {
        let parsed = ParsedTag::parse(tag).expect("tag is syntactically valid");
        assert_eq!(parsed.semantic_code(), Some(expected), "{tag}");
        assert_eq!(expected >> 53, 0);
    }
}

#[test]
fn semantic_codes_normalize_ascii_case() {
    for case in [
        SemanticCase {
            tag: "en",
            uppercase: "EN",
        },
        SemanticCase {
            tag: "es-419",
            uppercase: "ES-419",
        },
        SemanticCase {
            tag: "zh-Hant-HK",
            uppercase: "ZH-hANT-hk",
        },
        SemanticCase {
            tag: "en-001",
            uppercase: "EN-001",
        },
        SemanticCase {
            tag: "pt-PT",
            uppercase: "PT-pt",
        },
        SemanticCase {
            tag: "no",
            uppercase: "NO",
        },
    ] {
        let normalized = ParsedTag::parse(case.tag)
            .expect("tag is syntactically valid")
            .semantic_code();
        let mixed_case = ParsedTag::parse(case.uppercase)
            .expect("case variation is syntactically valid")
            .semantic_code();
        assert_eq!(mixed_case, normalized);
    }
}

#[test]
fn semantic_encoder_accepts_only_the_bounded_language_script_region_profile() {
    for tag in ["en-Latn", "en-US", "en-Latn-419", "eng", "und"] {
        assert!(
            ParsedTag::parse(tag)
                .expect("tag is syntactically valid")
                .semantic_code()
                .is_some(),
            "{tag}"
        );
    }
    for tag in [
        "root",
        "abcdefgh",
        "zh-cmn-Hans-CN",
        "de-CH-1901",
        "en-x-private",
        "x-internal-build",
        "i-klingon",
        "en-u-ca",
    ] {
        let parsed = ParsedTag::parse(tag);
        if let Ok(parsed) = parsed {
            assert_eq!(parsed.semantic_code(), None, "{tag}");
        }
    }
}

#[test]
fn string_comparison_is_ascii_case_insensitive_for_long_tags() {
    let parsed = ParsedTag::parse("EN-X-ABCDEFGH-ABC").expect("long valid tag");
    assert_eq!(
        parsed.cmp_ignore_ascii_case("en-x-abcdefgh-abc"),
        Ordering::Equal
    );
    assert_eq!(
        parsed.cmp_ignore_ascii_case("en-x-abcdefgh-abd"),
        Ordering::Less
    );
    assert_eq!(
        ParsedTag::parse("fr-x-abcdefgh-abc")
            .expect("long valid tag")
            .cmp_ignore_ascii_case("en-x-abcdefgh-abc"),
        Ordering::Greater
    );
}
