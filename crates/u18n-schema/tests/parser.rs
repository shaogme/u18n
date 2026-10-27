use u18n_ast::{ArgumentType, SchemaVersion};
use u18n_schema::{SchemaParseOutput, SchemaParser};

#[test]
fn parses_schema_messages_and_parameter_order() {
    let source = "schema 1; message cart.items(count: plural_count, item_name: string);";
    let schema = SchemaParser::parse(source, 7).expect("valid schema");

    assert_eq!(schema.version, SchemaVersion::V1);
    assert_eq!(schema.messages.len(), 1);
    let message = &schema.messages[0];
    assert_eq!(message.key.segments[0].text, "cart");
    assert_eq!(message.key.segments[1].text, "items");
    assert_eq!(message.parameters[0].name.text, "count");
    assert_eq!(
        message.parameters[0].argument_type,
        ArgumentType::PluralCount
    );
    assert_eq!(message.parameters[1].name.text, "item_name");
    assert_eq!(message.parameters[1].argument_type, ArgumentType::String);
    assert_eq!(message.span.file_id.0, 7);
}

#[test]
fn rejects_unknown_types_with_their_source_range() {
    let source = "schema 1; message cart.items(count: decimalish);";
    let error = SchemaParser::parse(source, 0).expect_err("unknown type must fail");

    assert_eq!(source.get(error.start..error.end), Some("decimalish"));
    assert!(error.message.contains("unknown argument type `decimalish`"));
}

#[test]
fn accepts_comments_and_empty_schema_bodies() {
    let schema = SchemaParser::parse("// header\nschema 1; // no declarations\n", 0)
        .expect("empty schema is syntactically valid");

    assert!(schema.messages.is_empty());
}

#[test]
fn collects_independent_declaration_errors_without_returning_an_ast() {
    let source = "schema 1; message first(value: decimalish); message second(value: moneyish); message good();";
    let output: SchemaParseOutput = SchemaParser::parse_collect(source, 0);

    assert_eq!(output.errors.len(), 2);
    assert!(output.ast.is_none());
    assert!(output.errors.iter().all(|error| {
        source
            .get(error.start..error.end)
            .is_some_and(|range| range == "decimalish" || range == "moneyish")
    }));
}

#[test]
fn recovers_at_message_boundaries_after_a_malformed_header() {
    let source = "schema 2; message first(value: decimalish); message second(value: moneyish);";
    let output = SchemaParser::parse_collect(source, 0);

    assert!(output.ast.is_none());
    assert_eq!(output.errors.len(), 3);
    assert!(
        output.errors[0]
            .message
            .contains("expected schema version `1`")
    );
    assert!(
        output.errors[1]
            .message
            .contains("unknown argument type `decimalish`")
    );
    assert!(
        output.errors[2]
            .message
            .contains("unknown argument type `moneyish`")
    );
}

#[test]
fn parses_typed_values_and_retains_schema_documentation() {
    let source = "schema 1;\n/// Amount shown at checkout.\nmessage checkout.total(\n    /// Exact amount and scale.\n    amount: decimal, charge: money, day: date, clock: time, moment: instant, rank: ordinal_count, quantity: plural_decimal\n);";
    let schema = SchemaParser::parse(source, 2).expect("typed schema parses");
    let message = &schema.messages[0];

    assert_eq!(message.documentation[0].text, "Amount shown at checkout.");
    assert_eq!(
        message.parameters[0].documentation[0].text,
        "Exact amount and scale."
    );
    assert_eq!(message.parameters[0].argument_type, ArgumentType::Decimal);
    assert_eq!(message.parameters[1].argument_type, ArgumentType::Money);
    assert_eq!(message.parameters[2].argument_type, ArgumentType::Date);
    assert_eq!(message.parameters[3].argument_type, ArgumentType::Time);
    assert_eq!(message.parameters[4].argument_type, ArgumentType::Instant);
    assert_eq!(
        message.parameters[5].argument_type,
        ArgumentType::OrdinalCount
    );
    assert_eq!(
        message.parameters[6].argument_type,
        ArgumentType::PluralDecimal
    );
    assert!(message.documentation[0].span.start < message.documentation[0].span.end);
}
