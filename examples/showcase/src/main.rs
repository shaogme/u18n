use std::{
    error::Error,
    fmt::Write as FmtWrite,
    io::{self, Write},
};

use u18n::{
    CivilDate, CivilTime, Decimal, FormatRequest, Formatter, FormatterError, Instant, Money,
    OrdinalCount, PluralCount,
};
use u18n_showcase::{
    catalogs, m_app_stitle, m_cart_sitems, m_cart_stotal, m_cart_styped_utotal,
    m_checkout_sorder_usummary, m_event_swhen, m_rank_sposition,
};

struct ShowcaseFormatter;

impl Formatter for ShowcaseFormatter {
    fn format(
        &self,
        request: FormatRequest<'_>,
        output: &mut dyn FmtWrite,
    ) -> Result<(), FormatterError> {
        write!(
            output,
            "[{}:{:?}:{:?}]",
            request.locale, request.value, request.options
        )
        .map_err(|_| FormatterError::Failed)
    }
}

fn require_value<T>(value: Option<T>, label: &str) -> io::Result<T> {
    value.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, label))
}

fn main() -> Result<(), Box<dyn Error>> {
    let english = &catalogs::locale_en::LOCALE;
    let spanish = &catalogs::locale_es_419::LOCALE;
    let formatter = ShowcaseFormatter;
    let mut output = io::stdout().lock();

    writeln!(output, "{}", m_app_stitle!(english)?)?;
    writeln!(
        output,
        "{}",
        m_cart_sitems!(english, PluralCount::new(0), "Ada", "other")?
    )?;
    writeln!(
        output,
        "{}",
        m_cart_sitems!(english, PluralCount::new(1), "Ada", "female")?
    )?;
    writeln!(
        output,
        "{}",
        m_cart_sitems!(english, PluralCount::new(3), "Ada", "male")?
    )?;
    writeln!(output, "{}", m_cart_stotal!(english, 19.95, "USD")?)?;
    writeln!(
        output,
        "{}",
        m_cart_sitems!(spanish, PluralCount::new(2), "Ada", "female")?
    )?;
    writeln!(output, "{}", m_cart_stotal!(spanish, 19.95, "USD")?)?;
    writeln!(
        output,
        "{}",
        m_checkout_sorder_usummary!(
            spanish,
            {
                customer: "Ada",
                product: "tazas",
                quantity: PluralCount::new(3),
                total: 19.95,
                currency: "USD",
            }
        )?
    )?;
    writeln!(
        output,
        "{}",
        m_cart_styped_utotal!(
            spanish,
            &formatter,
            Decimal::new(1995, 2),
            require_value(
                Money::new(Decimal::new(1234, 2), "USD"),
                "valid currency code"
            )?,
        )?
    )?;
    writeln!(
        output,
        "{}",
        m_event_swhen!(
            spanish,
            &formatter,
            require_value(CivilDate::new(2026, 7, 4), "valid date")?,
            require_value(CivilTime::new(9, 30, 0, 0), "valid time")?,
            require_value(Instant::new(1_783_150_200, 0), "valid instant")?,
        )?
    )?;
    writeln!(
        output,
        "{}",
        m_rank_sposition!(english, OrdinalCount::new(22))?
    )?;

    Ok(())
}
