use std::path::{Path, PathBuf};
use crate::hoard::{HoardPath, SystemPath};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct HoardFile {
    pile_name: Option<String>,
    hoard_prefix: HoardPath,
    system_prefix: SystemPath,
    hoard_path: HoardPath,
    system_path: SystemPath,
    relative_path: PathBuf,
}

impl HoardFile {
    pub(crate) fn new(pile_name: Option<String>, hoard_prefix: HoardPath, system_prefix: SystemPath, relative_path: PathBuf) -> Self {
        let (hoard_path, system_path) = if relative_path.to_str() == Some("") {
            (hoard_prefix.clone(), system_prefix.clone())
        } else {
            let hoard_path = HoardPath(hoard_prefix.join(&relative_path));
            let system_path = SystemPath(system_prefix.join(&relative_path));
            (hoard_path, system_path)
        };

        Self {
            pile_name,
            hoard_prefix,
            system_prefix,
            hoard_path,
            system_path,
            relative_path
        }
    }

    pub(crate) fn pile_name(&self) -> Option<&str> {
        self.pile_name.as_deref()
    }

    pub(crate) fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub(crate) fn hoard_prefix(&self) -> &Path {
        &self.hoard_prefix
    }

    pub(crate) fn system_prefix(&self) -> &Path {
        &self.system_prefix
    }

    pub(crate) fn hoard_path(&self) -> &Path {
        &self.hoard_path
    }

    pub(crate) fn system_path(&self) -> &Path {
        &self.system_path
    }

    pub(crate) fn is_file(&self) -> bool {
        let sys = self.system_path();
        let hoard = self.hoard_path();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_file() || !sys_exists)
            && (hoard.is_file() || !hoard_exists)
            && (sys_exists || hoard_exists)
    }

    pub(crate) fn is_dir(&self) -> bool {
        let sys = self.system_path();
        let hoard = self.hoard_path();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_dir() || !sys_exists)
            && (hoard.is_dir() || !hoard_exists)
            && (sys_exists || hoard_exists)
    }
}
