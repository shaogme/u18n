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
fn parses_physical_and_escaped_line_breaks_as_spanned_nodes() {
    let source = "locale en; message title = \"a\nb\r\nc\rd\\ne\\rf\\u{A}g\\u{D}h\";";
    let locale = LocaleParser::parse(source, 0).expect("all line-break spellings parse");
    let parts = &locale.messages[0].pattern.parts;
    let line_breaks = parts
        .iter()
        .filter_map(|part| match part {
            PatternPart::LineBreak { span } => source.get(span.start as usize..span.end as usize),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        line_breaks,
        ["\n", "\r\n", "\r", "\\n", "\\r", "\\u{A}", "\\u{D}"]
    );
    assert!(parts.iter().all(|part| match part {
        PatternPart::Text { value, .. } => !value.contains('\r') && !value.contains('\n'),
        _ => true,
    }));
    for window in parts.windows(2) {
        match window {
            [
                PatternPart::Text { span, .. },
                PatternPart::LineBreak { span: line_break },
            ] => {
                assert_eq!(span.end, line_break.start);
            }
            [
                PatternPart::LineBreak { span: line_break },
                PatternPart::Text { span, .. },
            ] => {
                assert_eq!(line_break.end, span.start);
            }
            _ => {}
        }
    }
}

#[test]
fn unicode_line_separators_remain_literal_text() {
    let source = r##"locale en; message title = "a\u{85}b\u{2028}c\u{2029}d";"##;
    let locale = LocaleParser::parse(source, 0).expect("Unicode separators are ordinary text");

    assert!(matches!(
        locale.messages[0].pattern.parts.as_slice(),
        [PatternPart::Text { value, .. }]
            if value == "a\u{85}b\u{2028}c\u{2029}d"
    ));
}

#[test]
fn preserves_leading_and_trailing_pattern_line_breaks() {
    let source = "locale en; message title = \"\r\nfirst\r\";";
    let locale = LocaleParser::parse(source, 0).expect("leading and trailing breaks parse");

    assert!(matches!(
        locale.messages[0].pattern.parts.as_slice(),
        [PatternPart::LineBreak { .. }, PatternPart::Text { value, .. }, PatternPart::LineBreak { .. }]
            if value == "first"
    ));
}

#[test]
fn selector_layout_newlines_are_discarded_but_branch_breaks_are_preserved() {
    let source = "locale en; message items = \"{count,\nplural,\none {# item\n    in cart}\nother {# items\n    in cart}}\";";
    let locale = LocaleParser::parse(source, 0).expect("multiline plural selector parses");
    let PatternPart::Plural { branches, .. } = &locale.messages[0].pattern.parts[0] else {
        panic!("expected plural selector");
    };

    assert!(matches!(
        branches[0].pattern.parts.as_slice(),
        [PatternPart::Pound { .. }, PatternPart::Text { value, .. }, PatternPart::LineBreak { .. }, PatternPart::Text { value: indentation, .. }]
            if value == " item" && indentation == "    in cart"
    ));
    assert!(matches!(
        branches[1].pattern.parts.as_slice(),
        [PatternPart::Pound { .. }, PatternPart::Text { value, .. }, PatternPart::LineBreak { .. }, PatternPart::Text { value: indentation, .. }]
            if value == " items" && indentation == "    in cart"
    ));
}

#[test]
fn all_selector_kinds_accept_layout_line_breaks() {
    let source = "locale en; message patterns = \"{count,\nplural,\none {#} other {#}} {gender,\nselect,\nfemale {F} other {O}} {amount,\nformat,\ndecimal,\ngrouping: auto,\nprecision: 2,\nrounding: half_even} {rank,\nordinal,\none {#st} other {#th}}\";";
    let locale = LocaleParser::parse(source, 0).expect("multiline selector syntax parses");

    assert!(matches!(
        locale.messages[0].pattern.parts.as_slice(),
        [PatternPart::Plural { .. }, PatternPart::Text { value, .. }, PatternPart::Select { .. }, PatternPart::Text { value: second, .. }, PatternPart::Format { .. }, PatternPart::Text { value: third, .. }, PatternPart::Plural { kind: PluralKind::Ordinal, .. }]
            if value == " " && second == " " && third == " "
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
fn recovery_ignores_message_looking_lines_inside_a_multiline_quote() {
    let source = "locale en; message broken = \"bad\\q\nmessage phantom = \\\"looks like\\\"\nstill broken\"; message second = \"bad\\z\";";
    let output = LocaleParser::parse_collect(source, 0);

    assert_eq!(output.errors().len(), 2);
    assert!(output.ast().is_none());
    assert!(output.errors()[0].message().contains("unknown escape"));
    assert!(output.errors()[1].message().contains("unknown escape"));
    assert_eq!(
        source.get(output.errors()[1].start()..output.errors()[1].end()),
        Some("\\z")
    );
}

#[test]
fn unterminated_multiline_quotes_stop_recovery_at_eof() {
    let source = "locale en; message broken = \"open\nmessage second = bad";
    let output = LocaleParser::parse_collect(source, 0);

    assert_eq!(output.errors().len(), 1);
    assert!(output.ast().is_none());
    assert!(
        output.errors()[0]
            .message()
            .contains("unterminated message pattern")
    );
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

#[test]
fn rejects_line_breaks_inside_escaped_time_zone_strings() {
    let source = "locale en; message instant = \"{moment, format, instant, style: medium, time_zone: \\\"America/\nNew_York\\\"}\";";
    let error = LocaleParser::parse(source, 0).expect_err("time-zone strings stay single-line");

    assert!(error.message().contains("ASCII time-zone identifier"));
    assert_eq!(source.get(error.start()..error.end()), Some("\n"));
}
