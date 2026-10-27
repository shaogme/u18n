use std::{cmp::Ordering, collections::BTreeMap};

use u18n_ast::{PluralCategory, PluralKind};

#[derive(Clone, Copy)]
pub(crate) struct DecimalData {
    pub(crate) coefficient: u128,
    pub(crate) scale: u8,
}

#[derive(Clone)]
pub(crate) struct RangeData {
    pub(crate) start: DecimalData,
    pub(crate) end: DecimalData,
}

#[derive(Clone, Copy)]
pub(crate) enum OperandData {
    N,
    I,
    V,
    W,
    F,
    T,
    C,
    E,
}

#[derive(Clone)]
pub(crate) struct PredicateData {
    pub(crate) operand: OperandData,
    pub(crate) modulus: Option<u64>,
    pub(crate) ranges: Vec<RangeData>,
    pub(crate) negated: bool,
    pub(crate) within: bool,
}

#[derive(Clone)]
pub(crate) struct ConjunctionData {
    pub(crate) predicates: Vec<PredicateData>,
}

#[derive(Clone)]
pub(crate) struct RuleData {
    pub(crate) category: PluralCategory,
    pub(crate) alternatives: Vec<ConjunctionData>,
}

#[derive(Clone)]
pub(crate) struct RuleSetData {
    pub(crate) kind: PluralKind,
    pub(crate) categories: Vec<PluralCategory>,
    pub(crate) rules: Vec<RuleData>,
}

#[derive(Clone)]
struct PredicateExpression {
    operand: OperandData,
    modulus: Option<u64>,
    ranges: Vec<RangeData>,
    negated: bool,
    within: bool,
}

struct BooleanDnf {
    clauses: Vec<Vec<PredicateExpression>>,
}

pub(super) struct RulesParser;

struct RuleTokenizer;

impl RuleTokenizer {
    fn tokenize(source: &str) -> Result<Vec<String>, String> {
        let mut tokens = Vec::new();
        let bytes = source.as_bytes();
        let mut position = 0usize;
        while position < bytes.len() {
            let byte = bytes.get(position).copied().unwrap_or_default();
            if byte.is_ascii_whitespace() {
                position += 1;
            } else if byte.is_ascii_alphabetic() {
                let start = position;
                while bytes.get(position).is_some_and(u8::is_ascii_alphabetic) {
                    position += 1;
                }
                tokens.push(source.get(start..position).unwrap_or("").to_string());
            } else if byte.is_ascii_digit() {
                let start = position;
                while bytes.get(position).is_some_and(u8::is_ascii_digit) {
                    position += 1;
                }
                if bytes.get(position) == Some(&b'.') && bytes.get(position + 1) != Some(&b'.') {
                    position += 1;
                    while bytes.get(position).is_some_and(u8::is_ascii_digit) {
                        position += 1;
                    }
                }
                tokens.push(source.get(start..position).unwrap_or("").to_string());
            } else if byte == b'!' && bytes.get(position + 1) == Some(&b'=') {
                tokens.push("!=".to_string());
                position += 2;
            } else if byte == b'.' && bytes.get(position + 1) == Some(&b'.') {
                tokens.push("..".to_string());
                position += 2;
            } else if b"=%(),".contains(&byte) {
                tokens.push(char::from(byte).to_string());
                position += 1;
            } else {
                return Err(format!(
                    "unsupported token in CLDR rule near `{}`",
                    source.get(position..).unwrap_or("")
                ));
            }
        }
        Ok(tokens)
    }
}

struct RuleExpressionParser {
    tokens: Vec<String>,
    position: usize,
}

impl RuleExpressionParser {
    fn parse(mut self) -> Result<BooleanDnf, String> {
        if self.tokens.is_empty() {
            return Ok(BooleanDnf {
                clauses: Vec::new(),
            });
        }
        let expression = self.parse_or()?;
        if self.position != self.tokens.len() {
            return Err(format!(
                "unexpected token `{}` in CLDR plural rule",
                self.peek().unwrap_or("")
            ));
        }
        Ok(expression)
    }

    fn parse_or(&mut self) -> Result<BooleanDnf, String> {
        let mut result = self.parse_and()?;
        while self.consume("or") {
            result.clauses.extend(self.parse_and()?.clauses);
        }
        Ok(result)
    }

    fn parse_and(&mut self) -> Result<BooleanDnf, String> {
        let mut result = self.parse_factor()?;
        while self.consume("and") {
            let next = self.parse_factor()?;
            let mut clauses = Vec::new();
            for left in result.clauses {
                for right in &next.clauses {
                    let mut joined = left.clone();
                    joined.extend(right.iter().cloned());
                    clauses.push(joined);
                }
            }
            result = BooleanDnf { clauses };
        }
        Ok(result)
    }

    fn parse_factor(&mut self) -> Result<BooleanDnf, String> {
        if self.consume("(") {
            let nested = self.parse_or()?;
            if !self.consume(")") {
                return Err("missing `)` in CLDR plural rule".to_string());
            }
            return Ok(nested);
        }
        let operand_name = self
            .next()
            .ok_or_else(|| "missing plural operand".to_string())?;
        let operand = RulesParser::operand(&operand_name)
            .ok_or_else(|| format!("unknown CLDR plural operand `{operand_name}`"))?;
        let modulus = if self.consume("%") || self.consume("mod") {
            Some(
                self.number()?
                    .parse::<u64>()
                    .map_err(|_| "invalid CLDR modulo value".to_string())?,
            )
        } else {
            None
        };
        let (negated, within) = self.relation()?;
        let ranges = self.ranges()?;
        Ok(BooleanDnf {
            clauses: vec![vec![PredicateExpression {
                operand,
                modulus,
                ranges,
                negated,
                within,
            }]],
        })
    }

    fn relation(&mut self) -> Result<(bool, bool), String> {
        if self.consume("=") {
            return Ok((false, true));
        }
        if self.consume("!=") {
            return Ok((true, true));
        }
        if self.consume("is") {
            return Ok((self.consume("not"), false));
        }
        let negated = self.consume("not");
        if self.consume("in") {
            return Ok((negated, false));
        }
        if self.consume("within") {
            return Ok((negated, true));
        }
        Err(format!(
            "expected a CLDR relation operator near `{}`",
            self.peek().unwrap_or("end of rule")
        ))
    }

    fn ranges(&mut self) -> Result<Vec<RangeData>, String> {
        let mut ranges = Vec::new();
        loop {
            let start = RulesParser::decimal(&self.number()?)?;
            let end = if self.consume("..") {
                RulesParser::decimal(&self.number()?)?
            } else {
                start
            };
            if RulesParser::compare_decimal(start, end) == Ordering::Greater {
                return Err("invalid numeric range in CLDR plural rule".to_string());
            }
            ranges.push(RangeData { start, end });
            if !self.consume(",") {
                break;
            }
        }
        Ok(ranges)
    }

    fn number(&mut self) -> Result<String, String> {
        let token = self
            .next()
            .ok_or_else(|| "expected a number in CLDR plural rule".to_string())?;
        if token.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
            && token.chars().any(|ch| ch.is_ascii_digit())
        {
            Ok(token)
        } else {
            Err(format!("expected a number, found `{token}`"))
        }
    }

    fn next(&mut self) -> Option<String> {
        let value = self.tokens.get(self.position).cloned();
        if value.is_some() {
            self.position += 1;
        }
        value
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.position).map(String::as_str)
    }

    fn consume(&mut self, expected: &str) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }
}

impl RulesParser {
    pub(super) fn parse(
        source: &str,
        kind: PluralKind,
    ) -> Result<BTreeMap<String, RuleSetData>, String> {
        let mut all_rules = BTreeMap::new();
        let mut offset = 0usize;
        while let Some(relative) = source
            .get(offset..)
            .and_then(|tail| tail.find("<pluralRules locales="))
        {
            let start = offset + relative;
            let tag_end = source
                .get(start..)
                .and_then(|tail| tail.find('>'))
                .map(|distance| start + distance + 1)
                .ok_or_else(|| "unterminated pluralRules element".to_string())?;
            let tag = source.get(start..tag_end).unwrap_or("");
            let locales = Self::attribute(tag, "locales")
                .ok_or_else(|| "pluralRules element has no locales attribute".to_string())?;
            let close = source
                .get(tag_end..)
                .and_then(|tail| tail.find("</pluralRules>"))
                .map(|distance| tag_end + distance)
                .ok_or_else(|| "unterminated pluralRules element".to_string())?;
            let body = source.get(tag_end..close).unwrap_or("");
            let rule_set = Self::parse_group(body, kind)?;
            for locale in locales.split_ascii_whitespace() {
                all_rules.insert(locale.to_ascii_lowercase(), rule_set.clone());
            }
            offset = close + "</pluralRules>".len();
        }
        if all_rules.is_empty() {
            return Err("CLDR plural rules could not be parsed".to_string());
        }
        Ok(all_rules)
    }

    fn parse_group(body: &str, kind: PluralKind) -> Result<RuleSetData, String> {
        let mut categories = Vec::new();
        let mut rules = Vec::new();
        let mut offset = 0usize;
        while let Some(relative) = body
            .get(offset..)
            .and_then(|tail| tail.find("<pluralRule count="))
        {
            let start = offset + relative;
            let tag_end = body
                .get(start..)
                .and_then(|tail| tail.find('>'))
                .map(|distance| start + distance + 1)
                .ok_or_else(|| "unterminated pluralRule element".to_string())?;
            let tag = body.get(start..tag_end).unwrap_or("");
            let category_text = Self::attribute(tag, "count")
                .ok_or_else(|| "pluralRule element has no count attribute".to_string())?;
            let category = Self::category(category_text)
                .ok_or_else(|| format!("unknown CLDR cardinal category `{category_text}`"))?;
            let close = body
                .get(tag_end..)
                .and_then(|tail| tail.find("</pluralRule>"))
                .map(|distance| tag_end + distance)
                .ok_or_else(|| "unterminated pluralRule body".to_string())?;
            let raw_expression = body.get(tag_end..close).unwrap_or("");
            let expression = raw_expression.split('@').next().unwrap_or("").trim();
            categories.push(category);
            if category != PluralCategory::Other {
                rules.push(Self::parse_rule(category, expression)?);
            }
            offset = close + "</pluralRule>".len();
        }
        if !categories.contains(&PluralCategory::Other) {
            return Err("CLDR cardinal rule set is missing `other`".to_string());
        }
        Ok(RuleSetData {
            kind,
            categories,
            rules,
        })
    }

    fn parse_rule(category: PluralCategory, expression: &str) -> Result<RuleData, String> {
        let tokens = RuleTokenizer::tokenize(expression)?;
        let dnf = RuleExpressionParser {
            tokens,
            position: 0,
        }
        .parse()?;
        let mut alternatives = Vec::new();
        for clause in dnf.clauses {
            let mut predicates = Vec::new();
            for expression in clause {
                predicates.push(Self::compile_predicate(expression)?);
            }
            alternatives.push(ConjunctionData { predicates });
        }
        Ok(RuleData {
            category,
            alternatives,
        })
    }

    fn compile_predicate(expression: PredicateExpression) -> Result<PredicateData, String> {
        if expression.modulus == Some(0) {
            return Err("CLDR plural modulo must be greater than zero".to_string());
        }
        Ok(PredicateData {
            operand: expression.operand,
            modulus: expression.modulus,
            ranges: expression.ranges,
            negated: expression.negated,
            within: expression.within,
        })
    }

    fn operand(value: &str) -> Option<OperandData> {
        match value {
            "n" => Some(OperandData::N),
            "i" => Some(OperandData::I),
            "v" => Some(OperandData::V),
            "w" => Some(OperandData::W),
            "f" => Some(OperandData::F),
            "t" => Some(OperandData::T),
            "c" => Some(OperandData::C),
            "e" => Some(OperandData::E),
            _ => None,
        }
    }

    fn decimal(value: &str) -> Result<DecimalData, String> {
        let (whole, fraction) = match value.split_once('.') {
            Some((whole, fraction)) if !fraction.is_empty() => (whole, fraction),
            Some(_) => return Err("invalid decimal in CLDR plural rule".to_string()),
            None => (value, ""),
        };
        if whole.is_empty()
            || !whole.bytes().all(|byte| byte.is_ascii_digit())
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.len() > 38
        {
            return Err("invalid decimal in CLDR plural rule".to_string());
        }
        let scale = u8::try_from(fraction.len())
            .map_err(|_| "CLDR decimal scale exceeds the runtime limit".to_string())?;
        let fraction_value = if fraction.is_empty() {
            0
        } else {
            fraction
                .parse::<u128>()
                .map_err(|_| "CLDR plural number exceeds the runtime domain".to_string())?
        };
        let coefficient = whole
            .parse::<u128>()
            .map_err(|_| "CLDR plural number exceeds the runtime domain".to_string())?
            .checked_mul(Self::power10(scale)?)
            .and_then(|whole| whole.checked_add(fraction_value))
            .ok_or_else(|| "CLDR plural number exceeds the runtime domain".to_string())?;
        Ok(DecimalData { coefficient, scale })
    }

    fn power10(scale: u8) -> Result<u128, String> {
        let mut value = 1u128;
        for _ in 0..scale {
            value = value
                .checked_mul(10)
                .ok_or_else(|| "CLDR decimal exceeds the runtime domain".to_string())?;
        }
        Ok(value)
    }

    fn compare_decimal(left: DecimalData, right: DecimalData) -> Ordering {
        let left_power = Self::power10(left.scale).unwrap_or(u128::MAX);
        let right_power = Self::power10(right.scale).unwrap_or(u128::MAX);
        match (left.coefficient / left_power).cmp(&(right.coefficient / right_power)) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        let left_fraction = left.coefficient % left_power;
        let right_fraction = right.coefficient % right_power;
        for position in 0..left.scale.max(right.scale) {
            let left_digit = Self::decimal_digit(left_fraction, left.scale, position);
            let right_digit = Self::decimal_digit(right_fraction, right.scale, position);
            match left_digit.cmp(&right_digit) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        Ordering::Equal
    }

    fn decimal_digit(fraction: u128, scale: u8, position: u8) -> u8 {
        if position >= scale {
            return 0;
        }
        let divisor = Self::power10(scale - position - 1).unwrap_or(u128::MAX);
        u8::try_from((fraction / divisor) % 10).unwrap_or_default()
    }

    fn category(value: &str) -> Option<PluralCategory> {
        match value {
            "zero" => Some(PluralCategory::Zero),
            "one" => Some(PluralCategory::One),
            "two" => Some(PluralCategory::Two),
            "few" => Some(PluralCategory::Few),
            "many" => Some(PluralCategory::Many),
            "other" => Some(PluralCategory::Other),
            _ => None,
        }
    }

    fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
        let marker = format!("{name}=\"");
        let start = tag.find(&marker)? + marker.len();
        let end = tag.get(start..)?.find('"')? + start;
        tag.get(start..end)
    }
}
