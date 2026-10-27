use core::cmp::Ordering;

use crate::RenderError;

/// Distinguishes cardinal quantities from ordinal ranks in compiled rules.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PluralKind {
    /// Rules used to count quantities.
    Cardinal,
    /// Rules used to express order or rank.
    Ordinal,
}

/// One of the six CLDR plural categories.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PluralCategory {
    /// Explicit zero category.
    Zero,
    /// Singular category.
    One,
    /// Dual category.
    Two,
    /// Paucal category.
    Few,
    /// Large-number category.
    Many,
    /// Fallback category used when no other rule matches.
    Other,
}

/// One exact non-negative decimal value used by a compiled plural rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluralNumber {
    /// Integer coefficient before applying `scale`.
    pub coefficient: u128,
    /// Number of visible fractional digits.
    pub scale: u8,
}

/// Inclusive exact decimal range used by a compiled plural predicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluralRange {
    /// Inclusive lower bound.
    pub start: PluralNumber,
    /// Inclusive upper bound.
    pub end: PluralNumber,
}

/// An operand from the CLDR plural rule language.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluralOperand {
    /// Absolute value of the source quantity.
    N,
    /// Integer digits of the source quantity.
    I,
    /// Number of visible fractional digits.
    V,
    /// Number of visible fractional digits without trailing zeroes.
    W,
    /// Visible fractional digits as an integer.
    F,
    /// Visible fractional digits without trailing zeroes as an integer.
    T,
    /// Compact decimal exponent, zero for supported values.
    C,
    /// Compact decimal exponent, zero for supported values.
    E,
}

/// One relation in a conjunction of CLDR plural rule conditions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluralPredicate {
    /// Operand evaluated by this relation.
    pub operand: PluralOperand,
    /// Optional modulo applied before range matching.
    pub modulus: Option<u64>,
    /// Inclusive decimal values accepted by this relation.
    pub ranges: &'static [PluralRange],
    /// Whether to invert membership in `ranges`.
    pub negated: bool,
    /// Whether non-integral operand values can match.
    pub within: bool,
}

/// A set of predicates joined with logical AND.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluralConjunction {
    /// Conditions joined with logical AND.
    pub predicates: &'static [PluralPredicate],
}

/// A category rule whose alternatives are joined with logical OR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluralRule {
    /// Category returned when one alternative matches.
    pub category: PluralCategory,
    /// Alternative conjunctions for this category.
    pub alternatives: &'static [PluralConjunction],
}

/// The compiled plural rules assigned to one resolved CLDR locale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleSet {
    /// Rule family represented by this table.
    pub kind: PluralKind,
    /// Non-`other` rules; `other` is selected when none matches.
    pub rules: &'static [PluralRule],
}

struct Operands {
    n: PluralNumber,
    i: u128,
    v: u128,
    w: u128,
    f: u128,
    t: u128,
}

pub(crate) struct RuleEvaluator;

impl RuleEvaluator {
    pub(crate) fn select(
        rule_set: &RuleSet,
        number: PluralNumber,
    ) -> Result<PluralCategory, RenderError> {
        let precedence = [
            PluralCategory::Zero,
            PluralCategory::One,
            PluralCategory::Two,
            PluralCategory::Few,
            PluralCategory::Many,
        ];
        let operands = Self::operands(number)?;
        for category in precedence {
            if let Some(rule) = Self::find_rule(rule_set, category)
                && Self::matches_rule(rule, &operands)?
            {
                return Ok(category);
            }
        }
        Ok(PluralCategory::Other)
    }

    pub(crate) fn validate(rule_set: &RuleSet) -> Result<(), RenderError> {
        Self::validate_categories(rule_set)?;
        for rule in rule_set.rules {
            for alternative in rule.alternatives {
                for predicate in alternative.predicates {
                    if predicate.modulus == Some(0)
                        || predicate.ranges.iter().any(|range| {
                            range.start.scale > 38
                                || range.end.scale > 38
                                || Self::compare(range.start, range.end) == Ordering::Greater
                        })
                    {
                        return Err(RenderError::InvalidCatalog);
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_categories(rule_set: &RuleSet) -> Result<(), RenderError> {
        for (position, rule) in rule_set.rules.iter().enumerate() {
            if rule.category == PluralCategory::Other
                || rule_set
                    .rules
                    .get(position + 1..)
                    .ok_or(RenderError::InvalidCatalog)?
                    .iter()
                    .any(|later| later.category == rule.category)
            {
                return Err(RenderError::InvalidCatalog);
            }
        }
        Ok(())
    }

    fn operands(number: PluralNumber) -> Result<Operands, RenderError> {
        let power = Self::power10(number.scale)?;
        let fraction = number.coefficient % power;
        let mut trailing_zeroes = 0u8;
        let mut trimmed_fraction = fraction;
        while trailing_zeroes < number.scale && trimmed_fraction.is_multiple_of(10) {
            trailing_zeroes += 1;
            trimmed_fraction /= 10;
        }
        Ok(Operands {
            n: number,
            i: number.coefficient / power,
            v: u128::from(number.scale),
            w: u128::from(number.scale - trailing_zeroes),
            f: fraction,
            t: trimmed_fraction,
        })
    }

    fn find_rule(rule_set: &RuleSet, category: PluralCategory) -> Option<&PluralRule> {
        rule_set.rules.iter().find(|rule| rule.category == category)
    }

    fn matches_rule(rule: &PluralRule, operands: &Operands) -> Result<bool, RenderError> {
        for alternative in rule.alternatives {
            if Self::matches_conjunction(alternative, operands)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn matches_conjunction(
        conjunction: &PluralConjunction,
        operands: &Operands,
    ) -> Result<bool, RenderError> {
        for predicate in conjunction.predicates {
            if !Self::matches_predicate(predicate, operands)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn matches_predicate(
        predicate: &PluralPredicate,
        operands: &Operands,
    ) -> Result<bool, RenderError> {
        let mut value = Self::operand_value(predicate.operand, operands);
        if let Some(modulus) = predicate.modulus {
            if modulus == 0 {
                return Err(RenderError::InvalidCatalog);
            }
            value = Self::modulo(value, modulus)?;
        }
        let integral = Self::is_integral(value)?;
        let contained = if !predicate.within && !integral {
            false
        } else {
            Self::contains(predicate.ranges, value)?
        };
        Ok(contained != predicate.negated)
    }

    fn operand_value(operand: PluralOperand, operands: &Operands) -> PluralNumber {
        match operand {
            PluralOperand::N => operands.n,
            PluralOperand::I => Self::integer(operands.i),
            PluralOperand::V => Self::integer(operands.v),
            PluralOperand::W => Self::integer(operands.w),
            PluralOperand::F => Self::integer(operands.f),
            PluralOperand::T => Self::integer(operands.t),
            PluralOperand::C | PluralOperand::E => Self::integer(0),
        }
    }

    fn integer(value: u128) -> PluralNumber {
        PluralNumber {
            coefficient: value,
            scale: 0,
        }
    }

    fn modulo(value: PluralNumber, modulus: u64) -> Result<PluralNumber, RenderError> {
        let modulus = u128::from(modulus);
        let divisor = modulus.checked_mul(Self::power10(value.scale)?);
        let coefficient = match divisor {
            Some(divisor) => value.coefficient % divisor,
            None => value.coefficient,
        };
        Ok(PluralNumber {
            coefficient,
            scale: value.scale,
        })
    }

    fn contains(ranges: &[PluralRange], value: PluralNumber) -> Result<bool, RenderError> {
        for range in ranges {
            if range.start.scale > 38
                || range.end.scale > 38
                || Self::compare(range.start, range.end) == Ordering::Greater
            {
                return Err(RenderError::InvalidCatalog);
            }
            if Self::compare(range.start, value) != Ordering::Greater
                && Self::compare(value, range.end) != Ordering::Greater
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn is_integral(value: PluralNumber) -> Result<bool, RenderError> {
        Ok(value
            .coefficient
            .is_multiple_of(Self::power10(value.scale)?))
    }

    fn compare(left: PluralNumber, right: PluralNumber) -> Ordering {
        let left_power = Self::power10(left.scale).unwrap_or(u128::MAX);
        let right_power = Self::power10(right.scale).unwrap_or(u128::MAX);
        let left_integer = left.coefficient / left_power;
        let right_integer = right.coefficient / right_power;
        match left_integer.cmp(&right_integer) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        let left_fraction = left.coefficient % left_power;
        let right_fraction = right.coefficient % right_power;
        let max_scale = left.scale.max(right.scale);
        for position in 0..max_scale {
            let left_digit = Self::fraction_digit(left_fraction, left.scale, position);
            let right_digit = Self::fraction_digit(right_fraction, right.scale, position);
            match left_digit.cmp(&right_digit) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        Ordering::Equal
    }

    fn fraction_digit(fraction: u128, scale: u8, position: u8) -> u8 {
        if position >= scale {
            return 0;
        }
        let remaining = scale - position - 1;
        let divisor = Self::power10(remaining).unwrap_or(u128::MAX);
        u8::try_from((fraction / divisor) % 10).unwrap_or_default()
    }

    fn power10(scale: u8) -> Result<u128, RenderError> {
        if scale > 38 {
            return Err(RenderError::InvalidCatalog);
        }
        let mut value = 1u128;
        for _ in 0..scale {
            value = value.checked_mul(10).ok_or(RenderError::InvalidCatalog)?;
        }
        Ok(value)
    }
}
