use u18n_ast::{
    FormatOptions, FormatStyle, Grouping, PatternPart, PluralCase, PluralCategory, PluralKind,
    Precision, Rounding,
};
use u18n_locale::{LocaleParseOutput, LocaleParser};

#[test]
fn parses_nested_plural_patterns_and_escapes() {
    let source = r##"locale en; message cart.items = "{count, plural, one {\# # item} other {{count, plural, one {#} other {# items}}}}";"##;
    let locale = LocaleParser::parse(source, 3).expect("valid locale");

    assert_eq!(locale.locale.source_tag, "en");
    assert_eq!(locale.locale.span.file_id.0, 3);
    let PatternPart::Plural { branches, .. } = &locale.messages[0].pattern.parts[0] else {
        panic!("expected outer plural expression");
    };
    assert_eq!(branches[0].case, PluralCase::Category(PluralCategory::One));
    assert!(matches!(
        branches[0].pattern.parts[0],
        PatternPart::Text { .. }
    ));
    assert!(matches!(
        branches[0].pattern.parts[1],
        PatternPart::Pound { .. }
    ));
    assert_eq!(branches[1].pattern.parts.len(), 1);
    assert!(matches!(
        branches[1].pattern.parts[0],
        PatternPart::Plural { .. }
    ));
}

#[test]
fn rejects_plural_nesting_beyond_the_runtime_limit() {
    let mut pattern = String::from("end");
    for _ in 0..33 {
        pattern = ["{count, plural, one {", &pattern, "} other {}}"].concat();
    }
    let source = format!("locale en; message items = \"{pattern}\";");
    let error = LocaleParser::parse(&source, 0).expect_err("33 nested selectors must fail");

    assert!(
        error
            .message()
            .contains("plural nesting exceeds the maximum depth of 32")
    );
    assert!(source.get(error.start()..error.end()).is_some());
}

#[test]
fn reports_the_source_range_of_invalid_escapes() {
    let source = r##"locale en; message title = "bad\q";"##;
    let error = LocaleParser::parse(source, 0).expect_err("unknown escape must fail");

    assert_eq!(source.get(error.start()..error.end()), Some("\\q"));
    assert!(error.message().contains("unknown escape sequence"));
}

#[test]
fn parses_fallback_select_and_exact_plural_cases() {
    let source = r##"locale fr-CA fallback fr; message cart.summary = "{gender, select, female {Elle} other {Iel}}: {count, plural, =0 {aucun} one {# article} other {# articles}}";"##;
    let locale = LocaleParser::parse(source, 4).expect("valid locale");

    assert_eq!(
        locale
            .fallback
            .as_ref()
            .map(|value| value.source_tag.as_str()),
        Some("fr")
    );
    assert_eq!(
        locale.fallback.as_ref().map(|value| value.span.file_id.0),
        Some(4)
    );
    assert!(matches!(
        locale.messages[0].pattern.parts.as_slice(),
        [
            PatternPart::Select { .. },
            PatternPart::Text { .. },
            PatternPart::Plural { .. }
        ]
    ));
    let PatternPart::Plural { branches, .. } = &locale.messages[0].pattern.parts[2] else {
        panic!("expected plural expression");
    };
    assert_eq!(branches[0].case, PluralCase::Exact(0));
    assert_eq!(branches[1].case, PluralCase::Category(PluralCategory::One));
}

#[test]
fn validates_bcp47_structure_for_locale_and_fallback_headers() {
    let source = "locale ES-419;";
    let parsed = LocaleParser::parse(source, 0).expect("well-formed mixed-case request tag");
    assert_eq!(parsed.locale.source_tag, "ES-419");

    for invalid_source in [
        "locale en--US;",
        "locale en-u-ca;",
        "locale x;",
        "locale en fallback fr_US;",
    ] {
        let error = LocaleParser::parse(invalid_source, 0)
            .expect_err("malformed or policy-rejected tags should fail at the header");
        assert!(invalid_source.get(error.start()..error.end()).is_some());
    }
}

#[test]
fn collects_independent_syntax_errors_without_returning_an_ast() {
    let source = r##"locale en; message first = "bad\q"; message second = "bad\z"; message good = "valid";"##;
    let output: LocaleParseOutput = LocaleParser::parse_collect(source, 0);

    assert_eq!(output.errors().len(), 2);
    assert!(output.ast().is_none());
    assert!(output.errors().iter().all(|error| {
        source
            .get(error.start()..error.end())
            .is_some_and(|range| range.starts_with('\\'))
    }));
}

#[test]
fn recovers_after_an_unterminated_pattern_at_the_next_line() {
    let source = "locale en; message broken = \"raw\nmessage second = \"bad\\q\";";
    let output = LocaleParser::parse_collect(source, 0);

    assert_eq!(output.errors().len(), 2);
    assert!(output.ast().is_none());
    assert!(output.errors()[0].message().contains("raw line breaks"));
    assert!(output.errors()[1].message().contains("unknown escape"));
}

#[test]
fn recovers_at_message_boundaries_after_an_unterminated_header() {
    let source = r##"locale en
message first = "bad\q";
message second = "bad\z";"##;
    let output = LocaleParser::parse_collect(source, 0);

    assert!(output.ast().is_none());
    assert_eq!(output.locale_tag(), Some("en"));
    assert_eq!(output.errors().len(), 3);
    assert!(
        output.errors()[0]
            .message()
            .contains("expected `;` after locale header")
    );
    assert!(
        output.errors()[1]
            .message()
            .contains("unknown escape sequence")
    );
    assert!(
        output.errors()[2]
            .message()
            .contains("unknown escape sequence")
    );
}

#[test]
fn parses_typed_format_operations_and_ordinal_selectors() {
    let source = r#"locale en; message details = "{amount, format, decimal, grouping: auto, precision: 2, rounding: half_even} / {moment, format, instant, style: medium, time_zone: \"America/New_York\"} / {rank, ordinal, one {#st} two {#nd} few {#rd} other {#th}}";"#;
    let locale = LocaleParser::parse(source, 8).expect("typed operations parse");
    let parts = &locale.messages[0].pattern.parts;

    assert!(matches!(
        parts.first(),
        Some(PatternPart::Format {
            options: FormatOptions::Decimal {
                grouping: Grouping::Auto,
                precision: Precision::Fixed(2),
                rounding: Rounding::HalfEven,
            },
            ..
        })
    ));
    assert!(matches!(
        parts.get(2),
        Some(PatternPart::Format {
            options: FormatOptions::Instant {
                style: FormatStyle::Medium,
                time_zone,
            },
            ..
        }) if time_zone == "America/New_York"
    ));
    assert!(matches!(
        parts.get(4),
        Some(PatternPart::Plural {
            kind: PluralKind::Ordinal,
            branches,
            ..
        }) if branches.len() == 4
    ));
}

#[test]
fn rejects_precision_tokens_that_exceed_the_grammar_width() {
    let sources = [
        r#"locale en; message amount = "{amount, format, decimal, grouping: auto, precision: 000, rounding: half_even}";"#,
        r#"locale en; message amount = "{amount, format, currency, precision: 000, rounding: half_even}";"#,
    ];

    for source in sources {
        let error = LocaleParser::parse(source, 0).expect_err("overlong precision must fail");

        assert!(error.message().contains("precision"));
        assert_eq!(source.get(error.start()..error.end()), Some("000"));
    }
}
