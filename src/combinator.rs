//! A `Combinator` provides a *newtype* to parse a two-dimensional list of items as a collection of
//! AND-ed and OR-ed elements. Every item in the outer list is OR-ed, while every item in an inner list
//! is AND-ed. That is, for
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

/// An internal container for the [`Combinator<T>`] type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum Inner<T: TryInto<bool>> {
    /// A single item that can be converted to a boolean.
    Single(T),
    /// Multiple items that can be converted to booleans.
    ///
    /// When the [`Inner<T>`] is evaluated, all of the booleans are AND-ed together.
    Multiple(Vec<T>),
}

impl<T> Inner<T>
where
    T: TryInto<bool>,
{
    /// Whether this [`Inner<T>`] contains a single item.
    ///
    /// This can be either an [`Inner<T>::Single`] or an [`Inner<T>::Multiple`] containing
    /// only one item.
    pub fn is_singleton(&self) -> bool {
        match self {
            Inner::Single(_) => true,
            Inner::Multiple(list) => list.len() == 1,
        }
    }

    /// Whether this [`Inner<T>`] is empty.
    ///
    /// This only applies if it is an [`Inner<T>::Multiple`] containing an empty list.
    pub fn is_empty(&self) -> bool {
        match self {
            Inner::Single(_) => false,
            Inner::Multiple(list) => list.is_empty(),
        }
    }
}

impl<T, E> TryFrom<Inner<T>> for bool
where
    T: TryInto<bool, Error = E>,
    E: Error,
{
    type Error = E;

    fn try_from(combinator: Inner<T>) -> Result<bool, Self::Error> {
        match combinator {
            Inner::Single(item) => item.try_into(),
            Inner::Multiple(list) => list
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, E>>()
                .map(|list| list.into_iter().all(|item| item)),
        }
    }
}

impl<T> fmt::Display for Inner<T>
where
    T: fmt::Display + TryInto<bool>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Inner::Single(item) => write!(f, "{}", item),
            Inner::Multiple(list) => {
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

/// A combination of things that can evaluate to `true` or `false`.
///
/// If at least one of the [`Inner<T>`] items evaluates to `true`, then entire `Combinator<T>`
/// will evaluate to `true`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[serde(transparent)]
pub struct Combinator<T: TryInto<bool>>(pub Vec<Inner<T>>);

impl<T> Combinator<T>
where
    T: Serialize + TryInto<bool>,
{
    /// Whether the [`Combinator<T>`] is empty.
    ///
    /// This can be if the list of [`Inner<T>`] is empty or if all of the contained
    /// [`Inner<T>`] are empty (think a list of empty lists).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty() || self.0.iter().all(Inner::is_empty)
    }

    /// Whether the [`Combinator<T>`] is a singleton.
    ///
    /// This can be any number of empty [`Inner<T>`] and exactly one that is a singleton.
    #[must_use]
    pub fn is_singleton(&self) -> bool {
        let mut iter = self.0.iter().filter(|inner| !inner.is_empty());
        match iter.next() {
            None => false,
            Some(inner) => inner.is_singleton() && iter.next().is_none(),
        }
    }

    /// Whether the [`Combinator<T>`] is only OR-ed items.
    ///
    /// This means the [`Combinator`] has at least two items that are OR-ed together; i.e.,
    /// it contains two or more [`Inner<T>`] that are all singletons.
    #[must_use]
    pub fn is_only_or(&self) -> bool {
        !self.is_empty() && !self.is_singleton() && self.0.iter().all(Inner::is_singleton)
    }

    /// Whether the [`Combinator<T>`] is only AND-ed items.
    ///
    /// This means the [`Combinator`] has at least two items that are AND-ed together; i.e.,
    /// it contains exactly one [`Inner<T>::Multiple`] with at least two items.
    #[must_use]
    pub fn is_only_and(&self) -> bool {
        self.0.len() == 1 && !self.is_empty() && !self.is_singleton()
    }

    /// Whether the [`Combinator<T>`] is complex.
    ///
    /// "Complex" here means that it contains at least three items with some combination of AND
    /// and OR.
    #[must_use]
    pub fn is_complex(&self) -> bool {
        !(self.is_empty() || self.is_singleton() || self.is_only_and() || self.is_only_or())
    }

    /// Convert this [`Combinator<T>`] to TOML.
    ///
    /// # Errors
    ///
    /// Any errors during the serialization process ([`toml::ser::Error`]).
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
            (true, Inner::Single(Tester(true))),
            (false, Inner::Single(Tester(false))),
            // CombinatorInner::Multiple should AND all items
            (
                true,
                Inner::Multiple(vec![Tester(true), Tester(true), Tester(true)]),
            ),
            (
                false,
                Inner::Multiple(vec![Tester(true), Tester(false), Tester(true)]),
            ),
        ];

        for (expected, input) in test_params {
            let result: bool = input.clone().try_into().unwrap();
            assert_eq!(
                result, expected,
                "CombinatorInner did not AND items correctly: {:?}", // grcov: ignore
                input // grcov: ignore
            );
        }
    }

    #[test]
    fn test_inner_serde_single() {
        let test_params_bool = vec![
            ([Token::Bool(true)], Inner::Single(true)),
            ([Token::Bool(false)], Inner::Single(false)),
        ];

        for (expected, input) in test_params_bool {
            assert_tokens(&input, &expected);
        }

        let test_params_tester = vec![
            ([Token::Bool(true)], Inner::Single(Tester(true))),
            ([Token::Bool(false)], Inner::Single(Tester(false))),
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
                Inner::Multiple(vec![Tester(true), Tester(false), Tester(false)]),
            ),
            (
                &[
                    Token::Seq { len: Some(3) },
                    Token::Bool(false),
                    Token::Bool(true),
                    Token::Bool(false),
                    Token::SeqEnd,
                ][..],
                Inner::Multiple(vec![Tester(false), Tester(true), Tester(false)]),
            ),
            (
                &[
                    Token::Seq { len: Some(1) },
                    Token::Bool(true),
                    Token::SeqEnd,
                ][..],
                Inner::Multiple(vec![Tester(true)]),
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

    #[allow(clippy::too_many_lines)]
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
                combinator: Combinator(vec![Inner::Multiple(vec![])]),
                // Empty should evaluate to true
                evaluates_to: true,
                expected_bool_str: "",
                typ: CombinatorType::Empty,
            },
            // BEGIN: Singleton
            TestItem {
                combinator: Combinator(vec![Inner::Single(Tester(true))]),
                evaluates_to: true,
                expected_bool_str: "true",
                typ: CombinatorType::Singleton,
            },
            TestItem {
                combinator: Combinator(vec![Inner::Single(Tester(false))]),
                evaluates_to: false,
                expected_bool_str: "false",
                typ: CombinatorType::Singleton,
            },
            // Containing a single Inner::Multiple with a single item counts as singleton
            TestItem {
                combinator: Combinator(vec![Inner::Multiple(vec![Tester(true)])]),
                evaluates_to: true,
                expected_bool_str: "true",
                typ: CombinatorType::Singleton,
            },
            TestItem {
                combinator: Combinator(vec![Inner::Multiple(vec![Tester(false)])]),
                evaluates_to: false,
                expected_bool_str: "false",
                typ: CombinatorType::Singleton,
            },
            // BEGIN: OnlyOr
            TestItem {
                combinator: Combinator(vec![
                    Inner::Single(Tester(true)),
                    Inner::Single(Tester(false)),
                ]),
                evaluates_to: true,
                expected_bool_str: "true OR false",
                typ: CombinatorType::OnlyOr,
            },
            TestItem {
                combinator: Combinator(vec![
                    Inner::Single(Tester(false)),
                    Inner::Single(Tester(false)),
                ]),
                evaluates_to: false,
                expected_bool_str: "false OR false",
                typ: CombinatorType::OnlyOr,
            },
            // Containing Inner::Multiples with only one item in them counts like a Single
            TestItem {
                combinator: Combinator(vec![
                    Inner::Single(Tester(false)),
                    Inner::Multiple(vec![Tester(true)]),
                ]),
                evaluates_to: true,
                expected_bool_str: "false OR true",
                typ: CombinatorType::OnlyOr,
            },
            TestItem {
                combinator: Combinator(vec![
                    Inner::Single(Tester(true)),
                    Inner::Multiple(vec![Tester(false)]),
                ]),
                evaluates_to: true,
                expected_bool_str: "true OR false",
                typ: CombinatorType::OnlyOr,
            },
            // BEGIN: OnlyAnd
            TestItem {
                combinator: Combinator(vec![Inner::Multiple(vec![
                    Tester(true),
                    Tester(true),
                    Tester(true),
                ])]),
                evaluates_to: true,
                expected_bool_str: "true AND true AND true",
                typ: CombinatorType::OnlyAnd,
            },
            TestItem {
                combinator: Combinator(vec![Inner::Multiple(vec![
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
                    Inner::Single(Tester(true)),
                    Inner::Multiple(vec![Tester(true), Tester(false)]),
                ]),
                evaluates_to: true,
                expected_bool_str: "true OR (true AND false)",
                typ: CombinatorType::Complex,
            },
            TestItem {
                combinator: Combinator(vec![
                    Inner::Single(Tester(false)),
                    Inner::Multiple(vec![Tester(true), Tester(false)]),
                ]),
                evaluates_to: false,
                expected_bool_str: "false OR (true AND false)",
                typ: CombinatorType::Complex,
            },
            TestItem {
                combinator: Combinator(vec![
                    Inner::Single(Tester(false)),
                    Inner::Multiple(vec![Tester(true), Tester(false), Tester(false)]),
                    Inner::Single(Tester(false)),
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
            let result: bool = case.combinator.clone().try_into().unwrap();
            assert_eq!(
                result, case.evaluates_to,
                "Combinator did not OR items correctly: {:?}", // grcov: ignore
                case.combinator // grcov: ignore
            );
        }
    }

    #[test]
    fn test_combinator_to_boolean_string() {
        for case in CASES.iter() {
            assert_eq!(
                case.expected_bool_str,
                &case.combinator.to_string(),
                "failed to create boolean string for {:?}", // grcov: ignore
                case.combinator // grcov: ignore
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
                "left value indicates whether {:?} should be empty", // grcov: ignore
                case.combinator // grcov: ignore
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
                "left value indicates whether {:?} should be only OR", // grcov: ignore
                case.combinator // grcov: ignore
            );
        }
    }

    #[test]
    fn test_is_only_and() {
        for case in CASES.iter() {
            assert_eq!(
                matches!(case.typ, CombinatorType::OnlyAnd),
                case.combinator.is_only_and(),
                "left value indicates whether {:?} should be only AND", // grcov: ignore
                case.combinator // grcov: ignore
            );
        }
    }

    #[test]
    fn test_is_complex() {
        for case in CASES.iter() {
            assert_eq!(
                matches!(case.typ, CombinatorType::Complex),
                case.combinator.is_complex(),
                "left value indicates whether {:?} should be complex", // grcov: ignore
                case.combinator // grcov: ignore
            );
        }
    }
}
