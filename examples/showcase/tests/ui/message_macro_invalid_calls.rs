use u18n::PluralCount;
use u18n_showcase::{catalogs, m_cart_sitems};

fn main() {
    let catalog = &catalogs::locale_en::LOCALE;
    let count = PluralCount::new(2);
    let owner = "Ada";
    let gender = "female";

    m_cart_sitems!(catalog, { count, owner });
    m_cart_sitems!(catalog, { count, owner, gender, extra: "value" });
    m_cart_sitems!(catalog, { count, owner, gender, gender });
    m_cart_sitems!(catalog, { count, owner, gender }, "other");
    m_cart_sitems!(catalog, count, owner);
    m_cart_sitems!(catalog, count, owner, gender, "extra");
    m_cart_sitems!(catalog, { count: "two", owner, gender });
}
