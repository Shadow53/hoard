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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_test::{assert_tokens, Token};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(transparent)]
    struct Tester(bool);

    impl From<Tester> for bool {
        fn from(Tester(b): Tester) -> bool {
            b
        }
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
                bool::from(input.clone()),
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

    #[test]
    fn test_combinator() {
        let test_params = vec![
            (
                true,
                Combinator(vec![CombinatorInner::Single(Tester(true))]),
            ),
            (
                false,
                Combinator(vec![CombinatorInner::Single(Tester(false))]),
            ),
            // Combinator should OR all items
            (
                true,
                Combinator(vec![
                    CombinatorInner::Single(Tester(true)),
                    CombinatorInner::Single(Tester(false)),
                    CombinatorInner::Multiple(vec![Tester(true), Tester(false), Tester(true)]),
                ]),
            ),
            (
                true,
                Combinator(vec![
                    CombinatorInner::Single(Tester(false)),
                    CombinatorInner::Single(Tester(false)),
                    CombinatorInner::Multiple(vec![Tester(true), Tester(true), Tester(true)]),
                ]),
            ),
            (
                false,
                Combinator(vec![
                    CombinatorInner::Single(Tester(false)),
                    CombinatorInner::Single(Tester(false)),
                    CombinatorInner::Multiple(vec![Tester(false), Tester(true), Tester(true)]),
                ]),
            ),
        ];

        for (expected, input) in test_params {
            assert_eq!(
                expected,
                bool::from(input.clone()),
                "Combinator did not OR items correctly: {:?}",
                input
            );
        }
    }
}

use serde::de::{self, Error, SeqAccess, Unexpected, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Formatter;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CombinatorInner<T: Into<bool>> {
    Single(T),
    Multiple(Vec<T>),
}

impl<T: Into<bool>> From<CombinatorInner<T>> for bool {
    fn from(combinator: CombinatorInner<T>) -> bool {
        match combinator {
            CombinatorInner::Single(item) => item.into(),
            CombinatorInner::Multiple(list) => list.into_iter().all(Into::into),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Combinator<T: Into<bool>>(Vec<CombinatorInner<T>>);

impl<T: Into<bool>> From<Combinator<T>> for bool {
    fn from(Combinator(combinator): Combinator<T>) -> bool {
        combinator.into_iter().any(Into::into)
    }
}
