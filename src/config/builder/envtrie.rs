//! Notes
//! - Length of matching path takes precedence over the strength of any one segment.
//! - Weights of each segment are determined by taking sets of mutually exclusive
//!   segments and creating a DAG to determine weights.
//! - No current design for making a short path win out over a longer one.

use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

/// Errors that may occur while building or evaluating an [`EnvTrie`].
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    /// Cannot decide between two environments based on length and mutual exclusion.
    /// The two `String`s are the environment conditions, so the user can look at their
    /// configuration file and resolve the issue.
    #[error("\"{0}\" and \"{1}\" have equal weight. Consider a more specific condition for the preferred one or make them mutually exclusive")]
    Indecision(String, String),
    /// One [`Pile`](super::hoard::Pile) has the same combination of environments defined
    /// multiple times.
    #[error("The same condition is defined twice with paths {0} and {1}")]
    DoubleDefine(String, String),
    /// No environment exists with the given name, but a [`Pile`](super::hoard::Pile) thinks
    /// one does.
    #[error("\"{0}\" is not an environment that exists")]
    EnvironmentNotExist(String),
    /// No environments were parsed for a [`Pile`](super::hoard::Pile) entry.
    #[error("Parsed 0 environments")]
    NoEnvironments,
    /// One or more exclusivity lists combined form an exclusion cycle containing the given
    /// environment.
    #[error("Environment \"{0}\" is simultaneously preferred to and not preferred to another")]
    WeightCycle(String),
    /// The given condition string is improperly formatted
    ///
    /// The string contains at least one non-empty environment name and at least one empty one.
    #[error("Condition \"{0}\" contains empty environment. Make sure it does not start or end with {}, or have multiple consecutive {}", ENV_SEPARATOR, ENV_SEPARATOR)]
    EmptyEnvironment(String),
    /// A condition string contains two environment names that are considered mutually exclusive and
    /// will probably never happen.
    #[error("Condition \"{0}\" contains two mutually exclusive environments")]
    CombinedMutuallyExclusive(String),
}

/// A single node in an [`EnvTrie`].
#[derive(Clone, Debug, PartialEq)]
struct Node {
    score: usize,
    tree: Option<BTreeMap<String, Node>>,
    value: Option<String>,
    name: String,
}

fn merge_two_trees(
    mut acc: BTreeMap<String, Node>,
    other: BTreeMap<String, Node>,
) -> Result<BTreeMap<String, Node>, Error> {
    let _span = tracing::trace_span!("merge_two_trees", left = ?acc, right = ?other).entered();
    tracing::trace!("merging two trees");
    for (key, val) in other {
        let _span = tracing::trace_span!("merge_tree_node", %key).entered();
        let prev = acc.remove(&key);
        let node = match prev {
            None => {
                tracing::trace!("key only exists in other tree: moving",);
                val
            }
            Some(prev) => {
                tracing::trace!("key exists in both trees: merging");
                prev.merge_with(val)?
            }
        };
        tracing::trace!(?node, "inserting merged node for key {} into tree", key);
        acc.insert(key, node);
    }

    Ok(acc)
}

#[derive(Clone, Debug, PartialEq)]
#[allow(single_use_lifetimes)]
struct Evaluation<'a> {
    name: String,
    path: Option<&'a str>,
    scores: Vec<usize>,
}

impl<'a> Evaluation<'a> {
    fn is_better_match_than(&self, other: &Evaluation<'a>) -> Result<bool, Error> {
        match (self.path, other.path) {
            (_, None) => Ok(true),
            (None, Some(_)) => Ok(false),
            (Some(_), Some(_)) => match self.scores.len().cmp(&other.scores.len()) {
                std::cmp::Ordering::Less => Ok(false),
                std::cmp::Ordering::Greater => Ok(true),
                std::cmp::Ordering::Equal => {
                    let rel_score: i32 = self
                        .scores
                        .iter()
                        .zip(other.scores.iter())
                        .map(|(s, o)| match s.cmp(o) {
                            std::cmp::Ordering::Less => -1,
                            std::cmp::Ordering::Equal => 0,
                            std::cmp::Ordering::Greater => 1,
                        })
                        .sum();

                    match rel_score.cmp(&0) {
                        std::cmp::Ordering::Less => Ok(false),
                        std::cmp::Ordering::Greater => Ok(true),
                        std::cmp::Ordering::Equal => {
                            tracing::error!(
                                left_eval = ?self,
                                right_eval = ?other,
                                "cannot choose between {} and {}",
                                self.name,
                                other.name
                            );
                            Err(Error::Indecision(self.name.clone(), other.name.clone()))
                        }
                    }
                }
            },
        }
    }
}

impl Node {
    fn merge_with(self, other: Node) -> Result<Node, Error> {
        let _span = tracing::trace_span!("merge_nodes", left = ?self, right = ?other).entered();
        if let (Some(first), Some(second)) = (&self.value, &other.value) {
            // Make order of paths deterministic
            #[cfg(test)]
            let (first, second) = if first < second {
                (first, second)
            } else {
                (second, first)
            };

            return Err(Error::DoubleDefine(first.clone(), second.clone()));
        }

        tracing::trace!("getting merged value, preferring left");
        let value = self.value.or(other.value);

        let tree = if self.tree.is_none() || other.tree.is_none() {
            tracing::trace!(
                left_tree = ?self.tree,
                right_tree = ?other.tree,
                "do not need to merge subtrees"
            );
            self.tree.or(other.tree)
        } else {
            tracing::trace!("both nodes have subtrees: merging");
            // Unwrap is safe because the above if checked for None
            let self_tree = self.tree.unwrap();
            let other_tree = other.tree.unwrap();

            Some(merge_two_trees(self_tree, other_tree)?)
        };

        tracing::trace!("nodes are merged");
        Ok(Node {
            name: self.name,
            score: self.score,
            tree,
            value,
        })
    }

    fn get_evaluation(&self, envs: &BTreeMap<String, bool>) -> Result<Evaluation, Error> {
        let _span = tracing::trace_span!("evaluate_node", node = ?self, ?envs).entered();

        // Default evaluation if subtree does not exist
        let mut eval = Evaluation {
            name: String::new(),
            path: self.value.as_deref(),
            scores: vec![],
        };

        if let Some(tree) = &self.tree {
            let _span = tracing::trace_span!("evaluating_subtree", subtree = ?tree).entered();
            for (name, node) in tree {
                let _span = tracing::trace_span!(
                    "evaluating_subtree_node",
                    %name,
                    ?node
                )
                .entered();
                // Ignore non-matching envs.
                // Error on environments that don't exist.
                if !envs
                    .get(name)
                    .copied()
                    .ok_or_else(|| Error::EnvironmentNotExist(name.clone()))?
                {
                    tracing::trace!("environment {} does not match; skipping", name);
                    continue;
                }

                // Get evaluation of child node
                let node_eval = match node.get_evaluation(envs) {
                    Ok(node_eval) => node_eval,
                    Err(err) => match err {
                        Error::Indecision(left, right) => {
                            return Err(Error::Indecision(
                                if left.is_empty() {
                                    self.name.clone()
                                } else {
                                    format!("{}|{}", self.name, left)
                                },
                                if right.is_empty() {
                                    self.name.clone()
                                } else {
                                    format!("{}|{}", self.name, right)
                                },
                            ))
                        }
                        _ => return Err(err),
                    },
                };

                if node_eval.is_better_match_than(&eval)? {
                    tracing::trace!(
                        old_eval = ?eval,
                        new_eval = ?node_eval,
                        "found child node with a better match"
                    );

                    eval = node_eval;
                }
            }
        }

        if eval.name.is_empty() {
            eval.name = self.name.clone();
        } else {
            eval.name = format!("{}|{}", self.name, eval.name);
        }
        eval.scores.push(self.score);
        // Sort largest values first
        eval.scores.sort_unstable_by(|left, right| right.cmp(left));
        Ok(eval)
    }

    fn get_highest_path(&self, envs: &BTreeMap<String, bool>) -> Result<Option<&str>, Error> {
        tracing::trace!("evaluating envtrie for best matching path");
        let Evaluation { path, .. } = self.get_evaluation(envs)?;
        Ok(path)
    }
}

/// A Trie-like structure to help match against different environments and determine the
/// best-matching path.
///
/// One `EnvTrie` is created for every pile. One hoard may then have multiple `EnvTrie`s created
/// for it. This means that it is possible to have different sets of environments match for
/// different piles. That is, if one pile's `EnvTrie` matches on `"foo|bar"` and a second pile
/// does not have a configuration for `"foo|bar"`, it is possible that `"bar|baz"` is the best
/// match instead.
#[derive(Clone, Debug, PartialEq)]
pub struct EnvTrie(Node);

const ENV_SEPARATOR: char = '|';

fn validate_environments(environments: &BTreeMap<String, String>) -> Result<(), Error> {
    let _span = tracing::trace_span!("validate_environment_strings", ?environments).entered();
    for (key, _) in environments.iter() {
        tracing::trace_span!("check_env_str", env_str = %key);
        if key.is_empty() {
            tracing::error!("environment string is empty");
            return Err(Error::NoEnvironments);
        }

        for env in key.split(ENV_SEPARATOR) {
            if env.is_empty() {
                tracing::error!("environment string contains empty component");
                return Err(Error::EmptyEnvironment(key.to_string()));
            }
        }
    }

    Ok(())
}

fn get_weighted_map(exclusive_list: &[Vec<String>]) -> Result<BTreeMap<String, usize>, Error> {
    let _span = tracing::trace_span!(
        "get_weighted_map",
        exclusivity = ?exclusive_list
    )
    .entered();

    tracing::trace!("calculating environment weights from exclusivity lists");
    let mut score_dag = DiGraph::<String, ()>::new();

    for list in exclusive_list.iter() {
        let mut prev_idx = None;

        for node in list.iter().rev() {
            // Add node to graph
            let idx = score_dag.add_node(node.clone());

            // If not first node, create edge
            if let Some(prev) = prev_idx {
                // With reversing,  a list [a, b, c] will create a subgraph
                // (prev, idx) => c -> b -> a, i.e. from lowest to highest
                // priority.
                score_dag.add_edge(prev, idx, ());
            }

            prev_idx = Some(idx);
        }
    }

    toposort(&score_dag, None)
        .map(|v| {
            // Toposort returns least to highest priority, so the enumerated index
            // suffices as relative weight
            v.into_iter()
                .enumerate()
                .map(|(i, id)| (score_dag[id].clone(), i + 1))
                .collect()
        })
        .map_err(|cycle| {
            let node: &str = &score_dag[cycle.node_id()];
            Error::WeightCycle(node.to_owned())
        })
}

fn merge_maps(
    mut map1: BTreeMap<String, HashSet<String>>,
    map2: BTreeMap<String, HashSet<String>>,
) -> BTreeMap<String, HashSet<String>> {
    for (key, set) in map2 {
        let new_set = match map1.remove(&key) {
            None => set,
            Some(other_set) => set.union(&other_set).into_iter().cloned().collect(),
        };

        map1.insert(key, new_set);
    }

    map1
}

fn get_exclusivity_map(exclusivity_list: &[Vec<String>]) -> BTreeMap<String, HashSet<String>> {
    let _span = tracing::trace_span!(
        "get_exclusivity_map",
        exclusivity = ?exclusivity_list
    )
    .entered();

    tracing::trace!(
        "creating a mapping of environment names to mutually exclusive other environments"
    );
    exclusivity_list
        .iter()
        .map(|list| {
            list.iter()
                .map(|item| (item.clone(), list.iter().cloned().collect()))
                .collect()
        })
        .reduce(merge_maps)
        .unwrap_or_else(BTreeMap::new)
}

impl EnvTrie {
    /// Create a new [`EnvTrie`] from the given information.
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] relating to parsing or validating environment condition strings.
    pub fn new(
        environments: &BTreeMap<String, String>,
        exclusive_list: &[Vec<String>],
    ) -> Result<Self, Error> {
        let _span =
            tracing::trace_span!("create_envtrie", ?environments, ?exclusive_list).entered();
        tracing::trace!("creating a new envtrie");

        validate_environments(environments)?;
        let weighted_map = get_weighted_map(exclusive_list)?;
        let exclusivity_map = get_exclusivity_map(exclusive_list);

        // Building a list of linked lists that represent paths from the root of a tree to a leaf.
        tracing::trace!(?exclusivity_map, ?weighted_map, "building trees for each environment string");
        let trees: Vec<_> = environments
            .iter()
            .map(|(env_str, path)| {
                let _span =
                    tracing::trace_span!("process_env_string", string = %env_str, %path).entered();
                let mut envs: Vec<&str> = env_str.split(ENV_SEPARATOR).collect();
                envs.sort_unstable();

                // Check for mutually exclusive items
                tracing::trace!("checking for mutually exclusive items");
                for (i, env1) in envs.iter().enumerate() {
                    for env2 in envs.iter().skip(i + 1) {
                        if let Some(set) = exclusivity_map.get(*env1) {
                            if set.contains(*env2) {
                                return Err(Error::CombinedMutuallyExclusive(env_str.clone()));
                            }
                        }
                    }
                }

                // Last node, then building up to the root.
                let mut prev_node = Node {
                    name: String::new(),
                    score: 0,
                    tree: None,
                    value: Some(path.clone()),
                };

                // Reverse-build a linked list
                tracing::trace!("building environment tree");
                for segment in envs.into_iter().rev() {
                    let segment = segment.to_string();

                    prev_node.score = weighted_map.get(&segment).copied().unwrap_or(1);
                    prev_node.name = segment.clone();
                    let tree = {
                        let mut tree = BTreeMap::new();
                        tree.insert(segment, prev_node);
                        Some(tree)
                    };

                    prev_node = Node {
                        score: 0,
                        tree,
                        value: None,
                        name: String::new(),
                    };
                }

                Ok(prev_node)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut tree_iter = trees.into_iter();
        let first = tree_iter.next().ok_or(Error::NoEnvironments);

        tracing::trace!("merging trees into a single trie");
        tree_iter
            .fold(first, |acc, node| {
                // TODO: Use result flattening when stable
                match acc {
                    Err(err) => Err(err),
                    Ok(acc_node) => acc_node.merge_with(node),
                }
            })
            .map(EnvTrie)
    }

    /// Get the best-matched (highest-scoring) path in the `EnvTrie`.
    ///
    /// # Errors
    ///
    /// - [`Error::EnvironmentNotExist`] if one of the environments does not exist in the
    ///   `environments` argument.
    pub fn get_path(&self, environments: &BTreeMap<String, bool>) -> Result<Option<&str>, Error> {
        tracing::trace!(
            trie = ?self,
            ?environments,
            "getting best matching path with given environments"
        );
        let EnvTrie(node) = self;
        node.get_highest_path(environments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::btreemap;
    use once_cell::sync::Lazy;
    use std::collections::BTreeMap;

    // Every const has a name of the form `LABEL_<char>_<int>`.
    // All consts with the same `<char>` are mutually exclusive for the purposes of testing.
    const LABEL_A_1: &str = "a1";
    const LABEL_A_2: &str = "a2";
    const LABEL_A_3: &str = "a3";
    const LABEL_B_1: &str = "b1";
    const LABEL_B_2: &str = "b2";
    const LABEL_B_3: &str = "b3";
    const LABEL_C_1: &str = "c1";
    const LABEL_C_2: &str = "c2";

    static PATH_1: Lazy<String> = Lazy::new(|| String::from("/tmp/path1"));
    static PATH_2: Lazy<String> = Lazy::new(|| String::from("/tmp/path2"));
    static PATH_3: Lazy<String> = Lazy::new(|| String::from("/tmp/path3"));

    fn node_eq_ignore_score(trie: &Node, expected: &Node) -> bool {
        if trie.value != expected.value {
            return false;
        }

        match (&trie.tree, &expected.tree) {
            (None, None) => true,
            (None, Some(_)) | (Some(_), None) => false,
            (Some(tree1), Some(tree2)) => {
                if tree1.len() != tree2.len() {
                    return false;
                }

                for (key, node1) in tree1.iter() {
                    let equal = match tree2.get(key) {
                        None => false,
                        Some(node2) => node_eq_ignore_score(node1, node2),
                    };

                    if !equal {
                        return false;
                    }
                }

                true
            }
        }
    }

    macro_rules! trie_test_ignore_score {
        (name: $name:ident, environments: $envs:expr, exclusivity: $excl:expr, expected: $result:expr) => {
            #[test]
            fn $name() {
                let environments: BTreeMap<String, String> = $envs;
                let exclusivity: Vec<Vec<String>> = $excl;

                let res: Result<EnvTrie, Error> = EnvTrie::new(&environments, &exclusivity);
                let expected: Result<EnvTrie, Error> = $result;

                match (res, expected) {
                    (Ok(trie), Err(err)) => {
                        panic!("expected error\n{:#?},\ngot trie\n{:#?}", err, trie)
                    }
                    (Err(err), Ok(trie)) => {
                        panic!("expected trie\n{:#?},\ngot error\n{:#?}", trie, err)
                    }
                    (Ok(EnvTrie(node1)), Ok(EnvTrie(node2))) => if !node_eq_ignore_score(&node1, &node2) {
                        panic!("received trie did not match expected\nReceived: {:#?}\nExpected: {:#?}", node1, node2)
                    },
                    (Err(err), Err(exp)) => assert_eq!(
                        err, exp,
                        "received (left) error does not match expected (right) error"
                    ),
                }
            }
        };
    }

    trie_test_ignore_score! {
        name: test_valid_single_env,
        environments: btreemap! {
            LABEL_A_1.into() => PATH_1.clone(),
            LABEL_B_1.into() => PATH_2.clone(),
            LABEL_C_1.into() => PATH_3.clone(),
        },
        exclusivity: vec![],
        expected: {
            let node = Node {
                name: String::new(),
                score: 1,
                value: None,
                tree: Some(btreemap!{
                    LABEL_A_1.to_owned() => Node {
                        name: LABEL_A_1.to_owned(),
                        score: 1,
                        tree: None,
                        value: Some(PATH_1.clone()),
                    },
                    LABEL_B_1.to_owned() => Node {
                        name: LABEL_B_1.to_owned(),
                        score: 1,
                        tree: None,
                        value: Some(PATH_2.clone()),
                    },
                    LABEL_C_1.to_owned() => Node {
                        name: LABEL_C_1.to_owned(),
                        score: 1,
                        tree: None,
                        value: Some(PATH_3.clone()),
                    },
                })
            };
            Ok(EnvTrie(node))
        }
    }

    trie_test_ignore_score! {
        name: test_valid_multi_env,
        environments: btreemap! {
            format!("{}|{}|{}", LABEL_A_1, LABEL_B_1, LABEL_C_1) => PATH_1.clone(),
            // Testing merged trees
            format!("{}|{}|{}", LABEL_A_1, LABEL_B_2, LABEL_C_1) => PATH_2.clone(),
            // The generated tree should be in sorted order
            format!("{}|{}|{}", LABEL_B_3, LABEL_A_3, LABEL_C_2) => PATH_3.clone(),
            // Testing overlapping trees
            format!("{}|{}", LABEL_A_3, LABEL_B_3) => PATH_2.clone(),
        },
        exclusivity: vec![
            vec![LABEL_A_1.into(), LABEL_A_2.into(), LABEL_A_3.into()],
            vec![LABEL_B_1.into(), LABEL_B_2.into(), LABEL_B_3.into()],
            vec![LABEL_C_1.into(), LABEL_C_2.into()],
        ],
        expected: {
            let node = Node {
                name: String::new(),
                score: 1,
                value: None,
                tree: Some(btreemap! {
                    LABEL_A_1.into() => Node {
                        name: LABEL_A_1.to_owned(),
                        score: 1,
                        value: None,
                        tree: Some(btreemap!{
                            LABEL_B_1.into() => Node {
                                name: LABEL_B_1.to_owned(),
                                score: 1,
                                value: None,
                                tree: Some(btreemap!{
                                    LABEL_C_1.into() => Node {
                                        name: LABEL_C_1.to_owned(),
                                        score: 1,
                                        tree: None,
                                        value: Some(PATH_1.clone()),
                                    }
                                })
                            },
                            LABEL_B_2.into() => Node {
                                name: LABEL_B_2.to_owned(),
                                score: 1,
                                value: None,
                                tree: Some(btreemap!{
                                    LABEL_C_1.into() => Node {
                                        name: LABEL_C_1.to_owned(),
                                        score: 1,
                                        tree: None,
                                        value: Some(PATH_2.clone())
                                    }
                                })
                            }
                        })
                    },
                    LABEL_A_3.into() => Node {
                        name: LABEL_A_3.to_owned(),
                        score: 1,
                        value: None,
                        tree: Some(btreemap! {
                            LABEL_B_3.into() => Node {
                                name: LABEL_B_3.to_owned(),
                                score: 1,
                                value: Some(PATH_2.clone()),
                                tree: Some(btreemap! {
                                    LABEL_C_2.into() => Node {
                                        name: LABEL_C_2.to_owned(),
                                        score: 1,
                                        tree: None,
                                        value: Some(PATH_3.clone()),
                                    }
                                })
                            }
                        })
                    },
                })
            };

            Ok(EnvTrie(node))
        }
    }

    trie_test_ignore_score! {
        name: test_invalid_separator_prefix,
        environments: btreemap! {
            format!("|{}|{}", LABEL_A_1, LABEL_B_1) => PATH_1.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::EmptyEnvironment(format!("|{}|{}", LABEL_A_1, LABEL_B_1)))
    }

    trie_test_ignore_score! {
        name: test_invalid_separator_suffix,
        environments: btreemap! {
            format!("{}|{}|", LABEL_A_1, LABEL_B_1) => PATH_1.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::EmptyEnvironment(format!("{}|{}|", LABEL_A_1, LABEL_B_1)))
    }

    trie_test_ignore_score! {
        name: test_invalid_consecutive_separator,
        environments: btreemap! {
            format!("{}||{}", LABEL_A_1, LABEL_B_1) => PATH_1.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::EmptyEnvironment(format!("{}||{}", LABEL_A_1, LABEL_B_1)))
    }

    trie_test_ignore_score! {
        name: test_combine_mutually_exclusive_is_invalid,
        environments: btreemap! {
            format!("{}|{}", LABEL_A_1, LABEL_A_2) => PATH_1.clone(),
        },
        exclusivity: vec![vec![LABEL_A_1.into(), LABEL_A_2.into()]],
        expected: Err(Error::CombinedMutuallyExclusive(format!("{}|{}", LABEL_A_1, LABEL_A_2)))
    }

    trie_test_ignore_score! {
        name: test_same_condition_twice_is_invalid,
        environments: btreemap! {
            format!("{}|{}", LABEL_A_1, LABEL_B_1) => PATH_1.clone(),
            format!("{}|{}", LABEL_B_1, LABEL_A_1) => PATH_2.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::DoubleDefine(PATH_1.clone(), PATH_2.clone()))
    }

    trie_test_ignore_score! {
        name: test_empty_condition_is_invalid,
        environments: btreemap! {
            "".into() => PATH_1.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::NoEnvironments)
    }
}
