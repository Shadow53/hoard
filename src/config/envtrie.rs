//! Notes
//! - Length of matching path takes precedence over the strength of any one segment.
//! - If two paths have the same length and score, that is an error.
//! - Weights of each segment are determined by taking sets of mutually exclusive
//!   segments and creating a DAG to determine weights.
//! - No current design for making a short path win out over a longer one.

use crate::config::environment::Environment;
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::DiGraph;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum Error {
    #[error("The same condition is defined twice with paths {0} and {1}")]
    DoubleDefine(PathBuf, PathBuf),
    #[error("\"{0}\" is not an environment that exists")]
    EnvironmentNotExist(String),
    #[error("Parsed 0 environments")]
    NoEnvironments,
    #[error("Environment \"{0}\" is simultaneously preferred to and not preferred to another")]
    WeightCycle(String),
    #[error("Condition \"{0}\" contains empty environment. Make sure it does not start or end with {}, or have multiple consecutive {}", EnvTrie::ENV_SEPARATOR, EnvTrie::ENV_SEPARATOR)]
    EmptyEnvironment(String),
    #[error("Condition \"{0}\" contains two mutually exclusive environments")]
    CombinedMutuallyExclusive(String),
}

#[derive(Debug, PartialEq)]
struct Node {
    score: usize,
    tree: Option<BTreeMap<String, Node>>,
    value: Option<PathBuf>,
}

#[derive(Debug, PartialEq)]
pub struct EnvTrie(Node);

impl EnvTrie {
    const ENV_SEPARATOR: char = '|';

    fn validate_environments(environments: &HashMap<String, PathBuf>) -> Result<(), Error> {
        for (key, _) in environments.iter() {
            if key.is_empty() {
                return Err(Error::NoEnvironments);
            }

            for env in key.split(Self::ENV_SEPARATOR) {
                if env.is_empty() {
                    return Err(Error::EmptyEnvironment(key.to_string()));
                }
            }
        }

        Ok(())
    }

    fn get_sorted_list(exclusive_list: &[Vec<String>]) -> Result<Vec<String>, Error> {
        let mut score_dag = DiGraph::<String, ()>::new();

        for list in exclusive_list.iter() {
            let mut prev_idx = None;

            for node in list.iter().rev() {
                // Add node to graph
                let idx = score_dag.add_node(node.to_owned());

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
            .map(|v| v.into_iter().map(|id| score_dag[id].to_owned()).collect())
            .map_err(|cycle| {
                let node: &str = &score_dag[cycle.node_id()];
                Error::WeightCycle(node.to_owned())
            })
    }

    fn merge_hashmaps(
        mut map1: HashMap<String, HashSet<String>>,
        map2: HashMap<String, HashSet<String>>,
    ) -> HashMap<String, HashSet<String>> {
        for (key, set) in map2 {
            let new_set = match map1.remove(&key) {
                None => set,
                Some(other_set) => set.union(&other_set).into_iter().cloned().collect(),
            };

            map1.insert(key, new_set);
        }

        map1
    }

    fn get_exclusivity_map(exclusivity_list: &[Vec<String>]) -> HashMap<String, HashSet<String>> {
        exclusivity_list
            .iter()
            .map(|list| {
                list.iter()
                    .map(|item| (item.to_owned(), list.iter().cloned().collect()))
                    .collect()
            })
            .reduce(Self::merge_hashmaps)
            .unwrap_or_else(HashMap::new)
    }

    fn merge_two_nodes(acc: Node, other: Node) -> Result<Node, Error> {
        if let (Some(first), Some(second)) = (&acc.value, &other.value) {
            // Make order of paths deterministic
            #[cfg(test)]
            let (first, second) = if first < second {
                (first, second)
            } else {
                (second, first)
            };

            return Err(Error::DoubleDefine(first.to_owned(), second.to_owned()));
        }

        let value = acc.value.or(other.value);

        let tree = if acc.tree.is_none() || other.tree.is_none() {
            acc.tree.or(other.tree)
        } else {
            let acc_tree = acc.tree.unwrap();
            let other_tree = other.tree.unwrap();

            Some(Self::merge_two_trees(acc_tree, other_tree)?)
        };

        Ok(Node {
            score: acc.score,
            tree,
            value,
        })
    }

    fn merge_two_trees(
        mut acc: BTreeMap<String, Node>,
        other: BTreeMap<String, Node>,
    ) -> Result<BTreeMap<String, Node>, Error> {
        for (key, val) in other {
            let prev = acc.remove(&key);
            let node = match prev {
                None => val,
                Some(prev) => Self::merge_two_nodes(prev, val)?,
            };
            acc.insert(key, node);
        }

        Ok(acc)
    }

    pub fn new(
        environments: &HashMap<String, PathBuf>,
        exclusive_list: &[Vec<String>],
    ) -> Result<Self, Error> {
        Self::validate_environments(environments)?;
        let sorted_list = Self::get_sorted_list(exclusive_list)?;
        let exclusivity_map = Self::get_exclusivity_map(exclusive_list);

        // Building a list of linked lists that represent paths from the root of a tree to a leaf.
        let trees: Vec<_> = environments
            .iter()
            .map(|(env_str, path)| {
                let mut envs: Vec<&str> = env_str.split(Self::ENV_SEPARATOR).collect();
                envs.sort_unstable();

                // Check for mutually exclusive items
                for (i, env1) in envs.iter().enumerate() {
                    for env2 in envs.iter().skip(i + 1) {
                        if let Some(set) = exclusivity_map.get(*env1) {
                            if set.contains(*env2) {
                                return Err(Error::CombinedMutuallyExclusive(env_str.to_owned()));
                            }
                        }
                    }
                }

                // Last node, then building up to the root.
                let mut prev_node = Node {
                    score: 1,
                    tree: None,
                    value: Some(path.to_owned()),
                };

                // Reverse-build a linked list
                for segment in envs.into_iter().rev() {
                    let segment = segment.to_string();

                    let score = sorted_list
                        .iter()
                        .enumerate()
                        .filter_map(|(i, s)| {
                            if s.as_str() == segment.as_str() {
                                Some(i + 1) // avoid score of 0
                            } else {
                                None
                            }
                        })
                        .next()
                        .unwrap_or(1);

                    let tree = {
                        let mut tree = BTreeMap::new();
                        tree.insert(segment, prev_node);
                        Some(tree)
                    };

                    prev_node = Node {
                        score,
                        tree,
                        value: None,
                    };
                }

                Ok(prev_node)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut tree_iter = trees.into_iter();
        let first = tree_iter.next().ok_or(Error::NoEnvironments);

        tree_iter
            .fold(first, |acc, node| {
                // TODO: Use result flattening when stable
                match acc {
                    Err(err) => Err(err),
                    Ok(acc_node) => Self::merge_two_nodes(acc_node, node),
                }
            })
            .map(EnvTrie)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::{btreemap, hashmap};
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    use std::path::PathBuf;

    // Every const has a name of the form `LABEL_<char>_<int>`.
    // All consts with the same `<char>` are mutually exclusive for the purposes of testing.
    pub const LABEL_A_1: &str = "a1";
    pub const LABEL_A_2: &str = "a2";
    pub const LABEL_A_3: &str = "a3";
    pub const LABEL_B_1: &str = "b1";
    pub const LABEL_B_2: &str = "b2";
    pub const LABEL_B_3: &str = "b3";
    pub const LABEL_C_1: &str = "c1";
    pub const LABEL_C_2: &str = "c2";

    pub static PATH_1: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("/tmp/path1"));
    pub static PATH_2: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("/tmp/path2"));
    pub static PATH_3: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("/tmp/path3"));

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
                let environments: HashMap<String, PathBuf> = $envs;
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
        environments: hashmap! {
            LABEL_A_1.into() => PATH_1.clone(),
            LABEL_B_1.into() => PATH_2.clone(),
            LABEL_C_1.into() => PATH_3.clone(),
        },
        exclusivity: vec![],
        expected: {
            let node = Node {
                score: 1,
                value: None,
                tree: Some(btreemap!{
                    LABEL_A_1.to_owned() => Node {
                        score: 1,
                        tree: None,
                        value: Some(PATH_1.clone()),
                    },
                    LABEL_B_1.to_owned() => Node {
                        score: 1,
                        tree: None,
                        value: Some(PATH_2.clone()),
                    },
                    LABEL_C_1.to_owned() => Node {
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
        environments: hashmap! {
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
                score: 1,
                value: None,
                tree: Some(btreemap! {
                    LABEL_A_1.into() => Node {
                        score: 1,
                        value: None,
                        tree: Some(btreemap!{
                            LABEL_B_1.into() => Node {
                                score: 1,
                                value: None,
                                tree: Some(btreemap!{
                                    LABEL_C_1.into() => Node {
                                        score: 1,
                                        tree: None,
                                        value: Some(PATH_1.clone()),
                                    }
                                })
                            },
                            LABEL_B_2.into() => Node {
                                score: 1,
                                value: None,
                                tree: Some(btreemap!{
                                    LABEL_C_1.into() => Node {
                                        score: 1,
                                        tree: None,
                                        value: Some(PATH_2.clone())
                                    }
                                })
                            }
                        })
                    },
                    LABEL_A_3.into() => Node {
                        score: 1,
                        value: None,
                        tree: Some(btreemap! {
                            LABEL_B_3.into() => Node {
                                score: 1,
                                value: Some(PATH_2.clone()),
                                tree: Some(btreemap! {
                                    LABEL_C_2.into() => Node {
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
        environments: hashmap! {
            format!("|{}|{}", LABEL_A_1, LABEL_B_1) => PATH_1.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::EmptyEnvironment(format!("|{}|{}", LABEL_A_1, LABEL_B_1)))
    }

    trie_test_ignore_score! {
        name: test_invalid_separator_suffix,
        environments: hashmap! {
            format!("{}|{}|", LABEL_A_1, LABEL_B_1) => PATH_1.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::EmptyEnvironment(format!("{}|{}|", LABEL_A_1, LABEL_B_1)))
    }

    trie_test_ignore_score! {
        name: test_invalid_consecutive_separator,
        environments: hashmap! {
            format!("{}||{}", LABEL_A_1, LABEL_B_1) => PATH_1.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::EmptyEnvironment(format!("{}||{}", LABEL_A_1, LABEL_B_1)))
    }

    trie_test_ignore_score! {
        name: test_combine_mutually_exclusive_is_invalid,
        environments: hashmap! {
            format!("{}|{}", LABEL_A_1, LABEL_A_2) => PATH_1.clone(),
        },
        exclusivity: vec![vec![LABEL_A_1.into(), LABEL_A_2.into()]],
        expected: Err(Error::CombinedMutuallyExclusive(format!("{}|{}", LABEL_A_1, LABEL_A_2)))
    }

    trie_test_ignore_score! {
        name: test_same_condition_twice_is_invalid,
        environments: hashmap! {
            format!("{}|{}", LABEL_A_1, LABEL_B_1) => PATH_1.clone(),
            format!("{}|{}", LABEL_B_1, LABEL_A_1) => PATH_2.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::DoubleDefine(PATH_1.clone(), PATH_2.clone()))
    }

    trie_test_ignore_score! {
        name: test_empty_condition_is_invalid,
        environments: hashmap! {
            "".into() => PATH_1.clone(),
        },
        exclusivity: vec![],
        expected: Err(Error::NoEnvironments)
    }
}
