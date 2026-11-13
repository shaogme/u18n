use core::{ptr, str::FromStr};
use std::borrow::Cow;

use u18n::{RenderError, RenderedMessage};
use u18n_single_locale::{LocaleTag, catalogs, m_app_sgreeting, m_app_stitle, t};

#[test]
fn generated_single_locale_tag_is_a_closed_enum() {
    let tag = LocaleTag::from_str("EN").expect("the source locale is included");

    assert_eq!(tag, LocaleTag::En);
    assert_eq!(tag.as_str(), "en");
    assert!(ptr::eq(tag.as_catalog(), &catalogs::locale_en::CATALOG));
    assert_eq!(
        LocaleTag::from_catalog(&catalogs::locale_en::CATALOG),
        Ok(LocaleTag::En)
    );
    assert_eq!(LocaleTag::fallback("zz-ZZ"), Ok(LocaleTag::En));
}

#[test]
fn translation_keeps_its_result_type_without_a_render_fallback() {
    let result: Result<RenderedMessage, RenderError> =
        t! { name: "app.title", locale: &catalogs::locale_en::LOCALE };

    assert!(matches!(result, Ok(Cow::Borrowed("Single catalog"))));
    assert!(matches!(
        m_app_stitle! { locale: &catalogs::locale_en::LOCALE },
        Ok(Cow::Borrowed("Single catalog"))
    ));

    let greeting =
        t! { name: "app.greeting", locale: &catalogs::locale_en::LOCALE, args: { name: "Ada" } };
    assert!(matches!(greeting, Ok(Cow::Owned(text)) if text == "Hello, Ada!"));
    assert!(matches!(
        m_app_sgreeting!{ locale: &catalogs::locale_en::LOCALE, args: {name: "Ada"} },
        Ok(Cow::Owned(text)) if text == "Hello, Ada!"
    ));
}
