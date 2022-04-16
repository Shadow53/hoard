use crate::checksum::ChecksumType;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::{fs, io};

/// Configuration for symmetric (password) encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SymmetricEncryption {
    /// Raw password.
    #[serde(rename = "password")]
    Password(String),
    /// Command whose first line of output to stdout is the password.
    #[serde(rename = "password_cmd")]
    PasswordCmd(Vec<String>),
}

/// Configuration for asymmetric (public key) encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsymmetricEncryption {
    #[serde(rename = "public_key")]
    pub(crate) public_key: String,
}

/// Configuration for hoard/pile encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Encryption {
    /// Symmetric encryption.
    Symmetric(SymmetricEncryption),
    /// Asymmetric encryption.
    Asymmetric(AsymmetricEncryption),
}

/// Configurable permissions for files and folders.
///
/// Can be declared as a unix `chmod(1)` style mode or as a set of boolean flags.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged, rename_all = "snake_case")]
pub enum Permissions {
    Mode(u32),
    #[serde(rename = "permissions")]
    Manual {
        is_executable: bool,
        is_readable: bool,
        is_writable: bool,
        others_can_read: bool,
        others_can_write: bool,
        #[serde(alias = "others_can_list")]
        others_can_execute: bool,
    },
}

impl Permissions {
    const OWNER_READ: u32 = 0o400;
    const OWNER_WRITE: u32 = 0o200;
    const OWNER_EXE: u32 = 0o100;
    const OTHER_READ: u32 = 0o044;
    const OTHER_WRITE: u32 = 0o022;
    const OTHER_EXE: u32 = 0o011;
    const EMPTY: u32 = 0;

    /// The default permissions for files.
    ///
    /// Currently, this is owner-only read/write permissions.
    pub fn file_default() -> Self {
        Self::Mode(Self::OWNER_READ | Self::OWNER_WRITE)
    }

    /// The default permissions for directories.
    ///
    /// Currently, this is owner-only read/write/execute permissions
    /// (execute is necessary on unix-y systems to list the contents).
    pub fn folder_default() -> Self {
        Self::Mode(Self::OWNER_READ | Self::OWNER_WRITE | Self::OWNER_EXE)
    }

    pub fn as_mode(self) -> u32 {
        match self {
            Self::Mode(mode) => mode,
            Self::Manual {
                is_executable,
                is_readable,
                is_writable,
                others_can_read,
                others_can_write,
                others_can_execute,
            } => {
                let owner_exe = if is_executable {
                    Self::OWNER_EXE
                } else {
                    Self::EMPTY
                };
                let owner_write = if is_writable {
                    Self::OWNER_WRITE
                } else {
                    Self::EMPTY
                };
                let owner_read = if is_readable {
                    Self::OWNER_READ
                } else {
                    Self::EMPTY
                };

                let other_exe = if others_can_execute {
                    Self::OTHER_EXE
                } else {
                    Self::EMPTY
                };
                let other_write = if others_can_write {
                    Self::OTHER_WRITE
                } else {
                    Self::EMPTY
                };
                let other_read = if others_can_read {
                    Self::OTHER_READ
                } else {
                    Self::EMPTY
                };

                owner_read | owner_write | owner_exe | other_read | other_write | other_exe
            }
        }
    }

    pub fn is_readonly(self) -> bool {
        match self {
            Self::Mode(mode) => (mode & Self::OWNER_WRITE) == 0,
            Self::Manual { is_writable, .. } => !is_writable,
        }
    }

    pub fn set_permissions(self, mut perms: fs::Permissions) -> fs::Permissions {
        #[cfg(unix)]
        perms.set_mode(self.as_mode());
        #[cfg(not(unix))]
        perms.set_readonly(self.is_readonly());
        perms
    }

    pub fn set_on_path(self, path: &Path) -> io::Result<()> {
        let perms = fs::metadata(path)
            .map_err(|err| {
                tracing::error!(
                    "failed to read current permissions for {}: {}",
                    path.display(),
                    err
                );
                err
            })?
            .permissions();
        let perms = self.set_permissions(perms);
        fs::set_permissions(path, perms).map_err(|err| {
            tracing::error!("failed to set permissions on {}: {}", path.display(), err);
            err
        })
    }
}

#[allow(single_use_lifetimes)]
fn deserialize_glob<'de, D>(deserializer: D) -> Result<Vec<glob::Pattern>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<String>::deserialize(deserializer)?
        .iter()
        .map(String::as_str)
        .map(glob::Pattern::new)
        .collect::<Result<_, _>>()
        .map_err(D::Error::custom)
}

#[allow(clippy::ptr_arg)]
fn serialize_glob<S>(value: &Vec<glob::Pattern>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let value = value
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    value.serialize(serializer)
}

/// Hoard/Pile configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The [`ChecksumType`] to use when hashing files.
    #[serde(default, rename = "hash_algorithm")]
    pub checksum_type: Option<ChecksumType>,
    /// The [`Encryption`] configuration for a pile.
    #[serde(default, rename = "encrypt")]
    pub encryption: Option<Encryption>,
    /// A list of glob patterns matching files to ignore.
    #[serde(
        default,
        deserialize_with = "deserialize_glob",
        serialize_with = "serialize_glob"
    )]
    pub ignore: Vec<glob::Pattern>,
    /// The [`Permissions`] to set on restored files.
    ///
    /// See [`Permissions::file_default`] for the default value.
    #[serde(default)]
    pub file_permissions: Option<Permissions>,
    /// The [`Permissions`] to set on restored folders.
    ///
    /// See [`Permissions::folder_default`] for the default value.
    #[serde(default)]
    pub folder_permissions: Option<Permissions>,
}

impl Config {
    /// Merge the `other` configuration with this one, preferring the content of this one, when
    /// appropriate.
    fn layer(&mut self, other: &Self) {
        // Overlay a more general encryption config, if a specific one doesn't exist.
        if self.encryption.is_none() {
            self.encryption = other.encryption.clone();
        }

        self.checksum_type = self.checksum_type.or(other.checksum_type);

        self.file_permissions = self.file_permissions.or(other.file_permissions);
        self.folder_permissions = self.folder_permissions.or(other.folder_permissions);

        // Merge ignore lists.
        self.ignore.extend(other.ignore.clone());
        self.ignore.sort_unstable();
        self.ignore.dedup();
    }

    /// Layer the `general` config with the `specific` one, modifying the `specific` one in place.
    pub fn layer_options(specific: &mut Option<Self>, general: Option<&Self>) {
        if let Some(general) = general {
            match specific {
                None => {
                    specific.replace(general.clone());
                }
                Some(this_config) => this_config.layer(general),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::ChecksumType;
    use crate::hoard::pile_config::Permissions;

    #[test]
    fn test_layer_configs_both_none() {
        let mut specific = None;
        let general = None;
        Config::layer_options(&mut specific, general);
        assert!(specific.is_none());
    }

    #[test]
    fn test_layer_specific_some_general_none() {
        let mut specific = Some(Config {
            checksum_type: Some(ChecksumType::default()),
            encryption: Some(Encryption::Symmetric(SymmetricEncryption::Password(
                "password".into(),
            ))),
            ignore: vec![glob::Pattern::new("ignore me").unwrap()],
            file_permissions: Some(Permissions::Mode(0o666)),
            folder_permissions: Some(Permissions::Mode(0o777)),
        });
        let old_specific = specific.clone();
        let general = None;
        Config::layer_options(&mut specific, general);
        assert_eq!(specific, old_specific);
    }

    #[test]
    fn test_layer_specific_none_general_some() {
        let mut specific = None;
        let general = Some(Config {
            checksum_type: Some(ChecksumType::default()),
            encryption: Some(Encryption::Symmetric(SymmetricEncryption::Password(
                "password".into(),
            ))),
            ignore: vec![glob::Pattern::new("ignore me").unwrap()],
            file_permissions: Some(Permissions::Mode(0o666)),
            folder_permissions: Some(Permissions::Mode(0o777)),
        });
        Config::layer_options(&mut specific, general.as_ref());
        assert_eq!(specific, general);
    }

    #[test]
    fn test_layer_configs_both_some() {
        let mut specific = Some(Config {
            checksum_type: Some(ChecksumType::default()),
            encryption: Some(Encryption::Symmetric(SymmetricEncryption::Password(
                "password".into(),
            ))),
            ignore: vec![
                glob::Pattern::new("ignore me").unwrap(),
                glob::Pattern::new("duplicate").unwrap(),
            ],
            file_permissions: Some(Permissions::Mode(0o644)),
            folder_permissions: Some(Permissions::Mode(0o777)),
        });
        let old_specific = specific.clone();
        let general = Some(Config {
            checksum_type: Some(ChecksumType::default()),
            encryption: Some(Encryption::Asymmetric(AsymmetricEncryption {
                public_key: "somekey".into(),
            })),
            ignore: vec![
                glob::Pattern::new("me too").unwrap(),
                glob::Pattern::new("duplicate").unwrap(),
            ],
            file_permissions: Some(Permissions::Mode(0o666)),
            folder_permissions: Some(Permissions::Mode(0o755)),
        });
        Config::layer_options(&mut specific, general.as_ref());
        assert!(specific.is_some());
        assert_eq!(
            specific.as_ref().unwrap().encryption,
            old_specific.unwrap().encryption
        );
        assert_eq!(
            specific.as_ref().unwrap().ignore,
            vec![
                glob::Pattern::new("duplicate").unwrap(),
                glob::Pattern::new("ignore me").unwrap(),
                glob::Pattern::new("me too").unwrap(),
            ]
        );
        assert_eq!(
            specific
                .as_ref()
                .unwrap()
                .file_permissions
                .unwrap()
                .as_mode(),
            0o644
        );
        assert_eq!(
            specific
                .as_ref()
                .unwrap()
                .folder_permissions
                .unwrap()
                .as_mode(),
            0o777
        );
    }

    mod permissions {
        use super::*;

        #[allow(clippy::too_many_lines)]
        #[test]
        fn test_as_mode() {
            let perms = [
                (
                    Permissions::Mode(0o000),
                    Permissions::Manual {
                        is_executable: false,
                        is_readable: false,
                        is_writable: false,
                        others_can_read: false,
                        others_can_write: false,
                        others_can_execute: false,
                    },
                ),
                (
                    Permissions::Mode(0o011),
                    Permissions::Manual {
                        is_executable: false,
                        is_readable: false,
                        is_writable: false,
                        others_can_read: false,
                        others_can_write: false,
                        others_can_execute: true,
                    },
                ),
                (
                    Permissions::Mode(0o100),
                    Permissions::Manual {
                        is_executable: true,
                        is_readable: false,
                        is_writable: false,
                        others_can_read: false,
                        others_can_write: false,
                        others_can_execute: false,
                    },
                ),
                (
                    Permissions::Mode(0o111),
                    Permissions::Manual {
                        is_executable: true,
                        is_readable: false,
                        is_writable: false,
                        others_can_read: false,
                        others_can_write: false,
                        others_can_execute: true,
                    },
                ),
                (
                    Permissions::Mode(0o022),
                    Permissions::Manual {
                        is_executable: false,
                        is_readable: false,
                        is_writable: false,
                        others_can_read: false,
                        others_can_write: true,
                        others_can_execute: false,
                    },
                ),
                (
                    Permissions::Mode(0o200),
                    Permissions::Manual {
                        is_executable: false,
                        is_readable: false,
                        is_writable: true,
                        others_can_read: false,
                        others_can_write: false,
                        others_can_execute: false,
                    },
                ),
                (
                    Permissions::Mode(0o222),
                    Permissions::Manual {
                        is_executable: false,
                        is_readable: false,
                        is_writable: true,
                        others_can_read: false,
                        others_can_write: true,
                        others_can_execute: false,
                    },
                ),
                (
                    Permissions::Mode(0o044),
                    Permissions::Manual {
                        is_executable: false,
                        is_readable: false,
                        is_writable: false,
                        others_can_read: true,
                        others_can_write: false,
                        others_can_execute: false,
                    },
                ),
                (
                    Permissions::Mode(0o400),
                    Permissions::Manual {
                        is_executable: false,
                        is_readable: true,
                        is_writable: false,
                        others_can_read: false,
                        others_can_write: false,
                        others_can_execute: false,
                    },
                ),
                (
                    Permissions::Mode(0o444),
                    Permissions::Manual {
                        is_executable: false,
                        is_readable: true,
                        is_writable: false,
                        others_can_read: true,
                        others_can_write: false,
                        others_can_execute: false,
                    },
                ),
                (
                    Permissions::Mode(0o555),
                    Permissions::Manual {
                        is_executable: true,
                        is_readable: true,
                        is_writable: false,
                        others_can_read: true,
                        others_can_write: false,
                        others_can_execute: true,
                    },
                ),
                (
                    Permissions::Mode(0o666),
                    Permissions::Manual {
                        is_executable: false,
                        is_readable: true,
                        is_writable: true,
                        others_can_read: true,
                        others_can_write: true,
                        others_can_execute: false,
                    },
                ),
                (
                    Permissions::Mode(0o777),
                    Permissions::Manual {
                        is_executable: true,
                        is_readable: true,
                        is_writable: true,
                        others_can_read: true,
                        others_can_write: true,
                        others_can_execute: true,
                    },
                ),
            ];

            for (mode, manual) in perms {
                if let Permissions::Mode(m) = mode {
                    assert_eq!(mode.as_mode(), m);
                } else {
                    unreachable!();
                }

                assert_eq!(mode.as_mode(), manual.as_mode());
            }
        }
    }
}
