use core::{ptr, str::FromStr};

use u18n::RenderError;
use u18n_single_locale::{LocaleTag, catalogs, t};

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
    let result: Result<&'static str, RenderError> =
        t!("app.title", locale = &catalogs::locale_en::LOCALE);

    assert_eq!(result, Ok("Single catalog"));
}
