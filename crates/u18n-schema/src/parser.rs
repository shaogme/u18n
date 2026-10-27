use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use u18n_ast::{
    ArgumentType, Documentation, FileId, Identifier, MessageDef, MessageKey, ParameterDef,
    SchemaAst, SchemaVersion, Span,
};

struct SchemaCursor<'a> {
    source: &'a str,
    file_id: FileId,
    position: usize,
}

#[derive(Debug)]
pub struct SchemaParseError {
    pub start: usize,
    pub end: usize,
    pub message: String,
}

pub struct SchemaParseOutput {
    pub ast: Option<SchemaAst>,
    pub errors: Vec<SchemaParseError>,
}

pub struct SchemaParser;

impl SchemaParser {
    pub fn parse(source: &str, file_id: u32) -> Result<SchemaAst, SchemaParseError> {
        let mut output = Self::parse_collect(source, file_id);
        if let Some(error) = output.errors.drain(..).next() {
            return Err(error);
        }
        output.ast.ok_or(SchemaParseError {
            start: 0,
            end: 0,
            message: "schema parser did not produce an AST".to_string(),
        })
    }

    pub fn parse_collect(source: &str, file_id: u32) -> SchemaParseOutput {
        let mut parser = SchemaCursor {
            source,
            file_id: FileId(file_id),
            position: 0,
        };
        if source.len() > u32::MAX as usize {
            return SchemaParseOutput {
                ast: None,
                errors: vec![parser.error(
                    source.len(),
                    source.len(),
                    "source exceeds the maximum span size",
                )],
            };
        }
        parser.skip_trivia();
        let start = parser.position;
        let mut errors = Vec::new();
        let header = (|| {
            parser.keyword("schema")?;
            parser.require_trivia("expected whitespace after `schema`")?;
            parser.skip_trivia();
            if !parser.consume_byte(b'1') {
                return Err(parser.error(
                    parser.position,
                    parser.position.saturating_add(1),
                    "expected schema version `1`",
                ));
            }
            parser.skip_trivia();
            parser.require_byte(b';', "expected `;` after schema header")
        })();
        if let Err(error) = header {
            errors.push(error);
            parser.recover_to_message_from(start);
        }
        let mut messages = Vec::new();
        loop {
            let documentation = parser.take_documentation();
            if parser.is_eof() {
                break;
            }
            let declaration_start = parser.position;
            let declaration = (|| {
                parser.keyword("message")?;
                parser.require_trivia("expected whitespace after `message`")?;
                parser.skip_trivia();
                let key = parser.message_key()?;
                parser.skip_trivia();
                parser.require_byte(b'(', "expected `(` after message key")?;
                let mut parameters = Vec::new();
                let mut parameter_documentation = parser.take_documentation();
                if !parser.peek_byte(b')') {
                    loop {
                        parameters.push(parser.parameter(parameter_documentation)?);
                        parser.skip_trivia();
                        if !parser.consume_byte(b',') {
                            break;
                        }
                        parameter_documentation = parser.take_documentation();
                        if parser.peek_byte(b')') {
                            return Err(parser.error(
                                parser.position,
                                parser.position + 1,
                                "trailing commas are not allowed in parameter lists",
                            ));
                        }
                    }
                }
                parser.skip_trivia();
                parser.require_byte(b')', "expected `)` after parameter list")?;
                parser.skip_trivia();
                parser.require_byte(b';', "expected `;` after message declaration")?;
                Ok(MessageDef {
                    key,
                    parameters,
                    documentation,
                    span: parser.span(declaration_start, parser.position),
                })
            })();
            match declaration {
                Ok(message) => messages.push(message),
                Err(error) => {
                    errors.push(error);
                    parser.recover_to_message(declaration_start);
                    if parser.is_eof() {
                        break;
                    }
                }
            }
        }
        let ast = if errors.is_empty() {
            Some(SchemaAst {
                version: SchemaVersion::V1,
                messages,
                span: parser.span(start, parser.position),
            })
        } else {
            None
        };
        SchemaParseOutput { ast, errors }
    }
}

impl SchemaCursor<'_> {
    fn parameter(
        &mut self,
        documentation: Vec<Documentation>,
    ) -> Result<ParameterDef, SchemaParseError> {
        let start = self.position;
        let name = self.identifier(false)?;
        self.skip_trivia();
        self.require_byte(b':', "expected `:` after parameter name")?;
        self.skip_trivia();
        let type_start = self.position;
        let spelling = self.take_ascii_word().to_string();
        let argument_type = match spelling.as_str() {
            "plural_count" => ArgumentType::PluralCount,
            "plural_decimal" => ArgumentType::PluralDecimal,
            "ordinal_count" => ArgumentType::OrdinalCount,
            "string" => ArgumentType::String,
            "number" => ArgumentType::Number,
            "decimal" => ArgumentType::Decimal,
            "money" => ArgumentType::Money,
            "date" => ArgumentType::Date,
            "time" => ArgumentType::Time,
            "instant" => ArgumentType::Instant,
            _ => {
                return Err(self.error(
                    type_start,
                    self.position.max(type_start + 1),
                    format!("unknown argument type `{spelling}`"),
                ));
            }
        };
        Ok(ParameterDef {
            name,
            argument_type,
            type_span: self.span(type_start, self.position),
            documentation,
            span: self.span(start, self.position),
        })
    }

    fn take_documentation(&mut self) -> Vec<Documentation> {
        let mut documentation = Vec::new();
        loop {
            while self.peek_byte(b' ')
                || self.peek_byte(b'\t')
                || self.peek_byte(b'\r')
                || self.peek_byte(b'\n')
            {
                self.position += 1;
            }
            if !self
                .source
                .get(self.position..)
                .is_some_and(|tail| tail.starts_with("//"))
            {
                break;
            }
            let start = self.position;
            let is_doc = self
                .source
                .get(self.position..)
                .is_some_and(|tail| tail.starts_with("///"));
            while !self.is_eof() && !self.peek_byte(b'\r') && !self.peek_byte(b'\n') {
                self.position += 1;
            }
            if is_doc {
                let content_start = start.saturating_add(3);
                let mut text = self
                    .source
                    .get(content_start..self.position)
                    .unwrap_or("")
                    .to_string();
                if text.starts_with(' ') {
                    text.remove(0);
                }
                documentation.push(Documentation {
                    text,
                    span: self.span(start, self.position),
                });
            }
        }
        documentation
    }

    fn message_key(&mut self) -> Result<MessageKey, SchemaParseError> {
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

    fn identifier(&mut self, key_segment: bool) -> Result<Identifier, SchemaParseError> {
        let start = self.position;
        let valid_start = self
            .peek_ascii()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_');
        if !valid_start {
            return Err(self.error(start, start.saturating_add(1), "expected an identifier"));
        }
        self.advance_ascii();
        while let Some(ch) = self.peek_ascii() {
            if ch.is_ascii_alphanumeric() || ch == '_' || (key_segment && ch == '-') {
                self.advance_ascii();
            } else {
                break;
            }
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

    fn take_ascii_word(&mut self) -> &str {
        let start = self.position;
        while self
            .peek_ascii()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            self.advance_ascii();
        }
        self.source.get(start..self.position).unwrap_or("")
    }

    fn keyword(&mut self, expected: &str) -> Result<(), SchemaParseError> {
        if self
            .source
            .get(self.position..)
            .is_some_and(|tail| tail.starts_with(expected))
        {
            self.position += expected.len();
            Ok(())
        } else {
            Err(self.error(
                self.position,
                self.position.saturating_add(expected.len()),
                format!("expected `{expected}`"),
            ))
        }
    }

    fn require_trivia(&mut self, message: &str) -> Result<(), SchemaParseError> {
        let start = self.position;
        self.skip_trivia();
        if self.position == start {
            Err(self.error(start, start.saturating_add(1), message))
        } else {
            Ok(())
        }
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

    fn require_byte(&mut self, expected: u8, message: &str) -> Result<(), SchemaParseError> {
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

    fn advance_ascii(&mut self) {
        self.position = self.position.saturating_add(1);
    }

    fn is_eof(&self) -> bool {
        self.position >= self.source.len()
    }

    fn recover_to_message(&mut self, declaration_start: usize) {
        self.recover_to_message_from(declaration_start.saturating_add("message".len()));
    }

    fn recover_to_message_from(&mut self, search_start: usize) {
        let mut position = search_start.min(self.source.len());
        let bytes = self.source.as_bytes();
        while position < bytes.len() {
            if self
                .source
                .get(position..)
                .is_some_and(|tail| tail.starts_with("//"))
            {
                while position < bytes.len()
                    && !bytes
                        .get(position)
                        .is_some_and(|byte| *byte == b'\r' || *byte == b'\n')
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
                if before.is_none_or(|byte| byte.is_ascii_whitespace() || byte == b';')
                    && after.is_none_or(|byte| byte.is_ascii_whitespace())
                {
                    self.position = position;
                    return;
                }
            }
            position += 1;
        }
        self.position = self.source.len();
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span {
            file_id: self.file_id,
            start: start as u32,
            end: end as u32,
        }
    }

    fn error(&self, start: usize, end: usize, message: impl Into<String>) -> SchemaParseError {
        SchemaParseError {
            start,
            end: end.min(self.source.len()),
            message: message.into(),
        }
    }
}
