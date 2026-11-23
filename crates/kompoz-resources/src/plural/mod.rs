//! Plural rules (CMP's `plural` package): CLDR's categories, the rules' syntax parsed and simplified for whole
//! numbers, the list of a language (and region), which picks the category of a quantity.

mod cldr;

use crate::caches::AsyncCache;
use crate::qualifier::{LanguageQualifier, RegionQualifier};
use cldr::{CLDR_PLURAL_RULE_LIST_INDEX_BY_LOCALE, CLDR_PLURAL_RULE_LISTS};
use std::fmt;
use std::sync::Arc;

/// CLDR's plural categories (CMP's internal `PluralCategory`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    pub const entries: [PluralCategory; 6] = [
        PluralCategory::Zero,
        PluralCategory::One,
        PluralCategory::Two,
        PluralCategory::Few,
        PluralCategory::Many,
        PluralCategory::Other,
    ];

    /// The category of a name (`one`, `FEW`: any case).
    pub fn from_string(name: &str) -> Option<PluralCategory> {
        PluralCategory::entries.into_iter().find(|c| c.name().eq_ignore_ascii_case(name))
    }

    /// Its name as Kotlin writes it (`ONE`).
    pub fn name(self) -> &'static str {
        match self {
            PluralCategory::Zero => "ZERO",
            PluralCategory::One => "ONE",
            PluralCategory::Two => "TWO",
            PluralCategory::Few => "FEW",
            PluralCategory::Many => "MANY",
            PluralCategory::Other => "OTHER",
        }
    }
}

/// A rule that doesn't parse (CMP's PluralRuleParseException): "Invalid syntax at position N: …".
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PluralRuleParseException {
    pub description: String,
    pub position: usize,
}

impl fmt::Display for PluralRuleParseException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid syntax at position {}: {}", self.position, self.description)
    }
}

impl std::error::Error for PluralRuleParseException {}

/// Plural operands (Unicode's LDML: `n`, `i`, `v`, `w`, `f`, `t`, `c`/`e`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Operand {
    N,
    I,
    V,
    W,
    F,
    T,
    C,
}

impl Operand {
    fn name(self) -> &'static str {
        match self {
            Operand::N => "n",
            Operand::I => "i",
            Operand::V => "v",
            Operand::W => "w",
            Operand::F => "f",
            Operand::T => "t",
            Operand::C => "c",
        }
    }
    fn is_integer_part(self) -> bool {
        matches!(self, Operand::N | Operand::I)
    }
}

/// A rule's condition (CMP's `Condition`).
#[derive(Clone, PartialEq, Debug)]
enum Condition {
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
    Relation { operand: Operand, divisor: Option<i32>, negated: bool, ranges: Vec<(i32, i32)> },
    True,
    False,
}

impl Condition {
    fn is_fulfilled(&self, n: i32) -> bool {
        match self {
            Condition::And(l, r) => l.is_fulfilled(n) && r.is_fulfilled(n),
            Condition::Or(l, r) => l.is_fulfilled(n) || r.is_fulfilled(n),
            Condition::Relation { operand, divisor, negated, ranges } => {
                let value = if operand.is_integer_part() { n.wrapping_abs() } else { 0 };
                let value = match divisor {
                    Some(d) => value % d,
                    None => value,
                };
                ranges.iter().any(|(a, b)| (*a..=*b).contains(&value)) != *negated
            }
            Condition::True => true,
            Condition::False => false,
        }
    }

    /// What it is for whole numbers (fraction operands are 0): CMP's `simplifyForInteger`.
    fn simplify_for_integer(&self) -> Condition {
        match self {
            Condition::And(l, r) => {
                let l = l.simplify_for_integer();
                if l == Condition::False {
                    return Condition::False;
                }
                let r = r.simplify_for_integer();
                match (&l, &r) {
                    (Condition::True, _) => return r,
                    (_, Condition::False) => return Condition::False,
                    (_, Condition::True) => return l,
                    _ => {}
                }
                if l.equivalent_for_integer(&r) {
                    return l;
                }
                Condition::And(Box::new(l), Box::new(r))
            }
            Condition::Or(l, r) => {
                let l = l.simplify_for_integer();
                if l == Condition::True {
                    return Condition::True;
                }
                let r = r.simplify_for_integer();
                match (&l, &r) {
                    (Condition::False, _) => return r,
                    (_, Condition::True) => return Condition::True,
                    (_, Condition::False) => return l,
                    _ => {}
                }
                if l.equivalent_for_integer(&r) {
                    return l;
                }
                Condition::Or(Box::new(l), Box::new(r))
            }
            Condition::Relation { operand, divisor, negated, ranges } => {
                if operand.is_integer_part() {
                    Condition::Relation {
                        operand: Operand::N,
                        divisor: *divisor,
                        negated: *negated,
                        ranges: ranges.clone(),
                    }
                } else if ranges.iter().any(|(a, b)| (*a..=*b).contains(&0)) != *negated {
                    Condition::True
                } else {
                    Condition::False
                }
            }
            Condition::True => Condition::True,
            Condition::False => Condition::False,
        }
    }

    fn equivalent_for_integer(&self, other: &Condition) -> bool {
        match (self, other) {
            (Condition::And(a, b), Condition::And(c, d)) | (Condition::Or(a, b), Condition::Or(c, d)) => {
                a.equivalent_for_integer(c) && b.equivalent_for_integer(d)
            }
            (
                Condition::Relation { operand: o1, divisor: d1, negated: n1, ranges: r1 },
                Condition::Relation { operand: o2, divisor: d2, negated: n2, ranges: r2 },
            ) => o1.is_integer_part() == o2.is_integer_part() && d1 == d2 && n1 == n2 && r1 == r2,
            (Condition::True, Condition::True) | (Condition::False, Condition::False) => true,
            _ => false,
        }
    }

    /// The rule's syntax (CMP's `parse`): `condition = and_condition ('or' and_condition)*`, `and_condition =
    /// relation ('and' relation)*`, `relation = operand ('%' value)? ('=' | '!=') range_list`; samples aren't read.
    fn parse(description: &str) -> Result<Condition, PluralRuleParseException> {
        Ok(Parser { d: description.as_bytes(), text: description, i: 0 }.parse()?.simplify_for_integer())
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Condition::And(l, r) => write!(f, "{l} and {r}"),
            Condition::Or(l, r) => write!(f, "{l} or {r}"),
            Condition::Relation { operand, divisor, negated, ranges } => {
                write!(f, "{}", operand.name())?;
                if let Some(d) = divisor {
                    write!(f, " % {d}")?;
                }
                write!(f, " {}= ", if *negated { "!" } else { "" })?;
                for (k, (a, b)) in ranges.iter().enumerate() {
                    if k > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{a}")?;
                    if a != b {
                        write!(f, "..{b}")?;
                    }
                }
                Ok(())
            }
            Condition::True => Ok(()),
            Condition::False => f.write_str("(false)"),
        }
    }
}

struct Parser<'a> {
    d: &'a [u8],
    text: &'a str,
    i: usize,
}

impl Parser<'_> {
    fn raise<T>(&self) -> Result<T, PluralRuleParseException> {
        Err(PluralRuleParseException { description: self.text.to_string(), position: self.i + 1 })
    }
    fn check(&self, ok: bool) -> Result<(), PluralRuleParseException> {
        if ok { Ok(()) } else { self.raise() }
    }
    fn eof(&self) -> bool {
        self.i >= self.d.len()
    }
    fn blanks(&mut self) {
        while !self.eof() && (self.d[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.d.get(self.i).copied()
    }
    fn next(&mut self) -> Result<u8, PluralRuleParseException> {
        match self.peek() {
            Some(c) => {
                self.i += 1;
                Ok(c)
            }
            None => self.raise(),
        }
    }
    fn int(&mut self) -> Result<i32, PluralRuleParseException> {
        self.check(self.peek().is_some_and(|c| c.is_ascii_digit()))?;
        let mut v: i32 = 0;
        while let Some(c) = self.peek().filter(u8::is_ascii_digit) {
            v = v.wrapping_mul(10).wrapping_add((c - b'0') as i32);
            self.i += 1;
        }
        Ok(v)
    }
    fn parse(&mut self) -> Result<Condition, PluralRuleParseException> {
        self.blanks();
        if self.eof() {
            return Ok(Condition::True);
        }
        let c = self.condition()?;
        self.blanks();
        self.check(self.eof())?;
        Ok(c)
    }
    fn condition(&mut self) -> Result<Condition, PluralRuleParseException> {
        let mut c = self.and_condition()?;
        loop {
            self.blanks();
            if self.peek() != Some(b'o') {
                return Ok(c);
            }
            self.next()?;
            let r = self.next()?;
            self.check(r == b'r')?;
            c = Condition::Or(Box::new(c), Box::new(self.and_condition()?));
        }
    }
    fn and_condition(&mut self) -> Result<Condition, PluralRuleParseException> {
        let mut c = self.relation()?;
        loop {
            self.blanks();
            if self.peek() != Some(b'a') {
                return Ok(c);
            }
            self.next()?;
            let n = self.next()?;
            self.check(n == b'n')?;
            let d = self.next()?;
            self.check(d == b'd')?;
            c = Condition::And(Box::new(c), Box::new(self.relation()?));
        }
    }
    fn relation(&mut self) -> Result<Condition, PluralRuleParseException> {
        self.blanks();
        let operand = match self.next()? {
            b'n' => Operand::N,
            b'i' => Operand::I,
            b'f' => Operand::F,
            b't' => Operand::T,
            b'v' => Operand::V,
            b'w' => Operand::W,
            b'c' | b'e' => Operand::C,
            _ => return self.raise(),
        };
        self.blanks();
        let divisor = if self.peek() == Some(b'%') {
            self.next()?;
            self.blanks();
            Some(self.int()?)
        } else if self.eof() {
            return self.raise();
        } else {
            None
        };
        self.blanks();
        let negated = match self.peek() {
            Some(b'!') => {
                self.next()?;
                let e = self.next()?;
                self.check(e == b'=')?;
                true
            }
            Some(b'=') => {
                self.next()?;
                false
            }
            _ => return self.raise(),
        };
        let mut ranges = vec![self.range()?];
        while self.peek() == Some(b',') {
            self.next()?;
            ranges.push(self.range()?);
        }
        Ok(Condition::Relation { operand, divisor, negated, ranges })
    }
    fn range(&mut self) -> Result<(i32, i32), PluralRuleParseException> {
        self.blanks();
        let start = self.int()?;
        if self.peek() != Some(b'.') {
            return Ok((start, start));
        }
        self.next()?;
        let dot = self.next()?;
        self.check(dot == b'.')?;
        Ok((start, self.int()?))
    }
}

/// A category and its condition (CMP's `PluralRule`).
#[derive(Clone, Debug)]
pub struct PluralRule {
    pub category: PluralCategory,
    condition: Condition,
}

impl PluralRule {
    /// A rule of CLDR's syntax (`n % 10 = 1 and n % 100 != 11`).
    pub fn new(category: PluralCategory, condition: &str) -> Result<PluralRule, PluralRuleParseException> {
        Ok(PluralRule { category, condition: Condition::parse(condition)? })
    }
    /// Whether `n` is of its category.
    pub fn applies_to(&self, n: i32) -> bool {
        self.condition.is_fulfilled(n)
    }
    /// Its condition as simplified for whole numbers, written in CLDR's syntax.
    pub fn condition(&self) -> String {
        self.condition.to_string()
    }
}

/// A language's rules, in order (CMP's `PluralRuleList`).
#[derive(Clone, Debug)]
pub struct PluralRuleList {
    rules: Arc<[PluralRule]>,
}

static RULE_LISTS: AsyncCache<usize, PluralRuleList> = AsyncCache::new();

pub(crate) fn clear_cache() {
    RULE_LISTS.clear();
}

impl PluralRuleList {
    /// The category of `quantity`: the first rule that applies (a list without rules: panics, as Kotlin's
    /// `first` throws NoSuchElementException — what a quantity string of a language CLDR doesn't have gets).
    pub fn get_category(&self, quantity: i32) -> PluralCategory {
        self.rules
            .iter()
            .find(|r| r.applies_to(quantity))
            .map(|r| r.category)
            .unwrap_or_else(|| panic!("NoSuchElementException: Array contains no element matching the predicate."))
    }

    /// The list of a language and region: CLDR's `language_REGION`, else its `language`'s; none of them, an empty
    /// one.
    pub async fn get_instance(language: &LanguageQualifier, region: &RegionQualifier) -> PluralRuleList {
        let with_region = format!("{}_{}", language.language, region.region);
        let name = if index_of(&with_region).is_some() {
            with_region
        } else if index_of(&language.language).is_some() {
            language.language.clone()
        } else {
            return PluralRuleList { rules: Arc::from([]) };
        };
        PluralRuleList::get_instance_of(&name).await
    }

    /// The list of a CLDR locale name (`pt_PT`, `ru`); panics for one CLDR doesn't have (Kotlin's `!!`).
    pub async fn get_instance_of(cldr_locale_name: &str) -> PluralRuleList {
        let index = index_of(cldr_locale_name).unwrap_or_else(|| panic!("NullPointerException: {cldr_locale_name}"));
        let loaded: Result<PluralRuleList, std::convert::Infallible> = RULE_LISTS
            .get_or_load(index, || async move {
                let rules: Vec<PluralRule> = CLDR_PLURAL_RULE_LISTS[index]
                    .iter()
                    .map(|(c, r)| PluralRule::new(*c, r).expect("CLDR's rules parse"))
                    .collect();
                Ok(PluralRuleList { rules: rules.into() })
            })
            .await;
        match loaded {
            Ok(list) => list,
        }
    }

    /// Its rules.
    pub fn rules(&self) -> &[PluralRule] {
        &self.rules
    }
}

fn index_of(name: &str) -> Option<usize> {
    CLDR_PLURAL_RULE_LIST_INDEX_BY_LOCALE.iter().find(|(n, _)| *n == name).map(|(_, i)| *i)
}
