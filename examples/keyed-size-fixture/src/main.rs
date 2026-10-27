use std::{
    error::Error,
    io::{self, Write},
};

include!(concat!(env!("OUT_DIR"), "/u18n_messages.rs"));

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "one")]
    write_output(&m_alpha_sone!(&catalogs::locale_en::LOCALE)?)?;
    #[cfg(feature = "several")]
    {
        write_output(&m_alpha_sone!(&catalogs::locale_en::LOCALE)?)?;
        write_output(&m_beta_stwo!(&catalogs::locale_en::LOCALE)?)?;
    }
    #[cfg(feature = "all")]
    {
        write_output(&m_alpha_sone!(&catalogs::locale_en::LOCALE)?)?;
        write_output(&m_beta_stwo!(&catalogs::locale_en::LOCALE)?)?;
        write_output(&m_gamma_sthree!(&catalogs::locale_en::LOCALE)?)?;
        write_output(&m_delta_sfour!(&catalogs::locale_en::LOCALE)?)?;
    }
    #[cfg(feature = "dynamic")]
    write_output(&catalogs::locale_en::CATALOG.render(0, &[])?)?;
    Ok(())
}

fn write_output(output: &str) -> io::Result<()> {
    io::stdout().lock().write_all(output.as_bytes())
}
