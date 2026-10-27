use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::{
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    mem,
};

use u18n_ast::{
    CurrencyPrecision, FileId, FormatOptions, FormatStyle, Grouping, Identifier, Pattern,
    PatternPart, PluralBranch, PluralCase, PluralCategory, PluralKind, Precision, Rounding,
    SelectBranch, Span,
};

#[derive(Debug)]
pub struct LocaleParseError {
    start: usize,
    end: usize,
    message: String,
}

impl LocaleParseError {
    pub(crate) fn new(start: usize, end: usize, message: impl Into<String>) -> Self {
        Self {
            start,
            end,
            message: message.into(),
        }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for LocaleParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.message)
    }
}

impl Error for LocaleParseError {}

const MAX_PLURAL_NESTING: usize = 32;
const MAX_SELECTOR_NESTING: usize = 32;

#[derive(Clone, Copy, Default)]
pub(super) struct PatternDepth {
    plural: usize,
    selector: usize,
}

pub(super) struct PatternParser<'a> {
    source: &'a str,
    file_id: FileId,
    position: usize,
}

impl<'a> PatternParser<'a> {
    pub(super) fn new(source: &'a str, file_id: FileId, position: usize) -> Self {
        Self {
            source,
            file_id,
            position,
        }
    }

    pub(super) fn position(&self) -> usize {
        self.position
    }

    fn identifier(&mut self, key_segment: bool) -> Result<Identifier, LocaleParseError> {
        let start = self.position;
        if !self
            .peek_ascii()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        {
            return Err(self.error(start, start.saturating_add(1), "expected an identifier"));
        }
        self.position += 1;
        while self
            .peek_ascii()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || (key_segment && ch == '-'))
        {
            self.position += 1;
        }
        let text = self
            .source
            .get(start..self.position)
            .unwrap_or("")
            .to_string();
        Ok(Identifier {
            text,
            span: self.span(start, self.position),
        })
    }

    pub(super) fn parse_pattern(
        &mut self,
        in_branch: bool,
        depth: PatternDepth,
    ) -> Result<Pattern, LocaleParseError> {
        let content_start = self.position;
        let terminator = if in_branch { b'}' } else { b'"' };
        let mut parts = Vec::new();
        let mut text_value = String::new();
        let mut text_start = None;
        loop {
            if self.is_eof() {
                return Err(self.error(
                    self.position,
                    self.position,
                    "unterminated message pattern",
                ));
            }
            if self.peek_byte(terminator) {
                self.flush_text(&mut parts, &mut text_value, &mut text_start, self.position);
                self.position += 1;
                return Ok(Pattern {
                    parts,
                    span: self.span(content_start, self.position),
                });
            }
            if self.peek_byte(b'\r') || self.peek_byte(b'\n') {
                return Err(self.error(
                    self.position,
                    self.position + 1,
                    "raw line breaks are not allowed in message patterns",
                ));
            }
            if in_branch && self.peek_byte(b'"') {
                return Err(self.error(
                    self.position,
                    self.position + 1,
                    "unescaped `\"` is not allowed inside a plural branch",
                ));
            }
            if self.peek_byte(b'\\') {
                let escape_start = self.position;
                let decoded = self.escape()?;
                Self::begin_text(&mut text_start, escape_start);
                text_value.push(decoded);
            } else if self.peek_byte(b'{') {
                self.flush_text(&mut parts, &mut text_value, &mut text_start, self.position);
                let selector = self.selector(depth)?;
                parts.push(selector);
            } else if self.peek_byte(b'}') {
                return Err(self.error(
                    self.position,
                    self.position + 1,
                    "unescaped `}` is not allowed in literal text",
                ));
            } else if self.peek_byte(b'#') && in_branch {
                self.flush_text(&mut parts, &mut text_value, &mut text_start, self.position);
                let start = self.position;
                self.position += 1;
                parts.push(PatternPart::Pound {
                    span: self.span(start, self.position),
                });
            } else {
                let start = self.position;
                let character = self
                    .next_char()
                    .ok_or_else(|| self.error(start, start + 1, "invalid UTF-8 source"))?;
                self.position += character.len_utf8();
                Self::begin_text(&mut text_start, start);
                text_value.push(character);
            }
        }
    }

    fn selector(&mut self, depth: PatternDepth) -> Result<PatternPart, LocaleParseError> {
        let start = self.position;
        self.position += 1;
        self.skip_pattern_space();
        let name = self.identifier(false)?;
        self.skip_pattern_space();
        if self.consume_byte(b'}') {
            return Ok(PatternPart::Argument {
                name,
                span: self.span(start, self.position),
            });
        }
        self.require_byte(b',', "expected `}` or `, plural,` after selector name")?;
        self.skip_pattern_space();
        let kind_start = self.position;
        let kind = self.take_ascii_word().to_string();
        self.skip_pattern_space();
        self.require_byte(b',', "expected `,` after selector kind")?;
        self.skip_pattern_space();
        match kind.as_str() {
            "plural" => self.plural_selector(start, name, PluralKind::Cardinal, depth),
            "ordinal" => self.plural_selector(start, name, PluralKind::Ordinal, depth),
            "select" => self.select_selector(start, name, depth),
            "format" => self.format_selector(start, name),
            _ => Err(self.error(
                kind_start,
                self.position.max(kind_start + 1),
                "expected `plural`, `ordinal`, `select`, or `format` selector kind",
            )),
        }
    }

    fn plural_selector(
        &mut self,
        start: usize,
        name: Identifier,
        kind: PluralKind,
        depth: PatternDepth,
    ) -> Result<PatternPart, LocaleParseError> {
        if depth.plural >= MAX_PLURAL_NESTING {
            return Err(self.error(
                start,
                self.position,
                format!("plural nesting exceeds the maximum depth of {MAX_PLURAL_NESTING}"),
            ));
        }
        if depth.selector >= MAX_SELECTOR_NESTING {
            return Err(self.error(
                start,
                self.position,
                format!("selector nesting exceeds the maximum depth of {MAX_SELECTOR_NESTING}"),
            ));
        }
        let mut branches = Vec::new();
        loop {
            let category_start = self.position;
            let (case, case_end) = if self.consume_byte(b'=') {
                let digit_start = self.position;
                while self.peek_ascii().is_some_and(|ch| ch.is_ascii_digit()) {
                    self.position += 1;
                }
                if self.position == digit_start {
                    return Err(self.error(
                        category_start,
                        self.position.max(category_start + 1),
                        "exact plural cases require an unsigned integer after `=`",
                    ));
                }
                let value = self
                    .source
                    .get(digit_start..self.position)
                    .unwrap_or("")
                    .parse::<u64>()
                    .map_err(|_| {
                        self.error(
                            category_start,
                            self.position,
                            "exact plural value exceeds u64",
                        )
                    })?;
                (PluralCase::Exact(value), self.position)
            } else {
                let spelling = self.take_ascii_word().to_string();
                let category = Self::plural_category(&spelling).ok_or_else(|| {
                    self.error(
                        category_start,
                        self.position.max(category_start + 1),
                        format!("unknown plural category `{spelling}`"),
                    )
                })?;
                (PluralCase::Category(category), self.position)
            };
            let case_span = self.span(category_start, case_end);
            self.skip_pattern_space();
            let branch_start = category_start;
            let open_brace = self.position;
            self.require_byte(b'{', "expected `{` before plural branch pattern")?;
            let mut pattern = self.parse_pattern(
                true,
                PatternDepth {
                    plural: depth.plural + 1,
                    selector: depth.selector + 1,
                },
            )?;
            pattern.span = self.span(open_brace, self.position);
            branches.push(PluralBranch {
                case,
                case_span,
                pattern,
                span: self.span(branch_start, self.position),
            });
            self.skip_pattern_space();
            if self.consume_byte(b'}') {
                break;
            }
            if self.is_eof() || self.peek_byte(b'\r') || self.peek_byte(b'\n') {
                return Err(self.error(
                    self.position,
                    self.position,
                    "expected another plural branch or `}`",
                ));
            }
        }
        Ok(PatternPart::Plural {
            name,
            kind,
            branches,
            span: self.span(start, self.position),
        })
    }

    fn format_selector(
        &mut self,
        start: usize,
        name: Identifier,
    ) -> Result<PatternPart, LocaleParseError> {
        let operation_start = self.position;
        let operation = self.take_ascii_word().to_string();
        self.skip_pattern_space();
        self.require_byte(b',', "expected `,` after format operation")?;
        self.skip_pattern_space();
        let options = match operation.as_str() {
            "decimal" => self.decimal_options()?,
            "currency" => self.currency_options()?,
            "date" => FormatOptions::Date {
                style: self.style_option()?,
            },
            "time" => FormatOptions::Time {
                style: self.style_option()?,
            },
            "instant" => {
                let style = self.style_option()?;
                self.require_option_comma("expected `,` before `time_zone`")?;
                self.option_name("time_zone")?;
                self.require_byte(b':', "expected `:` after `time_zone`")?;
                self.skip_pattern_space();
                let time_zone = self.quoted_option_string()?;
                FormatOptions::Instant { style, time_zone }
            }
            _ => {
                return Err(self.error(
                    operation_start,
                    self.position.max(operation_start + 1),
                    "expected `decimal`, `currency`, `date`, `time`, or `instant` format",
                ));
            }
        };
        self.skip_pattern_space();
        self.require_byte(b'}', "expected `}` after format options")?;
        Ok(PatternPart::Format {
            name,
            options,
            span: self.span(start, self.position),
        })
    }

    fn decimal_options(&mut self) -> Result<FormatOptions, LocaleParseError> {
        self.option_name("grouping")?;
        self.require_byte(b':', "expected `:` after `grouping`")?;
        self.skip_pattern_space();
        let grouping = match self.take_ascii_word() {
            "none" => Grouping::None,
            "auto" => Grouping::Auto,
            _ => {
                return Err(self.error(
                    self.position.saturating_sub(1),
                    self.position,
                    "expected `none` or `auto` grouping",
                ));
            }
        };
        self.require_option_comma("expected `,` before `precision`")?;
        self.option_name("precision")?;
        self.require_byte(b':', "expected `:` after `precision`")?;
        self.skip_pattern_space();
        let precision = self.precision_option()?;
        self.require_option_comma("expected `,` before `rounding`")?;
        self.option_name("rounding")?;
        self.require_byte(b':', "expected `:` after `rounding`")?;
        self.skip_pattern_space();
        let rounding = self.rounding_option()?;
        Ok(FormatOptions::Decimal {
            grouping,
            precision,
            rounding,
        })
    }

    fn currency_options(&mut self) -> Result<FormatOptions, LocaleParseError> {
        self.option_name("precision")?;
        self.require_byte(b':', "expected `:` after `precision`")?;
        self.skip_pattern_space();
        let precision_value = self.take_ascii_word().to_string();
        let precision = match precision_value.as_str() {
            "currency" => CurrencyPrecision::CurrencyDefault,
            digits => CurrencyPrecision::Fixed(self.parse_precision(digits)?),
        };
        self.require_option_comma("expected `,` before `rounding`")?;
        self.option_name("rounding")?;
        self.require_byte(b':', "expected `:` after `rounding`")?;
        self.skip_pattern_space();
        let rounding = self.rounding_option()?;
        Ok(FormatOptions::Currency {
            precision,
            rounding,
        })
    }

    fn style_option(&mut self) -> Result<FormatStyle, LocaleParseError> {
        self.option_name("style")?;
        self.require_byte(b':', "expected `:` after `style`")?;
        self.skip_pattern_space();
        match self.take_ascii_word() {
            "short" => Ok(FormatStyle::Short),
            "medium" => Ok(FormatStyle::Medium),
            "long" => Ok(FormatStyle::Long),
            "full" => Ok(FormatStyle::Full),
            _ => Err(self.error(
                self.position.saturating_sub(1),
                self.position,
                "expected `short`, `medium`, `long`, or `full` style",
            )),
        }
    }

    fn precision_option(&mut self) -> Result<Precision, LocaleParseError> {
        let value = self.take_ascii_word().to_string();
        if value == "input" {
            Ok(Precision::Input)
        } else {
            self.parse_precision(&value).map(Precision::Fixed)
        }
    }

    fn parse_precision(&self, value: &str) -> Result<u8, LocaleParseError> {
        if !(1..=2).contains(&value.len()) || !value.bytes().all(|digit| digit.is_ascii_digit()) {
            return Err(self.error(
                self.position.saturating_sub(value.len()),
                self.position,
                "precision must contain one or two digits",
            ));
        }
        let digits = value.parse::<u8>().map_err(|_| {
            self.error(
                self.position.saturating_sub(value.len()),
                self.position,
                "precision must be between 0 and 9",
            )
        })?;
        if digits > 9 {
            return Err(self.error(
                self.position.saturating_sub(value.len()),
                self.position,
                "precision must be between 0 and 9",
            ));
        }
        Ok(digits)
    }

    fn rounding_option(&mut self) -> Result<Rounding, LocaleParseError> {
        match self.take_ascii_word() {
            "half_even" => Ok(Rounding::HalfEven),
            "half_up" => Ok(Rounding::HalfUp),
            "toward_zero" => Ok(Rounding::TowardZero),
            "away_from_zero" => Ok(Rounding::AwayFromZero),
            _ => Err(self.error(
                self.position.saturating_sub(1),
                self.position,
                "unknown rounding mode",
            )),
        }
    }

    fn option_name(&mut self, expected: &str) -> Result<(), LocaleParseError> {
        self.skip_pattern_space();
        let start = self.position;
        let found = self.take_ascii_word();
        if found == expected {
            self.skip_pattern_space();
            Ok(())
        } else {
            Err(self.error(
                start,
                self.position.max(start + 1),
                format!("expected `{expected}` option"),
            ))
        }
    }

    fn require_option_comma(&mut self, message: &str) -> Result<(), LocaleParseError> {
        self.skip_pattern_space();
        self.require_byte(b',', message)?;
        self.skip_pattern_space();
        Ok(())
    }

    fn quoted_option_string(&mut self) -> Result<String, LocaleParseError> {
        self.require_byte(b'\\', "expected an escaped quoted time zone")?;
        self.require_byte(b'"', "expected an escaped quoted time zone")?;
        let mut value = String::new();
        while !self.is_eof()
            && !(self.peek_byte(b'\\')
                && self.source.as_bytes().get(self.position + 1) == Some(&b'"'))
        {
            let start = self.position;
            let character = self
                .next_char()
                .ok_or_else(|| self.error(start, start + 1, "invalid UTF-8 source"))?;
            if !(character.is_ascii_alphanumeric()
                || matches!(character, '_' | '-' | '+' | '/' | '.'))
            {
                return Err(self.error(
                    start,
                    start + character.len_utf8(),
                    "time zone must be an ASCII time-zone identifier",
                ));
            }
            self.position += character.len_utf8();
            value.push(character);
        }
        if value.is_empty() {
            return Err(self.error(self.position, self.position, "time zone cannot be empty"));
        }
        self.require_byte(b'\\', "unterminated time-zone string")?;
        self.require_byte(b'"', "unterminated time-zone string")?;
        Ok(value)
    }

    fn select_selector(
        &mut self,
        start: usize,
        name: Identifier,
        depth: PatternDepth,
    ) -> Result<PatternPart, LocaleParseError> {
        if depth.selector >= MAX_SELECTOR_NESTING {
            return Err(self.error(
                start,
                self.position,
                format!("selector nesting exceeds the maximum depth of {MAX_SELECTOR_NESTING}"),
            ));
        }
        let mut branches = Vec::new();
        loop {
            let branch_start = self.position;
            let case = self.identifier(false)?;
            self.skip_pattern_space();
            let open_brace = self.position;
            self.require_byte(b'{', "expected `{` before select branch pattern")?;
            let mut pattern = self.parse_pattern(
                true,
                PatternDepth {
                    plural: depth.plural,
                    selector: depth.selector + 1,
                },
            )?;
            pattern.span = self.span(open_brace, self.position);
            branches.push(SelectBranch {
                name: case,
                pattern,
                span: self.span(branch_start, self.position),
            });
            self.skip_pattern_space();
            if self.consume_byte(b'}') {
                break;
            }
            if self.is_eof() || self.peek_byte(b'\r') || self.peek_byte(b'\n') {
                return Err(self.error(
                    self.position,
                    self.position,
                    "expected another select branch or `}`",
                ));
            }
        }
        Ok(PatternPart::Select {
            name,
            branches,
            span: self.span(start, self.position),
        })
    }

    fn escape(&mut self) -> Result<char, LocaleParseError> {
        let start = self.position;
        self.position += 1;
        let Some(code) = self.source.as_bytes().get(self.position).copied() else {
            return Err(self.error(start, self.position, "incomplete escape sequence"));
        };
        self.position += 1;
        match code {
            b'\\' => Ok('\\'),
            b'"' => Ok('"'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'{' => Ok('{'),
            b'}' => Ok('}'),
            b'#' => Ok('#'),
            b'u' => self.unicode_escape(start),
            _ => Err(self.error(start, self.position, "unknown escape sequence")),
        }
    }

    fn unicode_escape(&mut self, escape_start: usize) -> Result<char, LocaleParseError> {
        if !self.consume_byte(b'{') {
            return Err(self.error(escape_start, self.position, "expected `{` after `\\u`"));
        }
        let hex_start = self.position;
        let mut digits = 0usize;
        while self.peek_ascii().is_some_and(|ch| ch.is_ascii_hexdigit()) && digits < 7 {
            self.position += 1;
            digits += 1;
        }
        if digits == 0 || digits > 6 || !self.consume_byte(b'}') {
            return Err(self.error(
                escape_start,
                self.position,
                "Unicode escape requires 1 to 6 hexadecimal digits and a closing brace",
            ));
        }
        let value = self.source.get(hex_start..self.position - 1).unwrap_or("");
        let scalar = u32::from_str_radix(value, 16)
            .map_err(|_| self.error(escape_start, self.position, "invalid Unicode scalar value"))?;
        char::from_u32(scalar)
            .ok_or_else(|| self.error(escape_start, self.position, "invalid Unicode scalar value"))
    }

    fn plural_category(spelling: &str) -> Option<PluralCategory> {
        match spelling {
            "zero" => Some(PluralCategory::Zero),
            "one" => Some(PluralCategory::One),
            "two" => Some(PluralCategory::Two),
            "few" => Some(PluralCategory::Few),
            "many" => Some(PluralCategory::Many),
            "other" => Some(PluralCategory::Other),
            _ => None,
        }
    }

    fn flush_text(
        &self,
        parts: &mut Vec<PatternPart>,
        value: &mut String,
        start: &mut Option<usize>,
        end: usize,
    ) {
        if let Some(start) = start.take() {
            parts.push(PatternPart::Text {
                value: mem::take(value),
                span: self.span(start, end),
            });
        }
    }

    fn begin_text(start: &mut Option<usize>, position: usize) {
        if start.is_none() {
            *start = Some(position);
        }
    }

    fn take_ascii_word(&mut self) -> &str {
        let start = self.position;
        while self
            .peek_ascii()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            self.position += 1;
        }
        self.source.get(start..self.position).unwrap_or("")
    }

    fn skip_pattern_space(&mut self) {
        while self.peek_byte(b' ') || self.peek_byte(b'\t') {
            self.position += 1;
        }
    }

    fn require_byte(&mut self, expected: u8, message: &str) -> Result<(), LocaleParseError> {
        if self.consume_byte(expected) {
            Ok(())
        } else {
            Err(self.error(self.position, self.position.saturating_add(1), message))
        }
    }

    fn consume_byte(&mut self, expected: u8) -> bool {
        if self.peek_byte(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn peek_byte(&self, expected: u8) -> bool {
        self.source
            .as_bytes()
            .get(self.position)
            .is_some_and(|byte| *byte == expected)
    }

    fn peek_ascii(&self) -> Option<char> {
        self.source
            .as_bytes()
            .get(self.position)
            .copied()
            .filter(u8::is_ascii)
            .map(char::from)
    }

    fn next_char(&self) -> Option<char> {
        self.source.get(self.position..)?.chars().next()
    }

    fn is_eof(&self) -> bool {
        self.position >= self.source.len()
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span {
            file_id: self.file_id,
            start: start as u32,
            end: end as u32,
        }
    }

    fn error(&self, start: usize, end: usize, message: impl Into<String>) -> LocaleParseError {
        LocaleParseError::new(start, end.min(self.source.len()), message)
    }
}
