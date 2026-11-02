use u18n::{RenderError, RenderedMessage};
use u18n_result_api_example::{catalogs, m_app_sgreeting, m_app_stitle, t};

#[test]
fn default_translation_signatures_preserve_static_and_owned_results() {
    let english = &catalogs::locale_en::LOCALE;
    let title: Result<&'static str, RenderError> = t!("app.title", locale = english);
    let greeting: Result<String, RenderError> =
        t!("app.greeting", locale = english, { name: "Ada" });
    let macro_title: Result<&'static str, RenderError> = m_app_stitle!(english);
    let macro_greeting: Result<String, RenderError> = m_app_sgreeting!(english, "Ada");

    assert_eq!(title, Ok("Result-based rendering"));
    assert_eq!(greeting, Ok(String::from("Hello, Ada!")));
    assert_eq!(macro_title, Ok("Result-based rendering"));
    assert_eq!(macro_greeting, Ok(String::from("Hello, Ada!")));
    let rendered: RenderedMessage = catalogs::locale_en::CATALOG
        .render(1, &[])
        .expect("dynamic rendering uses the Cow result type");
    assert_eq!(rendered, "Result-based rendering");
}
