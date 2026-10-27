use std::{collections::BTreeMap, fmt::Write};

use u18n_ast::{ArgumentType, MessageKey, SchemaAst};

use crate::semantic::{LinkedCatalog, LinkedPattern, LocaleInput};

pub(super) struct Extractor;

impl Extractor {
    pub(super) fn extract(
        schema: &SchemaAst,
        catalog: &LinkedCatalog,
        locales: &[LocaleInput],
        source_locale: &str,
    ) -> Result<String, String> {
        let mut locale_map = BTreeMap::new();
        for locale in locales {
            let tag = locale.ast.locale.source_tag.as_str();
            if locale_map.insert(tag, locale).is_some() {
                return Err(format!("duplicate source locale tag `{tag}`"));
            }
        }
        if !locale_map.contains_key(source_locale) {
            return Err(format!(
                "source locale `{source_locale}` is not present in the input set"
            ));
        }
        let selected = catalog
            .locales
            .iter()
            .find(|locale| locale.ast.locale.source_tag == source_locale)
            .ok_or_else(|| {
                format!("source locale `{source_locale}` is not present in the input set")
            })?;
        let mut messages = schema.messages.iter().collect::<Vec<_>>();
        messages.sort_by_key(|message| Self::key_text(&message.key));
        let mut output = String::new();
        writeln!(
            output,
            "{{\n  \"schema_version\": 1,\n  \"source_locale\": {},\n  \"messages\": [",
            Self::json_string(source_locale)
        )
        .map_err(|_| "could not format extraction output".to_string())?;
        for (message_index, definition) in messages.iter().enumerate() {
            let key = Self::key_text(&definition.key);
            let resolved = selected
                .messages
                .get(message_index)
                .ok_or_else(|| format!("resolved locale `{source_locale}` lacks `{key}`"))?;
            let message = resolved
                .message
                .as_ref()
                .ok_or_else(|| format!("source locale `{source_locale}` is missing `{key}`"))?;
            let author = locale_map
                .get(message.author_locale.as_str())
                .copied()
                .ok_or_else(|| {
                    format!(
                        "pattern author locale `{}` is not present in the input set",
                        message.author_locale
                    )
                })?;
            let pattern_source = Self::pattern_source(&message.pattern, author)?;
            let pattern_locale = message.author_locale.as_str();
            if message_index > 0 {
                output.push_str(",\n");
            }
            let description = definition
                .documentation
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            write!(
                output,
                "    {{\"key\": {}, \"description\": {}, \"pattern_locale\": {}, \"pattern\": {}, \"parameters\": [",
                Self::json_string(&key),
                Self::json_string(&description),
                Self::json_string(pattern_locale),
                Self::json_string(pattern_source),
            )
            .map_err(|_| "could not format extraction output".to_string())?;
            for (parameter_index, parameter) in definition.parameters.iter().enumerate() {
                if parameter_index > 0 {
                    output.push_str(", ");
                }
                let notes = parameter
                    .documentation
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                write!(
                    output,
                    "{{\"name\": {}, \"type\": {}, \"notes\": {}}}",
                    Self::json_string(&parameter.name.text),
                    Self::json_string(Self::type_name(parameter.argument_type)),
                    Self::json_string(&notes),
                )
                .map_err(|_| "could not format extraction output".to_string())?;
            }
            output.push_str("]}");
        }
        output.push_str("\n  ]\n}\n");
        Ok(output)
    }

    fn pattern_source<'locale>(
        pattern: &LinkedPattern,
        locale: &'locale LocaleInput,
    ) -> Result<&'locale str, String> {
        let start = usize::try_from(pattern.span.start)
            .map_err(|_| "pattern span exceeds the host index limit".to_string())?;
        let end = usize::try_from(pattern.span.end)
            .map_err(|_| "pattern span exceeds the host index limit".to_string())?;
        locale
            .source
            .get(start..end)
            .ok_or_else(|| "pattern span is outside its source file".to_string())
    }

    pub(super) fn coverage(catalog: &LinkedCatalog) -> Result<String, String> {
        let mut output = String::from("{\n  \"locales\": [");
        for (locale_index, locale) in catalog.locales.iter().enumerate() {
            let mut authored = 0usize;
            let mut inherited = 0usize;
            let mut missing = 0usize;
            for resolved in &locale.messages {
                match &resolved.message {
                    Some(message) if message.author_locale == locale.ast.locale.source_tag => {
                        authored += 1;
                    }
                    Some(_) => inherited += 1,
                    None => missing += 1,
                }
            }
            if locale_index == 0 {
                output.push('\n');
            } else {
                output.push_str(",\n");
            }
            write!(
                output,
                "    {{\"locale\": {}, \"summary\": {{\"authored\": {authored}, \"inherited\": {inherited}, \"missing\": {missing}}}, \"messages\": [",
                Self::json_string(&locale.ast.locale.source_tag)
            )
            .map_err(|_| "could not format coverage output".to_string())?;
            for (message_index, resolved) in locale.messages.iter().enumerate() {
                if message_index > 0 {
                    output.push_str(", ");
                }
                let key = Self::key_text(&resolved.key);
                match &resolved.message {
                    Some(message) => {
                        let status = if message.author_locale == locale.ast.locale.source_tag {
                            "authored"
                        } else {
                            "inherited"
                        };
                        write!(
                            output,
                            "{{\"key\": {}, \"status\": {}, \"author_locale\": {}}}",
                            Self::json_string(&key),
                            Self::json_string(status),
                            Self::json_string(&message.author_locale)
                        )
                        .map_err(|_| "could not format coverage output".to_string())?;
                    }
                    None => write!(
                        output,
                        "{{\"key\": {}, \"status\": \"missing\"}}",
                        Self::json_string(&key)
                    )
                    .map_err(|_| "could not format coverage output".to_string())?,
                }
            }
            output.push_str("]}");
        }
        output.push_str("\n  ]\n}\n");
        Ok(output)
    }

    fn key_text(key: &MessageKey) -> String {
        key.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }

    fn type_name(argument_type: ArgumentType) -> &'static str {
        match argument_type {
            ArgumentType::PluralCount => "plural_count",
            ArgumentType::PluralDecimal => "plural_decimal",
            ArgumentType::OrdinalCount => "ordinal_count",
            ArgumentType::String => "string",
            ArgumentType::Number => "number",
            ArgumentType::Decimal => "decimal",
            ArgumentType::Money => "money",
            ArgumentType::Date => "date",
            ArgumentType::Time => "time",
            ArgumentType::Instant => "instant",
        }
    }

    fn json_string(value: &str) -> String {
        let mut output = String::from("\"");
        for character in value.chars() {
            match character {
                '"' => output.push_str("\\\""),
                '\\' => output.push_str("\\\\"),
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                character if character.is_control() => {
                    let _ = write!(output, "\\u{:04x}", u32::from(character));
                }
                character => output.push(character),
            }
        }
        output.push('"');
        output
    }
}
