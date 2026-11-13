use std::{
    error::Error,
    io::{self, Write},
};

use u18n_result_api_example::{catalogs, t};

fn main() -> Result<(), Box<dyn Error>> {
    let english = &catalogs::locale_en::LOCALE;
    let title: &'static str = t! { name: "app.title", locale: english }?;
    let greeting: String = t! { name: "app.greeting", locale: english, args: { name: "Ada" } }?;

    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{title}")?;
    writeln!(stdout, "{greeting}")?;
    Ok(())
}
