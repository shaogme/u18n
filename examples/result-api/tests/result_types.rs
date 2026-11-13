use u18n::{RenderError, RenderedMessage};
use u18n_result_api_example::{catalogs, m_app_sgreeting, m_app_stitle, t};

#[test]
fn default_translation_signatures_preserve_static_and_owned_results() {
    let english = &catalogs::locale_en::LOCALE;
    let title: Result<&'static str, RenderError> = t! { name: "app.title", locale: english };
    let greeting: Result<String, RenderError> =
        t! { name: "app.greeting", locale: english, args: { name: "Ada" } };
    let macro_title: Result<&'static str, RenderError> = m_app_stitle! { locale: english };
    let macro_greeting: Result<String, RenderError> =
        m_app_sgreeting! { locale: english, args: {name: "Ada"} };

    assert_eq!(title, Ok("Result-based rendering"));
    assert_eq!(greeting, Ok(String::from("Hello, Ada!")));
    assert_eq!(macro_title, Ok("Result-based rendering"));
    assert_eq!(macro_greeting, Ok(String::from("Hello, Ada!")));
    let rendered: RenderedMessage = catalogs::locale_en::CATALOG
        .render(1, &[])
        .expect("dynamic rendering uses the Cow result type");
    assert_eq!(rendered, "Result-based rendering");
}

#[test]
fn default_output_mode_supports_keyed_sink_calls() {
    let english = &catalogs::locale_en::LOCALE;
    let mut output = String::from("prefix: ");
    let title: Result<(), RenderError> = m_app_stitle! { locale: english, sink: &mut output };
    assert_eq!(title, Ok(()));
    assert_eq!(output, "prefix: Result-based rendering");

    let mut output = String::from("prefix: ");
    let greeting: Result<(), RenderError> =
        t! { name: "app.greeting", locale: english, sink: &mut output, args: { name: "Ada" } };
    assert_eq!(greeting, Ok(()));
    assert_eq!(output, "prefix: Hello, Ada!");
}
