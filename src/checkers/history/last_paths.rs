//! Types dedicated to recording which paths were used for each pile in the last
//! operating using a given hoard.
//!
//! See the documentation for [`HoardPaths::enforce_old_and_new_piles_are_same`] for an
//! explanation of why this is useful.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tap::TapFallible;
use thiserror::Error;
use time::OffsetDateTime;
use tokio::{fs, io};

use crate::hoard::{Direction, Hoard};
use crate::newtypes::{HoardName, NonEmptyPileName};
use crate::paths::{HoardPath, RelativePath, SystemPath};

use super::super::Checker;

const FILE_NAME: &str = "last_paths.json";

/// Errors that may occur while working with a [`LastPaths`] or related types.
#[derive(Debug, Error)]
pub enum Error {
    /// An error while parsing the JSON file.
    #[error("could not parse {}: {0}", FILE_NAME)]
    Serde(#[from] serde_json::Error),
    /// An error while doing I/O.
    #[error("an I/O error occurred: {0}")]
    IO(#[from] io::Error),
    /// Unexpected differences in hoard paths. Operation must be forced to continue.
    #[error("paths used in current hoard operation do not match previous run")]
    HoardPathsMismatch,
    /// Expected the [`LastPaths`] to have at least one entry in it.
    #[error("LastPaths record has no entries in it!")]
    NoEntries,
}

/// Collection of the last paths matched per hoard.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastPaths(HashMap<HoardName, HoardPaths>);

impl<T> From<T> for LastPaths
where
    T: IntoIterator<Item = (HoardName, HoardPaths)>,
{
    fn from(other: T) -> Self {
        Self(other.into_iter().collect())
    }
}

#[tracing::instrument(level = "debug")]
async fn get_last_paths_file_path() -> Result<HoardPath, io::Error> {
    let id = super::get_or_generate_uuid().await?;
    Ok(super::get_history_dir_for_id(id).join(
        &RelativePath::try_from(PathBuf::from(FILE_NAME))
            .expect("file names are always valid RelativePaths"),
    ))
}

#[tracing::instrument(level = "debug")]
async fn read_last_paths_file() -> io::Result<Vec<u8>> {
    let path = get_last_paths_file_path().await?;
    tracing::debug!("opening lastpaths file at {}", path.display());
    fs::read(&path).await.tap_err(
        |error| tracing::error!(%error, "failed to read lastpaths file at {}", path.display()),
    )
}

#[async_trait::async_trait(? Send)]
impl Checker for LastPaths {
    type Error = Error;
    async fn new(
        _hoards_root: &HoardPath,
        name: &HoardName,
        hoard: &Hoard,
        _direction: Direction,
    ) -> Result<Self, Self::Error> {
        Ok(LastPaths({
            let mut map = HashMap::new();
            map.insert(name.clone(), HoardPaths::from(hoard.clone()));
            map
        }))
    }

    #[tracing::instrument(name = "last_paths_check")]
    async fn check(&mut self) -> Result<(), Self::Error> {
        let (name, new_hoard) = self.0.iter().next().ok_or(Error::NoEntries)?;

        let last_paths = LastPaths::from_default_file().await?;
        if let Some(old_hoard) = last_paths.hoard(name) {
            tracing::trace!(previous=?last_paths, "comparing against previous paths");
            HoardPaths::enforce_old_and_new_piles_are_same(old_hoard, new_hoard)?;
        }

        Ok(())
    }

    #[tracing::instrument(name = "last_paths_commit")]
    async fn commit_to_disk(self) -> Result<(), Self::Error> {
        let mut last_paths = LastPaths::from_default_file().await?;
        for (name, hoard) in self.0 {
            last_paths.set_hoard(name, hoard);
        }

        tracing::debug!("saving last paths to disk");
        let path = get_last_paths_file_path().await?;
        tracing::trace!("converting last paths to JSON");
        let content = serde_json::to_string(&last_paths).tap_err(|error| {
            tracing::error!(%error, "failed to serialize last paths record as JSON");
        })?;
        if let Some(parent) = path.parent() {
            tracing::trace!("ensuring parent directories exist");
            fs::create_dir_all(&parent).await.tap_err(|error| {
                tracing::error!(%error, "failed to create parent directory {}", parent.display());
            })?;
        }
        tracing::trace!("writing last paths file");
        fs::write(&path, content).await.tap_err(|error| {
            tracing::error!(%error, "failed to write last paths to {}", path.display());
        })?;
        Ok(())
    }
}

impl LastPaths {
    /// Get the entry for the given hoard, if exists.
    #[must_use]
    pub fn hoard(&self, hoard: &HoardName) -> Option<&HoardPaths> {
        self.0.get(hoard)
    }

    /// Set/overwrite the paths used for the given hoard.
    fn set_hoard(&mut self, hoard: HoardName, paths: HoardPaths) {
        self.0.insert(hoard, paths);
    }

    /// Read the last paths from the default file.
    ///
    /// # Errors
    ///
    /// Any I/O or `serde` error that occurs while reading and parsing the file.
    /// The exception is an I/O error with kind `NotFound`, which returns an empty
    /// `LastPaths`.
    #[tracing::instrument(level = "debug")]
    pub async fn from_default_file() -> Result<Self, Error> {
        match read_last_paths_file().await {
            Ok(content) => serde_json::from_slice(&content).map_err(|error| {
                tracing::error!(%error, "failed to parse last paths file as JSON");
                Error::from(error)
            }),
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    tracing::debug!("last paths file not found, creating new instance");
                    Ok(Self::default())
                } else {
                    Err(Error::from(err))
                }
            }
        }
    }
}

/// An entry for the last time a hoard was processed.
///
/// Contains the timestamp of the last operation on this hoard and a mapping
/// of every file in each of its piles to the corresponding path outside of the
/// hoard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoardPaths {
    /// The timestamp of the most recent operation on this [`Hoard`].
    pub timestamp: OffsetDateTime,
    /// A mapping of piles to the most recent path used.
    pub piles: PilePaths,
}

/// Internal type for [`HoardPaths`] mapping to anonymous or named piles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PilePaths {
    /// A single, anonymous pile's path.
    Anonymous(Option<SystemPath>),
    /// One or more named piles and their paths.
    Named(HashMap<NonEmptyPileName, SystemPath>),
}

impl From<SystemPath> for PilePaths {
    fn from(other: SystemPath) -> Self {
        Self::Anonymous(Some(other))
    }
}

impl From<Option<SystemPath>> for PilePaths {
    fn from(other: Option<SystemPath>) -> Self {
        Self::Anonymous(other)
    }
}

impl From<HashMap<NonEmptyPileName, SystemPath>> for PilePaths {
    fn from(other: HashMap<NonEmptyPileName, SystemPath>) -> Self {
        Self::Named(other)
    }
}

impl From<Hoard> for PilePaths {
    fn from(other: Hoard) -> Self {
        match other {
            Hoard::Anonymous(pile) => PilePaths::Anonymous(pile.path),
            Hoard::Named(named) => PilePaths::Named(
                named
                    .piles
                    .into_iter()
                    .filter_map(|(key, pile)| pile.path.map(|path| (key, path)))
                    .collect(),
            ),
        }
    }
}

impl<T> From<T> for HoardPaths
where
    T: Into<PilePaths>,
{
    fn from(val: T) -> Self {
        Self {
            timestamp: OffsetDateTime::now_utc(),
            piles: val.into(),
        }
    }
}

impl HoardPaths {
    /// Get the timestamp of the last operation on this hoard.
    #[must_use]
    pub fn time(&self) -> &OffsetDateTime {
        &self.timestamp
    }

    /// Get the entries for a pile by name.
    ///
    /// Returns `None` if the named pile is not found or if the hoard contains an
    /// anonymous pile.
    #[must_use]
    pub fn named_pile(&self, name: &NonEmptyPileName) -> Option<&SystemPath> {
        if let PilePaths::Named(named) = &self.piles {
            named.get(name)
        } else {
            None
        }
    }

    /// Get the entries for the anonymous pile.
    ///
    /// Returns `None` if the hoard contains named piles.
    #[must_use]
    pub fn anonymous_pile(&self) -> Option<&SystemPath> {
        if let PilePaths::Anonymous(path) = &self.piles {
            path.as_ref()
        } else {
            None
        }
    }

    /// Logs any inconsistencies and returns an error if any are found.
    ///
    /// This check basically returns an error if `old != new`, but does some extra checking to
    /// provided better logged warnings depending on what the mismatch is.
    ///
    /// Any mismatch is considered an inconsistency because it may mean that one of the following
    /// occurs because of an unexpected change in the system configuration:
    ///
    /// - Backing up an empty/non-existent directory unintentionally deletes the current backup.
    /// - Restoring to an unexpected directory causes nothing to change in the intended one.
    /// - Files don't get backed up or restored because the system configuration did not match a
    ///   pile's environment string.
    ///
    /// # Example
    ///
    /// I sometimes run a normal Steam installation on a Linux machine, sometimes the flatpak version.
    /// If the flatpak install takes priority over the normal install, the following set of events
    /// might occur:
    ///
    /// - Install flatpak Steam to try it out.
    /// - Realize I should back up my normal Steam saves so I can restore them to the flatpak locations.
    ///   - Alternatively, I told `hoard` to do this already and assumed it finished, but a different
    ///     hoard took longer than expected to back up.
    ///   - Either way, a backup happens after installing flatpak Steam.
    /// - Existing backup is erased so the new one is a clean backup.
    /// - The associated directories for flatpak are empty or don't exist, so nothing gets backed up.
    /// - My backups are all deleted.
    ///
    /// There are ways to recover from this, for example by uninstalling flatpak Steam and doing
    /// another backup, but the situation gets more complex when considering multiple devices
    /// each synchronizing files to each other. It's much easier to make this check and, if the
    /// changes are intended, have the user indicate as much.
    ///
    /// # Errors
    ///
    /// [`Error::HoardPathsMismatch`] if there is a difference between `old` and `new`.
    #[tracing::instrument(level = "trace")]
    pub fn enforce_old_and_new_piles_are_same(old: &Self, new: &Self) -> Result<(), Error> {
        tracing::debug!("comparing old and new piles' paths");
        tracing::trace!(?old, ?new);
        match (&old.piles, &new.piles) {
            (PilePaths::Anonymous(old), PilePaths::Anonymous(new)) => {
                tracing::trace!("both piles are anonymous");
                if old.is_some() && new.is_none() {
                    tracing::warn!(old_path=?old, "anonymous pile no longer has a path");
                    return Err(Error::HoardPathsMismatch);
                } else if old.is_none() && new.is_some() {
                    // TODO: This case may be necessary when restoring: consider.
                    tracing::warn!(new_path=?new, "anonymous pile matches a path but previously did not");
                    return Err(Error::HoardPathsMismatch);
                } else if let (Some(old), Some(new)) = (old, new) {
                    // If both are None, they are the same. So check only for both as Some(_).
                    // Then check if the paths match.
                    if old != new {
                        tracing::warn!(?old, ?new, "anonymous pile path changed");
                        return Err(Error::HoardPathsMismatch);
                    }
                }
            }
            (PilePaths::Anonymous(_), PilePaths::Named(_)) => {
                tracing::warn!("hoard previously with anonymous pile now has named pile(s)");
                return Err(Error::HoardPathsMismatch);
            }
            (PilePaths::Named(_), PilePaths::Anonymous(_)) => {
                tracing::warn!("hoard previously with named pile(s) now has an anonymous pile");
                return Err(Error::HoardPathsMismatch);
            }
            (PilePaths::Named(old), PilePaths::Named(new)) => {
                tracing::trace!("both piles are named");
                let old_set: HashSet<&NonEmptyPileName> = old.keys().collect();
                let new_set: HashSet<&NonEmptyPileName> = new.keys().collect();

                let only_in_old: Vec<&NonEmptyPileName> =
                    old_set.difference(&new_set).copied().collect();
                let only_in_new: Vec<&NonEmptyPileName> =
                    new_set.difference(&old_set).copied().collect();

                // Warn about both before returning.
                if !only_in_old.is_empty() {
                    tracing::warn!(piles=?only_in_old, "named piles previously with path no longer have a path");
                }
                if !only_in_new.is_empty() {
                    tracing::warn!(piles=?only_in_new, "named piles previously without path now have a path");
                }

                // Now return if either difference occurred.
                if !only_in_old.is_empty() || !only_in_new.is_empty() {
                    return Err(Error::HoardPathsMismatch);
                }

                // If all of the same piles exist, check if all the paths are the same.
                // Can expect because the above checks for any mismatched keys.
                let mut mismatch = false;
                for (key, old_path) in old {
                    let new_path = new.get(key).expect("key should exist in map");
                    if old_path != new_path {
                        mismatch = true;
                        tracing::warn!(
                            ?old_path,
                            ?new_path,
                            "pile \"{}\" has a different path",
                            key
                        );
                    }
                }

                if mismatch {
                    return Err(Error::HoardPathsMismatch);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use maplit::hashmap;

    use crate::test::system_path;

    use super::*;

    const NAMED_PILE_1: &str = "test1";
    const NAMED_PILE_2: &str = "test2";

    fn anonymous_hoard_paths() -> HoardPaths {
        HoardPaths::from(PilePaths::Anonymous(Some(system_path!("/test/path"))))
    }

    fn named_hoard_paths() -> HoardPaths {
        HoardPaths::from(PilePaths::Named(hashmap! {
            NAMED_PILE_1.parse().unwrap() => system_path!("/test/path"),
            NAMED_PILE_2.parse().unwrap() => system_path!("/test/other/path"),
        }))
    }

    #[test]
    fn default_lastpaths_is_empty() {
        let last_paths = LastPaths::default();
        assert_eq!(last_paths.0.len(), 0);
    }

    #[test]
    fn test_from_system_path() {
        let path = system_path!("/test/path");
        let pile_paths = PilePaths::from(path.clone());
        assert_eq!(pile_paths, PilePaths::Anonymous(Some(path)));
    }

    #[test]
    fn test_from_some_system_path() {
        let path = system_path!("/test/path");
        let pile_paths = PilePaths::from(Some(path.clone()));
        assert_eq!(pile_paths, PilePaths::Anonymous(Some(path)));
    }

    #[test]
    fn test_from_none_system_path() {
        let pile_paths = PilePaths::from(None);
        assert_eq!(pile_paths, PilePaths::Anonymous(None));
    }

    #[test]
    fn test_from_hashmap() {
        let map = hashmap! {
            "first".parse().unwrap() => system_path!("/first"),
            "second".parse().unwrap() => system_path!("/second"),
        };
        let pile_paths = PilePaths::from(map.clone());
        assert_eq!(pile_paths, PilePaths::Named(map));
    }

    #[test]
    fn test_lastpaths_get_set_hoard() {
        let hoard_paths = anonymous_hoard_paths();
        let mut last_paths = LastPaths::default();
        let key: HoardName = "testkey".parse().unwrap();
        last_paths.set_hoard(key.clone(), hoard_paths.clone());
        let got_hoard_paths = last_paths.hoard(&key);
        assert_eq!(got_hoard_paths, Some(&hoard_paths));
    }

    #[test]
    fn test_hoard_paths_time_returns_timestamp_reference() {
        let hoard_paths = anonymous_hoard_paths();
        assert_eq!(hoard_paths.time(), &hoard_paths.timestamp);
    }

    #[test]
    fn test_hoard_paths_named_pile() {
        let anonymous = anonymous_hoard_paths();
        assert_eq!(anonymous.named_pile(&NAMED_PILE_1.parse().unwrap()), None);
        let named = named_hoard_paths();
        assert_eq!(named.named_pile(&"no_exist".parse().unwrap()), None);
        assert!(named.named_pile(&NAMED_PILE_1.parse().unwrap()).is_some());
    }

    #[test]
    fn test_hoard_paths_anonymous_pile() {
        let anonymous = anonymous_hoard_paths();
        assert!(anonymous.anonymous_pile().is_some());
        let named = named_hoard_paths();
        assert_eq!(named.anonymous_pile(), None);
    }

    #[test]
    fn test_named_and_anonymous_paths_not_same() {
        let anonymous = anonymous_hoard_paths();
        let named = named_hoard_paths();

        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&anonymous, &named),
                Err(Error::HoardPathsMismatch)
            ),
            "anonymous and named paths are not the same"
        );
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&named, &anonymous),
                Err(Error::HoardPathsMismatch)
            ),
            "swapping parameter order should make no difference"
        );
    }

    #[test]
    fn test_compare_anonymous_paths() {
        let anon_none = HoardPaths::from(PilePaths::Anonymous(None));
        let anon_1 = HoardPaths::from(PilePaths::Anonymous(Some(system_path!("/test/path1"))));
        let anon_2 = HoardPaths::from(PilePaths::Anonymous(Some(system_path!("/test/path2"))));
        // Create dupe of 1 to get different timestamp
        std::thread::sleep(std::time::Duration::from_secs(1));
        let anon_3 = HoardPaths::from(PilePaths::Anonymous(Some(system_path!("/test/path1"))));

        // Test none/none and some/some are the same.
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&anon_none, &anon_none),
                Ok(())
            ),
            "two Anonymous(None) paths are the same"
        );
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&anon_1, &anon_3),
                Ok(())
            ),
            "two Some(_) paths with same path are the same"
        );

        // none/some doesn't match
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&anon_none, &anon_1),
                Err(Error::HoardPathsMismatch),
            ),
            "None/Some(_) are not the same"
        );
        // some/some with different paths are different
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&anon_1, &anon_2),
                Err(Error::HoardPathsMismatch),
            ),
            "Some(path1)/Some(path2) are not the same when different paths"
        );
    }

    #[test]
    fn test_compare_named_paths() {
        let named_empty = HoardPaths::from(PilePaths::Named(hashmap! {}));
        let named_with_1 = HoardPaths::from(PilePaths::Named(hashmap! {
            NAMED_PILE_1.parse().unwrap() => system_path!("/test/path1"),
        }));
        let named_with_2 = HoardPaths::from(PilePaths::Named(hashmap! {
            NAMED_PILE_2.parse().unwrap() => system_path!("/test/path2"),
        }));
        let named_with_both = HoardPaths::from(PilePaths::Named(hashmap! {
            NAMED_PILE_1.parse().unwrap() => system_path!("/test/path1"),
            NAMED_PILE_2.parse().unwrap() => system_path!("/test/path2"),
        }));
        // Create dupe of 1 to get different timestamp
        std::thread::sleep(std::time::Duration::from_secs(1));
        let named_with_1_again = HoardPaths::from(PilePaths::Named(hashmap! {
            NAMED_PILE_1.parse().unwrap() => system_path!("/test/path1"),
        }));

        // Test the same
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&named_empty, &named_empty),
                Ok(()),
            ),
            "empty paths are the same"
        );
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&named_with_1, &named_with_1),
                Ok(()),
            ),
            "single (same) paths are the same"
        );
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&named_with_1, &named_with_1_again),
                Ok(()),
            ),
            "single (same) paths are the same"
        );
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&named_with_both, &named_with_both),
                Ok(()),
            ),
            "same path maps are the same"
        );

        // Test are different
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&named_empty, &named_with_1),
                Err(Error::HoardPathsMismatch)
            ),
            "empty paths and single path are not the same"
        );
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&named_with_1, &named_with_2),
                Err(Error::HoardPathsMismatch)
            ),
            "single different paths are not the same"
        );
        assert!(
            matches!(
                HoardPaths::enforce_old_and_new_piles_are_same(&named_with_1, &named_with_both),
                Err(Error::HoardPathsMismatch)
            ),
            "single path and two paths containing that single are different"
        );
    }

    #[test]
    fn test_serde() {
        let original_anon = anonymous_hoard_paths();
        let parsed_anon: HoardPaths =
            serde_json::from_str(&serde_json::to_string(&original_anon).unwrap()).unwrap();
        assert_eq!(original_anon, parsed_anon);

        let original_named = named_hoard_paths();
        let parsed_named: HoardPaths =
            serde_json::from_str(&serde_json::to_string(&original_named).unwrap()).unwrap();
        assert_eq!(original_named, parsed_named);
    }

    #[test]
    fn pile_paths_from_hoard() {
        use crate::hoard::{Hoard, MultipleEntries, Pile, PileConfig};
        let anon_hoard = Hoard::Anonymous(Pile {
            config: PileConfig::default(),
            path: Some(system_path!("/anon/path")),
        });

        let named_hoard = Hoard::Named(MultipleEntries {
            piles: maplit::hashmap! {
                "first".parse().unwrap() => Pile {
                    config: PileConfig::default(),
                    path: Some(system_path!("/first/path")),
                },
                "missing".parse().unwrap() => Pile {
                    config: PileConfig::default(),
                    path: None,
                },
                "second".parse().unwrap() => Pile {
                    config: PileConfig::default(),
                    path: Some(system_path!("/second/path"))
                }
            },
        });

        let anon_paths = PilePaths::from(anon_hoard);
        let named_paths = PilePaths::from(named_hoard);

        assert_eq!(
            anon_paths,
            PilePaths::Anonymous(Some(system_path!("/anon/path")))
        );
        assert_eq!(
            named_paths,
            PilePaths::Named(maplit::hashmap! {
                "first".parse().unwrap() => system_path!("/first/path"),
                "second".parse().unwrap() => system_path!("/second/path"),
            })
        );
    }
}
