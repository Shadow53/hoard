//! Notes
//! - Length of matching path takes precedence over the strength of any one segment.
//! - Weights of each segment are determined by taking sets of mutually exclusive
//!   segments and creating a DAG to determine weights.
//! - No current design for making a short path win out over a longer one.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};

use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use tap::TapFallible;
use thiserror::Error;

use crate::env_vars::PathWithEnv;
use crate::newtypes::{EnvironmentName, EnvironmentString};

/// Errors that may occur while building or evaluating an [`EnvTrie`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    /// Cannot decide between two environment strings based on length and mutual exclusion.
    #[error("\"{0}\" and \"{1}\" have equal weight. Consider a more specific condition for the preferred one or make them mutually exclusive")]
    Indecision(EnvironmentString, EnvironmentString),
    /// One [`Pile`](super::hoard::Pile) has the same combination of environments defined
    /// multiple times.
    #[error("The same condition is defined twice with paths {0} and {1}")]
    DoubleDefine(PathWithEnv, PathWithEnv),
    /// No environment exists with the given name, but a [`Pile`](super::hoard::Pile) thinks
    /// one does.
    #[error("\"{0}\" is not an environment that exists")]
    EnvironmentNotExist(EnvironmentName),
    /// No environments were parsed for a [`Pile`](super::hoard::Pile) entry.
    #[error("Parsed 0 environments")]
    NoEnvironments,
    /// One or more exclusivity lists combined form an exclusion cycle containing the given
    /// environment.
    #[error("Environment \"{0}\" is simultaneously preferred to and not preferred to another")]
    WeightCycle(EnvironmentName),
    /// A condition string contains two environment names that are considered mutually exclusive and
    /// will probably never happen.
    #[error("Condition \"{0}\" contains two mutually exclusive environments")]
    CombinedMutuallyExclusive(EnvironmentString),
}

/// A single node in an [`EnvTrie`].
#[derive(Clone, Debug, PartialEq)]
struct Node {
    score: usize,
    tree: Option<BTreeMap<EnvironmentName, Node>>,
    value: Option<PathWithEnv>,
    name: EnvironmentName,
}

#[tracing::instrument(level = "trace")]
fn merge_two_trees(
    mut acc: BTreeMap<EnvironmentName, Node>,
    other: BTreeMap<EnvironmentName, Node>,
) -> Result<BTreeMap<EnvironmentName, Node>, Error> {
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
    name: EnvironmentString,
    path: Option<&'a PathWithEnv>,
    scores: Vec<usize>,
}

impl<'a> Evaluation<'a> {
    #[tracing::instrument]
    fn is_better_match_than(&self, other: &Evaluation<'a>) -> Result<bool, Error> {
        match (self.path, other.path) {
            (_, None) => Ok(true),
            (None, Some(_)) => Ok(false),
            (Some(_), Some(_)) => match self.scores.len().cmp(&other.scores.len()) {
                Ordering::Less => Ok(false),
                Ordering::Greater => Ok(true),
                Ordering::Equal => {
                    let rel_score: i32 = self
                        .scores
                        .iter()
                        .zip(other.scores.iter())
                        .map(|(s, o)| match s.cmp(o) {
                            Ordering::Less => -1,
                            Ordering::Equal => 0,
                            Ordering::Greater => 1,
                        })
                        .sum();

                    match rel_score.cmp(&0) {
                        Ordering::Less => Ok(false),
                        Ordering::Greater => Ok(true),
                        Ordering::Equal => crate::create_log_error(Error::Indecision(
                            self.name.clone(),
                            other.name.clone(),
                        )),
                    }
                }
            },
        }
    }
}

impl Node {
    #[tracing::instrument(level = "trace", name = "merge_nodes")]
    fn merge_with(self, other: Node) -> Result<Node, Error> {
        if let (Some(first), Some(second)) = (&self.value, &other.value) {
            // Make order of paths deterministic
            #[cfg(test)]
            let (first, second) = if first < second {
                (first, second)
            } else {
                (second, first) // grcov: ignore
            };

            return crate::create_log_error(Error::DoubleDefine(first.clone(), second.clone()));
        }

        tracing::trace!("getting merged value, preferring left");
        let value = self.value.or(other.value);

        let tree = if self.tree.is_none() || other.tree.is_none() {
            // grcov: ignore-start
            tracing::trace!(
                left_tree = ?self.tree,
                right_tree = ?other.tree,
                "do not need to merge subtrees"
            );
            // grcov: ignore-end
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

    #[tracing::instrument(level = "trace")]
    fn get_evaluation(&self, envs: &BTreeMap<EnvironmentName, bool>) -> Result<Evaluation, Error> {
        // Default evaluation if subtree does not exist
        let mut eval = Evaluation {
            name: EnvironmentString::from(self.name.clone()),
            path: None,
            scores: vec![self.score],
        };

        if envs
            .get(&self.name)
            .copied()
            .ok_or_else(|| Error::EnvironmentNotExist(self.name.clone()))?
        {
            eval.path = self.value.as_ref();
        } else {
            return Ok(eval);
        }

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
                        Error::Indecision(mut left, mut right) => {
                            // Because this is recursively crafted, better to report at call site.
                            return Err(Error::Indecision(
                                {
                                    left.insert(self.name.clone());
                                    left
                                },
                                {
                                    right.insert(self.name.clone());
                                    right
                                },
                            ));
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

        eval.scores.push(self.score);
        // Sort largest values first
        eval.scores.sort_unstable_by(|left, right| right.cmp(left));
        Ok(eval)
    }

    #[tracing::instrument(level = "trace")]
    fn get_highest_path(
        &self,
        envs: &BTreeMap<EnvironmentName, bool>,
    ) -> Result<Option<&PathWithEnv>, Error> {
        tracing::trace!("evaluating envtrie for best matching path");
        let Evaluation { path, .. } = self.get_evaluation(envs).tap_err(crate::tap_log_error)?;
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
pub struct EnvTrie(BTreeMap<EnvironmentName, Node>);

#[tracing::instrument(level = "trace")]
fn get_weighted_map(
    exclusive_list: &[Vec<EnvironmentName>],
) -> Result<BTreeMap<EnvironmentName, usize>, Error> {
    // Check for cycles, then discard graph
    tracing::trace!("checking for cycles");
    let mut score_dag = DiGraph::<EnvironmentName, ()>::new();
    for list in exclusive_list.iter() {
        let mut prev_idx = None;

        for name in list.iter().rev() {
            // Add node to graph
            let idx = score_dag.add_node(name.clone());

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
        .map_err(|cycle| {
            let node: &EnvironmentName = &score_dag[cycle.node_id()];
            Error::WeightCycle(node.clone())
        })
        .tap_err(crate::tap_log_error)?;

    // Actually calculate map
    tracing::trace!("calculating environment weights from exclusivity lists");
    let mut weighted_map: BTreeMap<EnvironmentName, usize> = BTreeMap::new();

    for list in exclusive_list {
        for (score, item) in list.iter().rev().enumerate() {
            // Scores should start a 1, not 0
            let score = score + 1;
            let weight =
                weighted_map
                    .get(item)
                    .map_or(score, |val| if *val > score { *val } else { score });
            weighted_map.insert(item.clone(), weight);
        }
    }

    Ok(weighted_map)
}

fn merge_maps(
    mut map1: BTreeMap<EnvironmentName, HashSet<EnvironmentName>>,
    map2: BTreeMap<EnvironmentName, HashSet<EnvironmentName>>,
) -> BTreeMap<EnvironmentName, HashSet<EnvironmentName>> {
    for (key, set) in map2 {
        let new_set = match map1.remove(&key) {
            None => set,
            Some(other_set) => set.union(&other_set).into_iter().cloned().collect(),
        };

        map1.insert(key, new_set);
    }

    map1
}

#[tracing::instrument(level = "trace")]
fn get_exclusivity_map(
    exclusivity_list: &[Vec<EnvironmentName>],
) -> BTreeMap<EnvironmentName, HashSet<EnvironmentName>> {
    // grcov: ignore-start
    tracing::trace!(
        "creating a mapping of environment names to mutually exclusive other environments"
    );
    // grcov: ignore-end

    exclusivity_list
        .iter()
        .map(|list| {
            list.iter()
                .map(|item| (item.clone(), list.iter().cloned().collect()))
                .collect()
        })
        .reduce(merge_maps)
        .unwrap_or_default()
}

impl EnvTrie {
    /// Create a new [`EnvTrie`] from the given information.
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] relating to parsing or validating environment condition strings.
    #[tracing::instrument(level = "trace", name = "new_envtrie")]
    pub fn new(
        envs: &BTreeMap<EnvironmentString, PathWithEnv>,
        exclusive_list: &[Vec<EnvironmentName>],
    ) -> Result<Self, Error> {
        tracing::trace!("creating a new envtrie");

        let weighted_map = get_weighted_map(exclusive_list)?;
        let exclusivity_map = get_exclusivity_map(exclusive_list);

        // grcov: ignore-start
        // Building a list of linked lists that represent paths from the root of a tree to a leaf.
        tracing::trace!(
            ?exclusivity_map,
            ?weighted_map,
            "building trees for each environment string"
        );
        // grcov: ignore-end

        let nodes: Vec<_> = envs
            .iter()
            .map(|(env_str, path)| {
                let _span =
                    tracing::trace_span!("process_env_string", string = %env_str, %path).entered();

                // Check for mutually exclusive items
                tracing::trace!("checking for mutually exclusive items");
                for (i, env1) in env_str.iter().enumerate() {
                    for env2 in env_str.iter().skip(i + 1) {
                        if let Some(set) = exclusivity_map.get(env1) {
                            if set.contains(env2) {
                                return crate::create_log_error(Error::CombinedMutuallyExclusive(
                                    env_str.clone(),
                                ));
                            }
                        }
                    }
                }

                let mut env_iter = env_str.into_iter().cloned().rev();
                let mut prev_node = match env_iter.next() {
                    None => return crate::create_log_error(Error::NoEnvironments),
                    Some(name) => Node {
                        name,
                        score: 1,
                        tree: None,
                        value: Some(path.clone()),
                    },
                };

                // Reverse-build a linked list
                tracing::trace!("building environment tree");
                for segment in env_iter {
                    prev_node = Node {
                        score: weighted_map.get(&segment).copied().unwrap_or(1),
                        name: segment.clone(),
                        tree: {
                            let mut tree = BTreeMap::new();
                            tree.insert(prev_node.name.clone(), prev_node);
                            Some(tree)
                        },
                        value: None,
                    };
                }

                Ok(prev_node)
            })
            .collect::<Result<Vec<_>, _>>()?;

        tracing::trace!("merging trees into a single trie");
        let tree =
            nodes
                .into_iter()
                .fold(Ok(BTreeMap::<EnvironmentName, Node>::new()), |acc, node| {
                    // TODO: Use result flattening when stable
                    match acc {
                        Err(err) => Err(err),
                        Ok(mut tree) => {
                            // Explicitly call `drop()` to drop any old value.
                            match tree.remove(&node.name) {
                                None => drop(tree.insert(node.name.clone(), node)),
                                Some(existing) => {
                                    let new_node = existing.merge_with(node)?;
                                    drop(tree.insert(new_node.name.clone(), new_node));
                                }
                            }
                            Ok(tree)
                        }
                    }
                })?;
        Ok(EnvTrie(tree))
    }

    /// Get the best-matched (highest-scoring) path in the `EnvTrie`.
    ///
    /// # Errors
    ///
    /// - [`Error::EnvironmentNotExist`] if one of the environments does not exist in the
    ///   `environments` argument.
    #[tracing::instrument(level = "trace", name = "get_path_from_envtrie")]
    pub fn get_path(
        &self,
        environments: &BTreeMap<EnvironmentName, bool>,
    ) -> Result<Option<&PathWithEnv>, Error> {
        tracing::trace!(
            trie = ?self,
            ?environments,
            "getting best matching path with given environments"
        );
        self.0
            .iter()
            .filter_map(|(env, node)| {
                node.get_highest_path(environments)
                    .transpose()
                    .map(|path| (env, node, path))
            })
            .fold(Ok(None), |acc, (_, node, path)| match (acc, path) {
                (Err(err), _) | (_, Err(err)) => Err(err),
                (Ok(None), Ok(path)) => Ok(Some((node, path))),
                (Ok(Some((acc, acc_path))), Ok(path)) => match acc.score.cmp(&node.score) {
                    Ordering::Equal => Err(Error::Indecision(
                        acc.name.clone().into(),
                        node.name.clone().into(),
                    )),
                    Ordering::Less => Ok(Some((node, path))),
                    Ordering::Greater => Ok(Some((acc, acc_path))),
                },
            })?
            .map(|(_, path)| path)
            .map(Ok)
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use maplit::btreemap;

    use super::*;

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

    const PATH_1: &str = "/tmp/path1";
    const PATH_2: &str = "/tmp/path2";
    const PATH_3: &str = "/tmp/path3";

    fn trie_eq_ignore_score(given: &EnvTrie, expected: &EnvTrie) -> bool {
        if given.0.len() != expected.0.len() {
            return false;
        }

        for (key, node1) in &given.0 {
            let equal = match expected.0.get(key) {
                None => false,
                Some(node2) => node_eq_ignore_score(node1, node2),
            };

            if !equal {
                return false;
            }
        }

        true
    }

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
                let environments: BTreeMap<EnvironmentString, PathWithEnv> = $envs;
                let exclusivity: Vec<Vec<EnvironmentName>> = $excl;

                let res: Result<EnvTrie, Error> = EnvTrie::new(&environments, &exclusivity);
                let expected: Result<EnvTrie, Error> = $result;

                match (res, expected) {
                    (Ok(trie), Err(err)) => {
                        panic!("expected error\n{:#?},\ngot trie\n{:#?}", err, trie)
                    }
                    (Err(err), Ok(trie)) => {
                        panic!("expected trie\n{:#?},\ngot error\n{:#?}", trie, err)
                    }
                    (Ok(trie), Ok(expected)) => if !trie_eq_ignore_score(&trie, &expected) {
                        panic!("received trie did not match expected\nReceived: {:#?}\nExpected: {:#?}", trie, expected)
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
            LABEL_A_1.parse().unwrap() => PATH_1.into(),
            LABEL_B_1.parse().unwrap() => PATH_2.into(),
            LABEL_C_1.parse().unwrap() => PATH_3.into(),
        },
        exclusivity: vec![],
        expected: {
            let tree = btreemap!{
                LABEL_A_1.parse().unwrap() => Node {
                    name: LABEL_A_1.parse().unwrap(),
                    score: 1,
                    tree: None,
                    value: Some(PATH_1.into()),
                },
                LABEL_B_1.parse().unwrap() => Node {
                    name: LABEL_B_1.parse().unwrap(),
                    score: 1,
                    tree: None,
                    value: Some(PATH_2.into()),
                },
                LABEL_C_1.parse().unwrap() => Node {
                    name: LABEL_C_1.parse().unwrap(),
                    score: 1,
                    tree: None,
                    value: Some(PATH_3.into()),
                },
            };
            Ok(EnvTrie(tree))
        }
    }

    trie_test_ignore_score! {
        name: test_valid_multi_env,
        environments: btreemap! {
            format!("{LABEL_A_1}|{LABEL_B_1}|{LABEL_C_1}").parse().unwrap() => PATH_1.into(),
            // Testing merged trees
            format!("{LABEL_A_1}|{LABEL_B_2}|{LABEL_C_1}").parse().unwrap() => PATH_2.into(),
            // The generated tree should be in sorted order
            format!("{LABEL_B_3}|{LABEL_A_3}|{LABEL_C_2}").parse().unwrap() => PATH_3.into(),
            // Testing overlapping trees
            format!("{LABEL_A_3}|{LABEL_B_3}").parse().unwrap() => PATH_2.into(),
        },
        exclusivity: vec![
            vec![LABEL_A_1.parse().unwrap(), LABEL_A_2.parse().unwrap(), LABEL_A_3.parse().unwrap()],
            vec![LABEL_B_1.parse().unwrap(), LABEL_B_2.parse().unwrap(), LABEL_B_3.parse().unwrap()],
            vec![LABEL_C_1.parse().unwrap(), LABEL_C_2.parse().unwrap()],
        ],
        expected: {
            let tree = btreemap! {
                LABEL_A_1.parse().unwrap() => Node {
                    name: LABEL_A_1.parse().unwrap(),
                    score: 1,
                    value: None,
                    tree: Some(btreemap!{
                        LABEL_B_1.parse().unwrap() => Node {
                            name: LABEL_B_1.parse().unwrap(),
                            score: 1,
                            value: None,
                            tree: Some(btreemap!{
                                LABEL_C_1.parse().unwrap() => Node {
                                    name: LABEL_C_1.parse().unwrap(),
                                    score: 1,
                                    tree: None,
                                    value: Some(PATH_1.into()),
                                }
                            })
                        },
                        LABEL_B_2.parse().unwrap() => Node {
                            name: LABEL_B_2.parse().unwrap(),
                            score: 1,
                            value: None,
                            tree: Some(btreemap!{
                                LABEL_C_1.parse().unwrap() => Node {
                                    name: LABEL_C_1.parse().unwrap(),
                                    score: 1,
                                    tree: None,
                                    value: Some(PATH_2.into())
                                }
                            })
                        }
                    })
                },
                LABEL_A_3.parse().unwrap() => Node {
                    name: LABEL_A_3.parse().unwrap(),
                    score: 1,
                    value: None,
                    tree: Some(btreemap! {
                        LABEL_B_3.parse().unwrap() => Node {
                            name: LABEL_B_3.parse().unwrap(),
                            score: 1,
                            value: Some(PATH_2.into()),
                            tree: Some(btreemap! {
                                LABEL_C_2.parse().unwrap() => Node {
                                    name: LABEL_C_2.parse().unwrap(),
                                    score: 1,
                                    tree: None,
                                    value: Some(PATH_3.into()),
                                }
                            })
                        }
                    })
                },
            };

            Ok(EnvTrie(tree))
        }
    }

    trie_test_ignore_score! {
        name: test_combine_mutually_exclusive_is_invalid,
        environments: btreemap! {
            format!("{LABEL_A_1}|{LABEL_A_2}").parse().unwrap() => PATH_1.into(),
        },
        exclusivity: vec![vec![LABEL_A_1.parse().unwrap(), LABEL_A_2.parse().unwrap()]],
        expected: Err(Error::CombinedMutuallyExclusive(format!("{LABEL_A_1}|{LABEL_A_2}").parse().unwrap()))
    }
}
