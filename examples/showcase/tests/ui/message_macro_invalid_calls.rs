use u18n::{Decimal, Money, PluralCount};
use u18n_showcase::{catalogs, m_cart_sitems, m_cart_styped_utotal, t};

fn main() {
    let catalog = &catalogs::locale_en::LOCALE;
    let count = PluralCount::new(2);
    let owner = "Ada";
    let gender = "female";
    let amount = Decimal::new(120, 2);
    let charge = Money::new(Decimal::new(100, 2), "USD").expect("valid currency");
    let formatter = ();
    let mut output = String::new();

    m_cart_sitems! { locale: catalog, args: { count, owner } };
    m_cart_sitems! { locale: catalog };
    m_cart_sitems! { locale: catalog, args: { count, owner, gender, extra: "value" } };
    m_cart_sitems! { locale: catalog, args: { count, owner, gender, gender } };
    m_cart_sitems! { locale: catalog, count: count };
    m_cart_sitems! { name: "cart.items", locale: catalog, args: { count, owner, gender } };
    m_cart_styped_utotal! { locale: catalog, args: { amount, charge } };
    m_cart_sitems! { locale: catalog, formatter: &formatter, args: { count, owner, gender } };
    m_cart_sitems! { sink: &mut output, locale: catalog, locale: catalog, args: { count, owner, gender } };
    t! { name: "cart.items", locale: catalog, locale: catalog, args: { count, owner, gender } };
    t! { locale: catalog, args: { count, owner, gender } };
    t! { name: 42, locale: catalog };
    t! { name: "cart.not_declared", locale: catalog };
    t! { name: "cart.items", locale: catalog };
    t!("cart.items", locale = catalog, { count, owner, gender });
    m_cart_sitems! { locale = catalog, count, owner, gender };
}
