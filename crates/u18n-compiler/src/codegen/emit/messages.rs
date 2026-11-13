use std::{
    collections::BTreeSet,
    fmt::{Error as FmtError, Write},
};

use u18n_ast::{ArgumentType, MessageDef, MessageKey};

struct MessagePlan<'a> {
    message_id: usize,
    message: &'a MessageDef,
    needs_formatter: bool,
    macro_name: String,
    cow_macro_output: bool,
}

struct InvocationEmission<'a> {
    mode: &'a str,
    allow_missing_formatter: &'a str,
    locale: &'a str,
    formatter: &'a str,
    sink: &'a str,
    arguments: &'a str,
}

pub(super) struct MessageEmissionOptions {
    pub(super) atomic_locale: bool,
    pub(super) cow_macro_output: bool,
}

pub(super) struct MessageEmitter;

impl MessageEmitter {
    pub(super) fn emit(
        output: &mut String,
        messages: &[MessageDef],
        formatted_keys: &BTreeSet<String>,
        options: MessageEmissionOptions,
    ) -> Result<(), String> {
        let plans = Self::build_plans(messages, formatted_keys, options.cow_macro_output)?;
        if options.atomic_locale {
            writeln!(
                output,
                "use core::{{ptr::{{self, null_mut}}, str::FromStr, sync::atomic::{{AtomicPtr, Ordering}}}};"
            )
            .map_err(|_| "could not format generated source".to_string())?;
        } else {
            writeln!(output, "use core::{{ptr, str::FromStr}};")
                .map_err(|_| "could not format generated source".to_string())?;
        }
        writeln!(
            output,
            "use u18n::{{__private::ParsedTag, Catalog, Locale, LocaleProvider}};\n"
        )
        .map_err(|_| "could not format generated source".to_string())?;
        writeln!(
            output,
            "#[doc(hidden)] pub mod __u18n_runtime {{ pub use u18n::{{__dispatch_translation, __invoke_message, ArgumentValue}}; }}\n"
        )
        .map_err(|_| "could not format generated source".to_string())?;
        for plan in &plans {
            Self::emit_macro(output, plan)
                .map_err(|_| "could not format generated source".to_string())?;
        }
        Ok(())
    }

    pub(super) fn emit_translation_macro(
        output: &mut String,
        atomic_locale: bool,
        render_error_fallback_message: Option<&str>,
    ) -> Result<(), FmtError> {
        let fallback = render_error_fallback_message
            .map_or_else(String::new, |message| format!(", {message:?}"));
        writeln!(
            output,
            "/// Dispatches a literal message key to its generated, schema-checked message macro."
        )?;
        writeln!(output, "#[macro_export]")?;
        writeln!(output, "macro_rules! t {{")?;
        writeln!(output, "    ($key:literal) => {{")?;
        writeln!(
            output,
            "        $crate::__u18n_runtime::__dispatch_translation! {{ {atomic_locale}, [$crate]{fallback}; name: $key }}"
        )?;
        writeln!(output, "    }};")?;
        writeln!(output, "    ($($fields:tt)*) => {{")?;
        writeln!(
            output,
            "        $crate::__u18n_runtime::__dispatch_translation! {{ {atomic_locale}, [$crate]{fallback}; $($fields)* }}"
        )?;
        writeln!(output, "    }};")?;
        writeln!(output, "}}")?;
        Ok(())
    }

    fn build_plans<'a>(
        messages: &'a [MessageDef],
        formatted_keys: &BTreeSet<String>,
        cow_macro_output: bool,
    ) -> Result<Vec<MessagePlan<'a>>, String> {
        let mut macro_names = BTreeSet::new();
        let mut plans = Vec::with_capacity(messages.len());
        for (message_id, message) in messages.iter().enumerate() {
            let macro_name = format!("m_{}", Self::encoded_key(&message.key));
            if !macro_names.insert(macro_name.clone()) {
                return Err(format!(
                    "generated message macro name collision `{macro_name}`"
                ));
            }
            let key = Self::key_text(&message.key);
            plans.push(MessagePlan {
                message_id,
                message,
                needs_formatter: formatted_keys.contains(&key),
                macro_name,
                cow_macro_output,
            });
        }
        Ok(plans)
    }

    fn emit_macro(output: &mut String, plan: &MessagePlan<'_>) -> Result<(), FmtError> {
        writeln!(
            output,
            "/// Renders the `{}` message with its schema-checked arguments.",
            Self::key_text(&plan.message.key)
        )?;
        writeln!(output, "#[macro_export]")?;
        writeln!(output, "macro_rules! {} {{", plan.macro_name)?;
        writeln!(
            output,
            "    (@__u18n_dispatch [$allow_missing:literal] [$locale:expr] [$($formatter:tt)*] [$($sink:tt)*] {{$($arguments:tt)*}}) => {{"
        )?;
        Self::emit_invocation(
            output,
            plan,
            InvocationEmission {
                mode: "dispatch",
                allow_missing_formatter: "$allow_missing",
                locale: "$locale",
                formatter: "$($formatter)*",
                sink: "$($sink)*",
                arguments: "$($arguments)*",
            },
        )?;
        writeln!(output, "    }};")?;
        writeln!(output, "    ($($arguments:tt)*) => {{")?;
        Self::emit_invocation(
            output,
            plan,
            InvocationEmission {
                mode: "direct",
                allow_missing_formatter: "false",
                locale: "",
                formatter: "",
                sink: "",
                arguments: "$($arguments)*",
            },
        )?;
        writeln!(output, "    }};")?;
        writeln!(output, "}}\n")?;
        Ok(())
    }

    fn emit_invocation(
        output: &mut String,
        plan: &MessagePlan<'_>,
        invocation: InvocationEmission<'_>,
    ) -> Result<(), FmtError> {
        let parameters = plan
            .message
            .parameters
            .iter()
            .map(|parameter| {
                format!(
                    "({}, {})",
                    parameter.name.text,
                    Self::argument_tag(parameter.argument_type)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            output,
            "        $crate::__u18n_runtime::__invoke_message! {{"
        )?;
        writeln!(
            output,
            "            @{} [{}, {}, {}, [$crate::__u18n_runtime], [{}]]",
            invocation.mode,
            plan.needs_formatter,
            plan.cow_macro_output,
            invocation.allow_missing_formatter,
            parameters
        )?;
        if invocation.mode == "direct" {
            writeln!(
                output,
                "            {{ &$crate::catalogs::__MESSAGE_DATA_{} }}",
                plan.message_id
            )?;
            writeln!(output, "            {}", invocation.arguments)?;
            writeln!(output, "        }}")?;
            return Ok(());
        }
        writeln!(output, "            {{ {} }}", invocation.locale)?;
        writeln!(
            output,
            "            {{ &$crate::catalogs::__MESSAGE_DATA_{} }}",
            plan.message_id
        )?;
        writeln!(output, "            {{ {} }}", invocation.formatter)?;
        writeln!(output, "            {{ {} }}", invocation.sink)?;
        writeln!(output, "            {{ {} }}", invocation.arguments)?;
        writeln!(output, "        }}")?;
        Ok(())
    }

    fn argument_tag(argument_type: ArgumentType) -> &'static str {
        match argument_type {
            ArgumentType::String => "string",
            ArgumentType::Number => "number",
            ArgumentType::PluralCount => "plural_count",
            ArgumentType::PluralDecimal => "plural_decimal",
            ArgumentType::OrdinalCount => "ordinal_count",
            ArgumentType::Decimal => "decimal",
            ArgumentType::Money => "money",
            ArgumentType::Date => "date",
            ArgumentType::Time => "time",
            ArgumentType::Instant => "instant",
        }
    }

    fn encoded_key(key: &MessageKey) -> String {
        let mut encoded = String::new();
        for (index, segment) in key.segments.iter().enumerate() {
            for byte in segment.text.bytes() {
                match byte {
                    b'_' => encoded.push_str("_u"),
                    b'-' => encoded.push_str("_h"),
                    _ => encoded.push(char::from(byte)),
                }
            }
            if index + 1 < key.segments.len() {
                encoded.push_str("_s");
            }
        }
        encoded
    }

    fn key_text(key: &MessageKey) -> String {
        key.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}
