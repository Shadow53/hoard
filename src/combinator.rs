//! A `Combinator` provides a *newtype* to parse a two-dimensional list of items as a collection of
//! ANDed and ORed elements. Every item in the outer list is ORed, while every item in an inner list
//! is ANDed. That is, for
//!
//! ```ignore
//! [ foo, bar, [baz, quux]]
//! ```
//!
//! The list will be parsed as `foo OR bar OR (baz AND quux)`.
//!
//! All AND/OR combinations must be expressed in this fashion. For example, to get `foo AND bar`,
//! you want `[[foo, bar]]`.
//!
//! It also implements `serde::Serialize` and `serde::Deserialize` for innermost types that implement
//! `Into<bool>` and the appropriate serde impl. No optimizations are applied to make later
//! evaluations easier.

use core::fmt;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::error::Error;
use std::fmt::Formatter;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CombinatorInner<T: TryInto<bool>> {
    Single(T),
    Multiple(Vec<T>),
}

impl<T> CombinatorInner<T>
where
    T: TryInto<bool>,
{
    pub fn is_singleton(&self) -> bool {
        match self {
            CombinatorInner::Single(_) => true,
            CombinatorInner::Multiple(list) => list.len() == 1,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            CombinatorInner::Single(_) => false,
            CombinatorInner::Multiple(list) => list.is_empty(),
        }
    }
}

impl<T, E> TryFrom<CombinatorInner<T>> for bool
where
    T: TryInto<bool, Error = E>,
    E: Error,
{
    type Error = E;

    fn try_from(combinator: CombinatorInner<T>) -> Result<bool, Self::Error> {
        match combinator {
            CombinatorInner::Single(item) => item.try_into(),
            CombinatorInner::Multiple(list) => list
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, E>>()
                .map(|list| list.into_iter().all(|item| item)),
        }
    }
}

impl<T> fmt::Display for CombinatorInner<T>
where
    T: fmt::Display + TryInto<bool>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CombinatorInner::Single(item) => write!(f, "{}", item),
            CombinatorInner::Multiple(list) => {
                let s = list.iter().map(ToString::to_string).reduce(|mut a, s| {
                    a.push_str(" AND ");
                    a.push_str(&s);
                    a
                });

                match s {
                    None => write!(f, ""),
                    Some(s) => write!(f, "{}", s),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Combinator<T: TryInto<bool>>(pub Vec<CombinatorInner<T>>);

impl<T> Combinator<T>
where
    T: Serialize + TryInto<bool>,
{
    pub fn is_empty(&self) -> bool {
        self.0.is_empty() || (self.0.len() == 1 && self.0.get(0).unwrap().is_empty())
    }

    pub fn is_singleton(&self) -> bool {
        self.0.len() == 1 && self.0.get(0).unwrap().is_singleton()
    }

    pub fn is_only_or(&self) -> bool {
        !self.is_empty() && !self.is_singleton() && self.0.iter().all(|item| item.is_singleton())
    }

    pub fn is_only_and(&self) -> bool {
        self.0.len() == 1 && !self.is_empty() && !self.is_singleton()
    }

    pub fn is_complex(&self) -> bool {
        self.0.len() > 1 && self.0.iter().any(|item| !item.is_singleton())
    }

    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(&self)
    }
}

impl<T> fmt::Display for Combinator<T>
where
    T: fmt::Display + TryInto<bool>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Combinator(list) = self;

        let is_one_item = list.len() == 1;
        let s = list
            .iter()
            .map(|item| {
                if item.is_singleton() || item.is_empty() || is_one_item {
                    item.to_string()
                } else {
                    format!("({})", item)
                }
            })
            .reduce(|mut a, s| {
                a.push_str(" OR ");
                a.push_str(&s);
                a
            });

        match s {
            None => write!(f, ""),
            Some(s) => write!(f, "{}", s),
        }
    }
}

impl<T, E> TryFrom<Combinator<T>> for bool
where
    T: TryInto<bool, Error = E>,
    E: Error,
{
    type Error = E;

    fn try_from(Combinator(combinator): Combinator<T>) -> Result<bool, Self::Error> {
        if combinator.is_empty() {
            return Ok(true);
        }

        combinator
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, E>>()
            .map(|list| list.into_iter().any(|item| item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use serde_test::{assert_tokens, Token};
    use std::convert::Infallible;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(transparent)]
    struct Tester(bool);

    impl TryFrom<Tester> for bool {
        type Error = Infallible;

        fn try_from(Tester(b): Tester) -> Result<bool, Self::Error> {
            Ok(b)
        }
    }

    impl fmt::Display for Tester {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    enum CombinatorType {
        Empty,
        Singleton,
        OnlyOr,
        OnlyAnd,
        Complex,
    }

    #[test]
    fn test_inner() {
        let test_params = vec![
            (true, CombinatorInner::Single(Tester(true))),
            (false, CombinatorInner::Single(Tester(false))),
            // CombinatorInner::Multiple should AND all items
            (
                true,
                CombinatorInner::Multiple(vec![Tester(true), Tester(true), Tester(true)]),
            ),
            (
                false,
                CombinatorInner::Multiple(vec![Tester(true), Tester(false), Tester(true)]),
            ),
        ];

        for (expected, input) in test_params {
            assert_eq!(
                expected,
                input.clone().try_into().unwrap(),
                "CombinatorInner did not AND items correctly: {:?}",
                input
            );
        }
    }

    #[test]
    fn test_inner_serde_single() {
        let test_params_bool = vec![
            ([Token::Bool(true)], CombinatorInner::Single(true)),
            ([Token::Bool(false)], CombinatorInner::Single(false)),
        ];

        for (expected, input) in test_params_bool {
            assert_tokens(&input, &expected);
        }

        let test_params_tester = vec![
            ([Token::Bool(true)], CombinatorInner::Single(Tester(true))),
            ([Token::Bool(false)], CombinatorInner::Single(Tester(false))),
        ];

        for (expected, input) in test_params_tester {
            assert_tokens(&input, &expected);
        }
    }

    #[test]
    fn test_inner_serde_multiple() {
        let test_params = vec![
            (
                &[
                    Token::Seq { len: Some(3) },
                    Token::Bool(true),
                    Token::Bool(false),
                    Token::Bool(false),
                    Token::SeqEnd,
                ][..],
                CombinatorInner::Multiple(vec![Tester(true), Tester(false), Tester(false)]),
            ),
            (
                &[
                    Token::Seq { len: Some(3) },
                    Token::Bool(false),
                    Token::Bool(true),
                    Token::Bool(false),
                    Token::SeqEnd,
                ][..],
                CombinatorInner::Multiple(vec![Tester(false), Tester(true), Tester(false)]),
            ),
            (
                &[
                    Token::Seq { len: Some(1) },
                    Token::Bool(true),
                    Token::SeqEnd,
                ][..],
                CombinatorInner::Multiple(vec![Tester(true)]),
            ),
        ];

        for (expected, input) in test_params {
            assert_tokens(&input, expected);
        }
    }

    struct TestItem {
        combinator: Combinator<Tester>,
        evaluates_to: bool,
        expected_bool_str: &'static str,
        typ: CombinatorType,
    }

    static CASES: Lazy<Vec<TestItem>> = Lazy::new(|| {
        vec![
            // BEGIN: Empty combinators
            TestItem {
                combinator: Combinator(vec![]),
                // Empty should evaluate to true
                evaluates_to: true,
                expected_bool_str: "",
                typ: CombinatorType::Empty,
            },
            // Containing only an empty Inner::Multiple counts as empty
            TestItem {
                combinator: Combinator(vec![CombinatorInner::Multiple(vec![])]),
                // Empty should evaluate to true
                evaluates_to: true,
                expected_bool_str: "",
                typ: CombinatorType::Empty,
            },
            // BEGIN: Singleton
            TestItem {
                combinator: Combinator(vec![CombinatorInner::Single(Tester(true))]),
                evaluates_to: true,
                expected_bool_str: "true",
                typ: CombinatorType::Singleton,
            },
            TestItem {
                combinator: Combinator(vec![CombinatorInner::Single(Tester(false))]),
                evaluates_to: false,
                expected_bool_str: "false",
                typ: CombinatorType::Singleton,
            },
            // Containing a single Inner::Multiple with a single item counts as singleton
            TestItem {
                combinator: Combinator(vec![CombinatorInner::Multiple(vec![Tester(true)])]),
                evaluates_to: true,
                expected_bool_str: "true",
                typ: CombinatorType::Singleton,
            },
            TestItem {
                combinator: Combinator(vec![CombinatorInner::Multiple(vec![Tester(false)])]),
                evaluates_to: false,
                expected_bool_str: "false",
                typ: CombinatorType::Singleton,
            },
            // BEGIN: OnlyOr
            TestItem {
                combinator: Combinator(vec![
                    CombinatorInner::Single(Tester(true)),
                    CombinatorInner::Single(Tester(false)),
                ]),
                evaluates_to: true,
                expected_bool_str: "true OR false",
                typ: CombinatorType::OnlyOr,
            },
            TestItem {
                combinator: Combinator(vec![
                    CombinatorInner::Single(Tester(false)),
                    CombinatorInner::Single(Tester(false)),
                ]),
                evaluates_to: false,
                expected_bool_str: "false OR false",
                typ: CombinatorType::OnlyOr,
            },
            // Containing Inner::Multiples with only one item in them counts like a Single
            TestItem {
                combinator: Combinator(vec![
                    CombinatorInner::Single(Tester(false)),
                    CombinatorInner::Multiple(vec![Tester(true)]),
                ]),
                evaluates_to: true,
                expected_bool_str: "false OR true",
                typ: CombinatorType::OnlyOr,
            },
            TestItem {
                combinator: Combinator(vec![
                    CombinatorInner::Single(Tester(true)),
                    CombinatorInner::Multiple(vec![Tester(false)]),
                ]),
                evaluates_to: true,
                expected_bool_str: "true OR false",
                typ: CombinatorType::OnlyOr,
            },
            // BEGIN: OnlyAnd
            TestItem {
                combinator: Combinator(vec![CombinatorInner::Multiple(vec![
                    Tester(true),
                    Tester(true),
                    Tester(true),
                ])]),
                evaluates_to: true,
                expected_bool_str: "true AND true AND true",
                typ: CombinatorType::OnlyAnd,
            },
            TestItem {
                combinator: Combinator(vec![CombinatorInner::Multiple(vec![
                    Tester(true),
                    Tester(false),
                    Tester(true),
                ])]),
                evaluates_to: false,
                expected_bool_str: "true AND false AND true",
                typ: CombinatorType::OnlyAnd,
            },
            // BEGIN: Complex
            TestItem {
                combinator: Combinator(vec![
                    CombinatorInner::Single(Tester(true)),
                    CombinatorInner::Multiple(vec![Tester(true), Tester(false)]),
                ]),
                evaluates_to: true,
                expected_bool_str: "true OR (true AND false)",
                typ: CombinatorType::Complex,
            },
            TestItem {
                combinator: Combinator(vec![
                    CombinatorInner::Single(Tester(false)),
                    CombinatorInner::Multiple(vec![Tester(true), Tester(false)]),
                ]),
                evaluates_to: false,
                expected_bool_str: "false OR (true AND false)",
                typ: CombinatorType::Complex,
            },
            TestItem {
                combinator: Combinator(vec![
                    CombinatorInner::Single(Tester(false)),
                    CombinatorInner::Multiple(vec![Tester(true), Tester(false), Tester(false)]),
                    CombinatorInner::Single(Tester(false)),
                ]),
                evaluates_to: false,
                expected_bool_str: "false OR (true AND false AND false) OR false",
                typ: CombinatorType::Complex,
            },
        ]
    });

    #[test]
    fn test_combinator() {
        for case in CASES.iter() {
            assert_eq!(
                case.evaluates_to,
                case.combinator.clone().try_into().unwrap(),
                "Combinator did not OR items correctly: {:?}",
                case.combinator
            );
        }
    }

    #[test]
    fn test_combinator_to_boolean_string() {
        for case in CASES.iter() {
            assert_eq!(
                case.expected_bool_str,
                &case.combinator.to_string(),
                "failed to create boolean string for {:?}",
                case.combinator
            );
        }
    }

    #[test]
    fn test_combinator_to_toml_string() {
        for case in CASES.iter() {
            let toml_str = toml::to_string(&case.combinator);
            assert_eq!(toml_str, case.combinator.to_toml_string());
        }
    }

    #[test]
    fn test_combinator_is_empty() {
        for case in CASES.iter() {
            assert_eq!(
                matches!(case.typ, CombinatorType::Empty),
                case.combinator.is_empty(),
                "left value indicates whether {:?} should be empty",
                case.combinator
            );
        }
    }

    #[test]
    fn test_is_singleton() {
        for case in CASES.iter() {
            assert_eq!(
                matches!(case.typ, CombinatorType::Singleton),
                case.combinator.is_singleton()
            );
        }
    }

    #[test]
    fn test_is_only_or() {
        for case in CASES.iter() {
            assert_eq!(
                matches!(case.typ, CombinatorType::OnlyOr),
                case.combinator.is_only_or(),
                "left value indicates whether {:?} should be only OR",
                case.combinator
            );
        }
    }

    #[test]
    fn test_is_only_and() {
        for case in CASES.iter() {
            assert_eq!(
                matches!(case.typ, CombinatorType::OnlyAnd),
                case.combinator.is_only_and(),
                "left value indicates whether {:?} should be only AND",
                case.combinator
            );
        }
    }

    #[test]
    fn test_is_complex() {
        for case in CASES.iter() {
            assert_eq!(
                matches!(case.typ, CombinatorType::Complex),
                case.combinator.is_complex(),
                "left value indicates whether {:?} should be complex",
                case.combinator
            );
        }
    }
}
