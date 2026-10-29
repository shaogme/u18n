mod pattern;

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
pub use pattern::LocaleParseError;
use pattern::{PatternDepth, PatternParser};
use u18n_ast::{FileId, Identifier, LocaleAst, LocaleId, LocalizedMessage, MessageKey, Span};
use u18n_locale_tag::ParsedTag;

pub struct LocaleParser;

pub struct LocaleParseOutput {
    ast: Option<LocaleAst>,
    errors: Vec<LocaleParseError>,
    locale_tag: Option<String>,
}

impl LocaleParseOutput {
    /// Returns the parsed AST or the first syntax error, if parsing failed.
    pub fn into_result(self) -> Result<LocaleAst, LocaleParseError> {
        let Self { ast, errors, .. } = self;
        if let Some(error) = errors.into_iter().next() {
            return Err(error);
        }
        ast.ok_or_else(|| LocaleParseError::new(0, 0, "locale parser did not produce an AST"))
    }

    pub fn ast(&self) -> Option<&LocaleAst> {
        self.ast.as_ref()
    }

    pub fn errors(&self) -> &[LocaleParseError] {
        &self.errors
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn locale_tag(&self) -> Option<&str> {
        self.locale_tag.as_deref()
    }

    pub fn into_ast(self) -> Option<LocaleAst> {
        self.ast
    }
}

struct SourceParser<'a> {
    source: &'a str,
    file_id: FileId,
    position: usize,
}

struct ParsedLocaleHeader {
    locale: LocaleId,
    fallback: Option<LocaleId>,
    end: usize,
}

impl<'a> SourceParser<'a> {
    fn parse_collect(source: &'a str, file_id: u32) -> LocaleParseOutput {
        let mut parser = Self {
            source,
            file_id: FileId(file_id),
            position: 0,
        };
        let mut errors = Vec::new();
        parser.skip_trivia();
        let file_start = parser.position;
        let mut parsed_locale_tag = None;
        let header = (|| {
            parser.keyword("locale")?;
            parser.require_trivia("expected whitespace after `locale`")?;
            parser.skip_trivia();
            let locale = parser.locale_id("expected a locale tag")?;
            parsed_locale_tag = Some(locale.source_tag.clone());
            parser.validate_locale_id(&locale)?;
            parser.skip_trivia();
            let fallback = if parser.consume_word("fallback") {
                parser.require_trivia("expected whitespace after `fallback`")?;
                parser.skip_trivia();
                let fallback = parser.locale_id("expected a fallback locale tag")?;
                parser.validate_locale_id(&fallback)?;
                Some(fallback)
            } else {
                None
            };
            parser.require_byte(b';', "expected `;` after locale header")?;
            Ok(ParsedLocaleHeader {
                locale,
                fallback,
                end: parser.position,
            })
        })();
        let mut locale = None;
        let mut fallback = None;
        let mut header_end = 0;
        match header {
            Ok(header) => {
                locale = Some(header.locale);
                fallback = header.fallback;
                header_end = header.end;
            }
            Err(error) => {
                errors.push(error);
                parser.recover_to_message_from(file_start);
            }
        }
        let mut messages = Vec::new();
        loop {
            parser.skip_trivia();
            if parser.is_eof() {
                break;
            }
            let message_start = parser.position;
            let declaration: Result<LocalizedMessage, LocaleParseError> = (|| {
                parser.keyword("message")?;
                parser.require_trivia("expected whitespace after `message`")?;
                parser.skip_trivia();
                let key = parser.message_key()?;
                parser.skip_trivia();
                parser.require_byte(b'=', "expected `=` after message key")?;
                parser.skip_trivia();
                let quote_start = parser.position;
                parser.require_byte(b'\"', "expected a quoted message pattern")?;
                let mut pattern_parser =
                    PatternParser::new(parser.source, parser.file_id, parser.position);
                let mut pattern = pattern_parser.parse_pattern(false, PatternDepth::default())?;
                parser.position = pattern_parser.position();
                pattern.span = parser.span(quote_start, parser.position);
                parser.skip_trivia();
                parser.require_byte(b';', "expected `;` after message declaration")?;
                Ok(LocalizedMessage {
                    key,
                    pattern,
                    span: parser.span(message_start, parser.position),
                })
            })();
            match declaration {
                Ok(message) => messages.push(message),
                Err(error) => {
                    let unterminated_pattern = error.message() == "unterminated message pattern";
                    errors.push(error);
                    if unterminated_pattern {
                        parser.position = parser.source.len();
                    } else {
                        parser.recover_to_message(message_start);
                    }
                    if parser.is_eof() {
                        break;
                    }
                }
            }
        }
        let ast = match locale {
            Some(locale) if errors.is_empty() => Some(LocaleAst {
                locale,
                fallback,
                locale_span: parser.span(file_start, header_end),
                messages,
                span: parser.span(file_start, parser.position),
            }),
            _ => None,
        };
        LocaleParseOutput {
            ast,
            errors,
            locale_tag: parsed_locale_tag,
        }
    }

    fn message_key(&mut self) -> Result<MessageKey, LocaleParseError> {
        let start = self.position;
        let mut segments = vec![self.identifier(true)?];
        while self.consume_byte(b'.') {
            segments.push(self.identifier(true)?);
        }
        Ok(MessageKey {
            segments,
            span: self.span(start, self.position),
        })
    }

    fn locale_id(&mut self, message: &str) -> Result<LocaleId, LocaleParseError> {
        let start = self.position;
        while self.position < self.source.len() {
            let remaining = self.source.get(self.position..).unwrap_or("");
            if self
                .source
                .as_bytes()
                .get(self.position)
                .is_some_and(u8::is_ascii_whitespace)
                || self.peek_byte(b';')
                || remaining.starts_with("//")
            {
                break;
            }
            self.position += 1;
        }
        let end = self.position;
        if start == end {
            return Err(self.error(start, start.saturating_add(1), message));
        }
        let source_tag = self.source.get(start..end).unwrap_or("").to_string();
        Ok(LocaleId {
            source_tag,
            span: self.span(start, end),
        })
    }

    fn validate_locale_id(&self, locale: &LocaleId) -> Result<(), LocaleParseError> {
        ParsedTag::parse(&locale.source_tag)
            .map(|_| ())
            .map_err(|error| {
                self.error(
                    locale.span.start as usize,
                    locale.span.end as usize,
                    error.message(),
                )
            })
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

    fn consume_word(&mut self, word: &str) -> bool {
        if self
            .source
            .get(self.position..)
            .is_some_and(|tail| tail.starts_with(word))
        {
            let next = self
                .source
                .as_bytes()
                .get(self.position + word.len())
                .copied();
            if next.is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
                return false;
            }
            self.position += word.len();
            return true;
        }
        false
    }

    fn recover_to_message(&mut self, declaration_start: usize) {
        self.recover_to_message_from(declaration_start.saturating_add("message".len()));
    }

    fn recover_to_message_from(&mut self, search_start: usize) {
        let mut position = search_start.min(self.source.len());
        let bytes = self.source.as_bytes();
        let mut in_quote = false;
        let mut escaped = false;
        while position < bytes.len() {
            let byte = bytes.get(position).copied().unwrap_or_default();
            if in_quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_quote = false;
                }
                position += 1;
                continue;
            }
            if byte == b'"' {
                in_quote = true;
                position += 1;
                continue;
            }
            if self
                .source
                .get(position..)
                .is_some_and(|tail| tail.starts_with("//"))
            {
                while position < bytes.len()
                    && !bytes
                        .get(position)
                        .is_some_and(|ch| *ch == b'\r' || *ch == b'\n')
                {
                    position += 1;
                }
                continue;
            }
            if self
                .source
                .get(position..)
                .is_some_and(|tail| tail.starts_with("message"))
            {
                let before = position
                    .checked_sub(1)
                    .and_then(|index| bytes.get(index))
                    .copied();
                let after = bytes.get(position + "message".len()).copied();
                let before_is_boundary =
                    before.is_none_or(|byte| byte.is_ascii_whitespace()) || before == Some(b';');
                let after_is_boundary = after.is_none_or(|byte| byte.is_ascii_whitespace());
                if before_is_boundary && after_is_boundary {
                    self.position = position;
                    return;
                }
            }
            position += 1;
        }
        self.position = self.source.len();
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.peek_byte(b' ')
                || self.peek_byte(b'\t')
                || self.peek_byte(b'\r')
                || self.peek_byte(b'\n')
            {
                self.position += 1;
            }
            if self
                .source
                .get(self.position..)
                .is_some_and(|tail| tail.starts_with("//"))
            {
                while !self.is_eof() && !self.peek_byte(b'\r') && !self.peek_byte(b'\n') {
                    self.position += 1;
                }
            } else {
                break;
            }
        }
    }

    fn require_trivia(&mut self, message: &str) -> Result<(), LocaleParseError> {
        let start = self.position;
        self.skip_trivia();
        if self.position == start {
            Err(self.error(start, start + 1, message))
        } else {
            Ok(())
        }
    }

    fn keyword(&mut self, word: &str) -> Result<(), LocaleParseError> {
        if self
            .source
            .get(self.position..)
            .is_some_and(|tail| tail.starts_with(word))
        {
            self.position += word.len();
            Ok(())
        } else {
            Err(self.error(
                self.position,
                self.position.saturating_add(word.len()),
                format!("expected `{word}`"),
            ))
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

impl LocaleParser {
    pub fn parse(source: &str, file_id: u32) -> Result<LocaleAst, LocaleParseError> {
        Self::parse_collect(source, file_id).into_result()
    }

    pub fn parse_collect(source: &str, file_id: u32) -> LocaleParseOutput {
        if source.len() > u32::MAX as usize {
            return LocaleParseOutput {
                ast: None,
                errors: vec![LocaleParseError::new(
                    source.len(),
                    source.len(),
                    "source exceeds the maximum span size",
                )],
                locale_tag: None,
            };
        }
        SourceParser::parse_collect(source, file_id)
    }
}
