use core::{
    fmt::{Error as FmtError, Result as FmtResult, Write},
    ptr,
    str::FromStr,
};
use std::cell::RefCell;

use u18n::{
    ArgumentValue, Catalog, CatalogData, CivilDate, CivilTime, Decimal, FormatOptions,
    FormatRequest, FormatValue, Formatter, FormatterError, Instant, MessageBinding, Money,
    OrdinalCount, PluralCount, PluralDecimal, RenderError, SharedProgramStore,
};
use u18n_showcase::{
    FINAL_FALLBACK_LOCALE, LocaleTag, catalogs, clear_current_locale, m_app_stitle,
    m_cart_sdecimal_ucount, m_cart_sitems, m_cart_stotal, m_cart_styped_utotal, m_event_swhen,
    m_rank_sposition, m_report_svalues, set_current_locale, t,
};

struct ExampleFormatter;

const ENGLISH_CATALOG: &Catalog = &catalogs::locale_en::CATALOG;

static FOREIGN_BINDINGS: [MessageBinding; 0] = [];
static FOREIGN_STORE: SharedProgramStore = SharedProgramStore {
    text_pool: "",
    message_signatures: &[],
    programs: &[],
    instructions: &[],
    plurals: &[],
    selects: &[],
    branches: &[],
    select_branches: &[],
    formats: &[],
    rule_sets: &[],
};
static FOREIGN_CATALOG: Catalog = Catalog::new_trusted(CatalogData {
    locale_tag: "en",
    fallback: None,
    bindings: &FOREIGN_BINDINGS,
    shared_store: &FOREIGN_STORE,
});

#[test]
fn catalog_lookup_validates_tags_and_matches_case_insensitively() {
    let english_tag = LocaleTag::from_str("EN").expect("mixed-case known locale tag");
    let spanish_tag = LocaleTag::from_str("es-419").expect("known locale tag");
    let private_tag = LocaleTag::from_str("EN-X-PRIVATE").expect("mixed-case private-use tag");
    assert_eq!(english_tag.as_str(), "en");
    assert_eq!(spanish_tag.as_str(), "es-419");
    assert_eq!(LocaleTag::from_str("ES-419"), Ok(spanish_tag));
    assert_eq!(private_tag.as_str(), "en-x-private");
    let long_private_tag =
        LocaleTag::from_str("EN-X-ABCDEFGH-ABC").expect("mixed-case long private-use tag");
    assert_eq!(long_private_tag.as_str(), "en-x-abcdefgh-abc");
    assert_eq!(
        LocaleTag::from_str("EN-X-ABCDEFGH-XYZ"),
        Err("unsupported locale tag")
    );
    assert_eq!(english_tag, LocaleTag::En);
    assert_eq!(spanish_tag, LocaleTag::Es419);
    assert_eq!(private_tag, LocaleTag::EnXPrivate);
    assert_eq!(long_private_tag, LocaleTag::EnXAbcdefghAbc);
    assert_eq!(LocaleTag::from_str("de-DE"), Err("unsupported locale tag"));
    assert_eq!(
        LocaleTag::from_str("en--US"),
        Err("invalid or policy-rejected BCP 47 locale tag")
    );
    assert!(ptr::eq(
        spanish_tag.as_catalog(),
        &catalogs::locale_es_419::CATALOG
    ));
    assert!(ptr::eq(
        spanish_tag.as_locale(),
        &catalogs::locale_es_419::LOCALE
    ));
    assert!(ptr::eq(english_tag.as_catalog(), ENGLISH_CATALOG));
    assert_eq!(LocaleTag::from_catalog(ENGLISH_CATALOG), Ok(english_tag));
    assert_eq!(
        LocaleTag::from_catalog(&catalogs::locale_es_419::CATALOG),
        Ok(spanish_tag)
    );
    assert_eq!(
        LocaleTag::from_catalog(&FOREIGN_CATALOG),
        Err("catalog is not part of this generated locale set")
    );
}

#[test]
fn locale_fallback_uses_main_parents_then_subtag_truncation() {
    let spanish = LocaleTag::from_str("es-419").expect("known locale tag");
    let french = LocaleTag::from_str("fr").expect("known locale tag");
    let traditional_chinese = LocaleTag::from_str("zh-Hant-HK").expect("known locale tag");
    let indian_english = LocaleTag::from_str("en-IN").expect("known locale tag");
    let arabic = LocaleTag::from_str("ar").expect("known locale tag");
    assert_eq!(traditional_chinese, LocaleTag::ZhHantHk);
    assert_eq!(LocaleTag::fallback("es-AR"), Ok(spanish));
    assert_eq!(LocaleTag::fallback("ES-ar"), Ok(spanish));
    assert_eq!(LocaleTag::fallback("fr-CA"), Ok(french));
    assert_eq!(LocaleTag::fallback("zh-Hant-MO"), Ok(traditional_chinese));
    assert_eq!(LocaleTag::fallback("hi-Latn"), Ok(indian_english));
    assert_eq!(LocaleTag::fallback("en-IN"), Ok(indian_english));
    assert_eq!(LocaleTag::fallback("az-Arab"), Ok(arabic));
    assert_eq!(LocaleTag::fallback("zz-ZZ"), Ok(arabic));
    assert_eq!(LocaleTag::fallback("ES-419"), Ok(spanish));
    assert_eq!(LocaleTag::fallback("fr-x-private-tail"), Ok(french));
    assert_eq!(
        LocaleTag::fallback("EN-X-ABCDEFGH-ABC"),
        Ok(LocaleTag::EnXAbcdefghAbc)
    );
    assert_eq!(LocaleTag::fallback("en-x-abcdefgh-xyz"), Ok(LocaleTag::En));
    assert_eq!(LocaleTag::fallback("en-Latn-US-1901"), Ok(LocaleTag::En));
    assert_eq!(LocaleTag::fallback("x-private"), Ok(arabic));
    assert_eq!(LocaleTag::fallback("i-klingon"), Ok(arabic));
    assert_eq!(
        LocaleTag::fallback("en-u-ca-gregory"),
        Err("invalid or policy-rejected BCP 47 locale tag")
    );
}

#[test]
fn key_based_translation_uses_atomic_selection_and_explicit_overrides() {
    let english = &catalogs::locale_en::LOCALE;
    let spanish = &catalogs::locale_es_419::LOCALE;
    assert!(ptr::eq(FINAL_FALLBACK_LOCALE, english));

    clear_current_locale();
    assert_eq!(t!("app.title"), Ok(String::from("Tiny Cart")));

    let spanish_tag = LocaleTag::from_str("es-419").expect("known locale tag");
    set_current_locale(spanish_tag);
    assert_eq!(t!(r#"app.title"#), Ok(String::from("Carrito pequeño")));
    assert_eq!(
        t!("app.title", locale = english),
        Ok(String::from("Tiny Cart"))
    );
    let value = PluralDecimal::new(150, 2).expect("valid plural decimal");
    assert_eq!(
        t!("cart.decimal_count", value),
        Ok(String::from("1.50 items"))
    );
    assert_eq!(
        t!("cart.decimal_count", { value: value, }),
        Ok(String::from("1.50 items"))
    );

    let formatter = ExampleFormatter;
    let amount = Decimal::new(1995, 2);
    let charge = Money::new(Decimal::new(1234, 2), "USD").expect("valid currency code");
    assert_eq!(
        t!("cart.typed_total", formatter = &formatter, { amount, charge })
            .expect("current catalog is used for formatting"),
        "Total: decimal:1995:2@es-419; charge: money:1234:2:USD@es-419"
    );
    assert_eq!(
        t!("cart.typed_total", locale = english, formatter = &formatter, { amount, charge })
            .expect("explicit catalog overrides the current selection"),
        "Total: decimal:1995:2@en; charge: money:1234:2:USD@en"
    );

    clear_current_locale();
    assert_eq!(t!("app.title"), Ok(String::from("Tiny Cart")));
    assert!(ptr::eq(spanish_tag.as_locale(), spanish));
}

#[test]
fn key_dispatch_evaluates_headers_and_named_values_once_in_written_order() {
    let trace = RefCell::new(Vec::new());
    let formatter = ExampleFormatter;
    let english = &catalogs::locale_en::LOCALE;
    let rendered = t!(
        "cart.typed_total",
        locale = {
            trace.borrow_mut().push("locale");
            english
        },
        formatter = {
            trace.borrow_mut().push("formatter");
            &formatter
        },
        {
            charge: {
                trace.borrow_mut().push("charge");
                Money::new(Decimal::new(1234, 2), "USD").expect("valid currency code")
            },
            amount: {
                trace.borrow_mut().push("amount");
                Decimal::new(1995, 2)
            },
        }
    )
    .expect("key dispatch renders the formatted message");

    assert_eq!(
        rendered,
        "Total: decimal:1995:2@en; charge: money:1234:2:USD@en"
    );
    assert_eq!(
        &*trace.borrow(),
        &["locale", "formatter", "charge", "amount"]
    );
}

#[test]
fn renders_decimal_cardinal_values_with_their_visible_scale() {
    let value = PluralDecimal::new(150, 2).expect("valid plural decimal");
    assert_eq!(
        m_cart_sdecimal_ucount!(&catalogs::locale_es_419::LOCALE, value,),
        Ok(String::from("1.50 items"))
    );

    let exact = PluralDecimal::new(200, 2).expect("valid plural decimal");
    assert_eq!(
        m_cart_sdecimal_ucount!(&catalogs::locale_en::LOCALE, exact),
        Ok(String::from("exact 2.00"))
    );
}

#[test]
fn repeated_plural_selection_matches_keyed_and_dynamic_rendering() {
    struct DecimalPluralCase {
        coefficient: i128,
        scale: u8,
        expected: &'static str,
    }

    let english_locale = &catalogs::locale_en::LOCALE;
    let english_catalog = &catalogs::locale_en::CATALOG;

    for (count, expected) in [
        (0, "Ada has no items in the cart. They have it covered."),
        (1, "Ada has 1 item in the cart. They have it covered."),
        (2, "Ada has 2 items in the cart. They have it covered."),
        (1, "Ada has 1 item in the cart. They have it covered."),
        (0, "Ada has no items in the cart. They have it covered."),
    ] {
        let keyed = m_cart_sitems!(english_locale, PluralCount::new(count), "Ada", "other");
        let dynamic = english_catalog.render(
            2,
            &[
                ArgumentValue::PluralCount(PluralCount::new(count)),
                ArgumentValue::String("Ada"),
                ArgumentValue::String("other"),
            ],
        );
        assert_eq!(keyed, dynamic);
        assert_eq!(dynamic, Ok(String::from(expected)));
    }

    for case in [
        DecimalPluralCase {
            coefficient: 200,
            scale: 2,
            expected: "exact 2.00",
        },
        DecimalPluralCase {
            coefficient: 100,
            scale: 2,
            expected: "1.00 items",
        },
        DecimalPluralCase {
            coefficient: 1,
            scale: 0,
            expected: "1 item",
        },
        DecimalPluralCase {
            coefficient: 150,
            scale: 2,
            expected: "1.50 items",
        },
        DecimalPluralCase {
            coefficient: 200,
            scale: 2,
            expected: "exact 2.00",
        },
    ] {
        let value = PluralDecimal::new(case.coefficient, case.scale).expect("valid plural decimal");
        let keyed = m_cart_sdecimal_ucount!(english_locale, value);
        let dynamic = english_catalog.render(1, &[ArgumentValue::PluralDecimal(value)]);
        assert_eq!(keyed, dynamic);
        assert_eq!(dynamic, Ok(String::from(case.expected)));
    }

    for (rank, expected) in [
        (1, "1st"),
        (2, "2nd"),
        (3, "3rd"),
        (4, "4th"),
        (11, "11th"),
        (22, "22nd"),
        (23, "23rd"),
        (24, "24th"),
        (22, "22nd"),
        (1, "1st"),
    ] {
        let keyed = m_rank_sposition!(english_locale, OrdinalCount::new(rank));
        let dynamic =
            english_catalog.render(7, &[ArgumentValue::OrdinalCount(OrdinalCount::new(rank))]);
        assert_eq!(keyed, dynamic);
        assert_eq!(dynamic, Ok(String::from(expected)));
    }
}

impl Formatter for ExampleFormatter {
    fn format(
        &self,
        request: FormatRequest<'_>,
        output: &mut dyn Write,
    ) -> Result<(), FormatterError> {
        let result = match (request.value, request.options) {
            (FormatValue::Decimal(value), FormatOptions::Decimal { .. }) => write!(
                output,
                "decimal:{}:{}@{}",
                value.coefficient(),
                value.scale(),
                request.locale
            ),
            (FormatValue::Money(value), FormatOptions::Currency { .. }) => write!(
                output,
                "money:{}:{}:{}@{}",
                value.amount().coefficient(),
                value.amount().scale(),
                value.currency(),
                request.locale
            ),
            (FormatValue::Date(value), FormatOptions::Date { .. }) => write!(
                output,
                "date:{:04}-{:02}-{:02}@{}",
                value.year(),
                value.month(),
                value.day(),
                request.locale
            ),
            (FormatValue::Time(value), FormatOptions::Time { .. }) => write!(
                output,
                "time:{:02}:{:02}:{:02}@{}",
                value.hour(),
                value.minute(),
                value.second(),
                request.locale
            ),
            (FormatValue::Instant(value), FormatOptions::Instant { time_zone, .. }) => write!(
                output,
                "instant:{}:{}@{}",
                value.unix_seconds(),
                time_zone,
                request.locale
            ),
            _ => return Err(FormatterError::Unsupported),
        };
        result.map_err(|_| FormatterError::Failed)
    }
}

#[test]
fn renders_schema_typed_messages_for_each_locale() {
    let english = &catalogs::locale_en::LOCALE;

    assert_eq!(
        m_app_stitle!(english, {}).expect("title renders"),
        "Tiny Cart"
    );
    assert_eq!(
        m_cart_sitems!(english, PluralCount::new(1), "Ada", "female").expect("one item renders"),
        "Ada has 1 item in the cart. She has it covered."
    );
    assert_eq!(
        m_cart_sitems!(english, PluralCount::new(3), "Ada", "male").expect("many items render"),
        "Ada has 3 items in the cart. He has it covered."
    );
    assert_eq!(
        m_cart_stotal!(english, 19.95, "USD").expect("total renders"),
        "Total: 19.95 USD"
    );
    assert_eq!(
        m_cart_stotal!(
            english,
            {
                let amount = 20.0;
                amount - 0.05
            },
            "USD"
        )
        .expect("a block expression renders as a positional argument"),
        "Total: 19.95 USD"
    );
}

#[test]
fn renders_exact_plural_select_and_fallback_branches() {
    let english = &catalogs::locale_en::LOCALE;
    let spanish = &catalogs::locale_es_419::LOCALE;
    let count = PluralCount::new(2);
    let owner = "Ada";
    let gender = "female";

    assert_eq!(
        m_cart_sitems!(english, { gender: "other", owner: "Ada", count: PluralCount::new(0), })
            .expect("exact zero branch renders"),
        "Ada has no items in the cart. They have it covered."
    );
    assert_eq!(
        m_cart_sitems!(spanish, { gender, owner, count }).expect("Spanish selectors render"),
        "Ada tiene 2 artículos en el carrito. Gracias, señora."
    );
    assert_eq!(
        m_cart_sitems!(spanish, PluralCount::new(0), "Ada", "unknown")
            .expect("Spanish other cases render"),
        "Ada no tiene ningún artículo en el carrito. Gracias."
    );
    assert_eq!(
        m_cart_stotal!(spanish, 19.95, "USD").expect("fallback total renders"),
        "Total: 19.95 USD"
    );
}

#[test]
fn keyed_macros_match_dynamic_rendering_for_each_locale_path() {
    let english = &catalogs::locale_en::CATALOG;
    let english_locale = &catalogs::locale_en::LOCALE;
    let spanish = &catalogs::locale_es_419::CATALOG;
    let spanish_locale = &catalogs::locale_es_419::LOCALE;
    let french = &catalogs::locale_fr::CATALOG;
    let french_locale = &catalogs::locale_fr::LOCALE;

    assert_eq!(m_app_stitle!(english_locale), english.render(0, &[]));
    assert_eq!(
        m_cart_sdecimal_ucount!(
            french_locale,
            PluralDecimal::new(200, 2).expect("valid plural decimal")
        ),
        french.render(
            1,
            &[ArgumentValue::PluralDecimal(
                PluralDecimal::new(200, 2).expect("valid plural decimal")
            )]
        )
    );
    assert_eq!(
        m_cart_sitems!(spanish_locale, PluralCount::new(2), "Ada", "female"),
        spanish.render(
            2,
            &[
                ArgumentValue::PluralCount(PluralCount::new(2)),
                ArgumentValue::String("Ada"),
                ArgumentValue::String("female"),
            ]
        )
    );
    let amount = 19.95;
    assert_eq!(
        m_cart_stotal!(spanish_locale, amount, "USD"),
        spanish.render(
            3,
            &[ArgumentValue::Number(&amount), ArgumentValue::String("USD"),]
        )
    );

    let formatter = ExampleFormatter;
    let amount = Decimal::new(1995, 2);
    let charge = Money::new(Decimal::new(1234, 2), "USD").expect("valid currency code");
    assert_eq!(
        m_cart_styped_utotal!(spanish_locale, &formatter, amount, charge),
        spanish.render_with(
            &formatter,
            4,
            &[ArgumentValue::Decimal(amount), ArgumentValue::Money(charge),]
        )
    );
    assert_eq!(
        m_event_swhen!(
            french_locale,
            &formatter,
            CivilDate::new(2026, 7, 4).expect("valid date"),
            CivilTime::new(9, 30, 0, 0).expect("valid time"),
            Instant::new(1_783_150_200, 0).expect("valid instant"),
        ),
        french.render_with(
            &formatter,
            6,
            &[
                ArgumentValue::Date(CivilDate::new(2026, 7, 4).expect("valid date")),
                ArgumentValue::Time(CivilTime::new(9, 30, 0, 0).expect("valid time")),
                ArgumentValue::Instant(Instant::new(1_783_150_200, 0).expect("valid instant"),),
            ]
        )
    );
    assert_eq!(
        m_rank_sposition!(english_locale, OrdinalCount::new(22)),
        english.render(7, &[ArgumentValue::OrdinalCount(OrdinalCount::new(22))])
    );
}

#[test]
fn keyed_and_dynamic_streaming_report_the_same_sink_error() {
    struct RejectWrites;

    impl Write for RejectWrites {
        fn write_str(&mut self, _value: &str) -> FmtResult {
            Err(FmtError)
        }
    }

    let locale = &catalogs::locale_en::LOCALE;
    let catalog = &catalogs::locale_en::CATALOG;
    let mut keyed_output = RejectWrites;
    let mut dynamic_output = RejectWrites;

    assert_eq!(
        catalogs::__MESSAGE_DATA_0.render_to(locale, &[], &mut keyed_output),
        catalog.render_to(0, &[], &mut dynamic_output)
    );
    assert_eq!(
        catalogs::__MESSAGE_DATA_0.render_to(locale, &[], &mut RejectWrites),
        Err(RenderError::WriteFailed)
    );
}

#[test]
fn delegates_typed_formatting_with_catalog_locale_and_selects_ordinals() {
    let english = &catalogs::locale_en::LOCALE;
    let spanish = &catalogs::locale_es_419::LOCALE;
    let formatter = ExampleFormatter;
    let amount = Decimal::new(1995, 2);
    let charge = Money::new(Decimal::new(1234, 2), "USD").expect("valid currency code");

    assert_eq!(
        m_cart_styped_utotal!(english, &formatter, amount, charge).expect("typed total renders"),
        "Total: decimal:1995:2@en; charge: money:1234:2:USD@en"
    );
    assert_eq!(
        m_cart_styped_utotal!(spanish, &formatter, amount, charge)
            .expect("fallback formatting uses selected catalog locale"),
        "Total: decimal:1995:2@es-419; charge: money:1234:2:USD@es-419"
    );
    assert_eq!(
        m_event_swhen!(
            english,
            &formatter,
            CivilDate::new(2026, 7, 4).expect("valid date"),
            CivilTime::new(9, 30, 0, 0).expect("valid time"),
            Instant::new(1_783_150_200, 0).expect("valid instant"),
        )
        .expect("date and time formatters render"),
        "date:2026-07-04@en at time:09:30:00@en; logged instant:1783150200:UTC@en"
    );
    assert_eq!(
        m_rank_sposition!(english, OrdinalCount::new(22)).expect("ordinal renders"),
        "22nd"
    );
    assert_eq!(
        m_report_svalues!(english, 3, 4.5).expect("multiple display values render"),
        "3/4.5"
    );
}

#[test]
fn evaluates_catalog_formatter_and_named_values_once_in_written_order() {
    let trace = RefCell::new(Vec::new());
    let formatter = ExampleFormatter;
    let rendered = m_event_swhen!(
        {
            trace.borrow_mut().push("catalog");
            &catalogs::locale_en::LOCALE
        },
        {
            trace.borrow_mut().push("formatter");
            &formatter
        },
        {
            moment: {
                trace.borrow_mut().push("moment");
                Instant::new(1_783_150_200, 0).expect("valid instant")
            },
            clock: {
                trace.borrow_mut().push("clock");
                CivilTime::new(9, 30, 0, 0).expect("valid time")
            },
            day: {
                trace.borrow_mut().push("day");
                CivilDate::new(2026, 7, 4).expect("valid date")
            },
        }
    )
    .expect("formatted message renders");

    assert_eq!(
        rendered,
        "date:2026-07-04@en at time:09:30:00@en; logged instant:1783150200:UTC@en"
    );
    assert_eq!(
        &*trace.borrow(),
        &["catalog", "formatter", "moment", "clock", "day"]
    );
}
