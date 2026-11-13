use app_catalog::{catalogs, m_app_stitle, m_report_svalues, t};

fn main() {
    let locale = &catalogs::locale_en::LOCALE;
    let _ = m_app_stitle! { locale: locale };
    let _ = m_report_svalues! { locale: locale, args: { first: 3, second: 4.5 } };
    let _ = t! { name: "report.values", locale: locale, args: { first: 3, second: 4.5 } };
}

#[test]
fn macros_keep_the_catalog_crate_anchor_after_dependency_renaming() {
    let locale = &catalogs::locale_en::LOCALE;
    let direct = m_app_stitle! { locale: locale }.expect("direct message renders");
    let keyed = t! { name: "app.title", locale: locale };
    let direct_with_arguments =
        m_report_svalues! { locale: locale, args: { first: 3, second: 4.5 } }
            .expect("report renders");
    let keyed_with_arguments =
        t! { name: "report.values", locale: locale, args: { first: 3, second: 4.5 } };

    assert_eq!(direct, "Tiny Cart");
    assert_eq!(keyed, "Tiny Cart");
    assert_eq!(direct_with_arguments, "3/4.5");
    assert_eq!(keyed_with_arguments, "3/4.5");
}
