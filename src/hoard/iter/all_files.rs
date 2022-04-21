use crate::checkers::history::operation::{ItemOperation, Operation, OperationImpl};
use crate::filters::{Filter, Filters};
use crate::hoard::Hoard;
use crate::hoard_item::HoardItem;
use crate::newtypes::{HoardName, PileName};
use crate::paths::{HoardPath, RelativePath, SystemPath};
use futures::stream::Peekable;
use futures::{StreamExt, TryStream};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::pin::Pin;
use tokio::{fs, io};
use tokio_stream::wrappers::ReadDirStream;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RootPathItem {
    hoard_file: HoardItem,
    filters: Filters,
}

impl RootPathItem {
    fn keep(&self) -> bool {
        (!self.exists() || self.is_file() || self.is_dir())
            && self.filters.keep(
                self.hoard_file.system_prefix(),
                self.hoard_file.relative_path(),
            )
    }

    fn is_file(&self) -> bool {
        self.hoard_file.is_file()
    }

    fn is_dir(&self) -> bool {
        self.hoard_file.is_dir()
    }

    fn exists(&self) -> bool {
        self.hoard_file.is_file() || self.hoard_file.is_dir()
    }
}

pub(crate) struct AllFilesIter {
    seen_paths: BTreeSet<SystemPath>,
    root_paths: Vec<RootPathItem>,
    system_entries: Option<Peekable<ReadDirStream>>,
    hoard_entries: Option<Peekable<ReadDirStream>>,
    current_root: Option<RootPathItem>,
}

impl AllFilesIter {
    #[tracing::instrument]
    fn paths_from_hoard(
        hoard: &Hoard,
        hoard_name_root: &HoardPath,
    ) -> Result<Vec<RootPathItem>, super::Error> {
        match hoard {
            Hoard::Anonymous(pile) => {
                let path = pile.path.clone();
                let filters = Filters::new(&pile.config)?;
                match path {
                    None => Ok(Vec::default()),
                    Some(system_prefix) => std::iter::once(Ok(RootPathItem {
                        hoard_file: HoardItem::new(
                            PileName::anonymous(),
                            hoard_name_root.clone(),
                            system_prefix,
                            RelativePath::none(),
                        ),
                        filters,
                    }))
                    .collect(),
                }
            }
            Hoard::Named(piles) => piles
                .piles
                .iter()
                .filter_map(|(name, pile)| {
                    let filters = match Filters::new(&pile.config) {
                        Ok(filters) => filters,
                        Err(err) => return Some(Err(super::Error::Filter(err))),
                    };
                    let name_path = RelativePath::from(name);
                    pile.path.as_ref().map(|path| {
                        let hoard_prefix = hoard_name_root.join(&name_path);
                        let system_prefix = path.clone();

                        Ok(RootPathItem {
                            hoard_file: HoardItem::new(
                                name.clone().into(),
                                hoard_prefix,
                                system_prefix,
                                RelativePath::none(),
                            ),
                            filters,
                        })
                    })
                })
                .collect(),
        }
    }

    #[tracing::instrument]
    async fn paths_from_logs(
        hoard: &Hoard,
        hoard_name: &HoardName,
        hoard_name_root: &HoardPath,
    ) -> Result<Vec<RootPathItem>, super::Error> {
        // This is used to detect files deleted locally and remotely
        let from_logs: Vec<ItemOperation<HoardItem>> = {
            let _span = tracing::trace_span!("load_paths_from_logs").entered();
            let local = Operation::latest_local(hoard_name, None)
                .await
                .map_err(Box::new)?;
            let remote = Operation::latest_remote_backup(hoard_name, None, false)
                .await
                .map_err(Box::new)?;

            match (local, remote) {
                (None, None) => Vec::new(),
                (None, Some(single)) => single
                    .hoard_operations_iter(hoard_name_root, hoard)
                    .map_err(Box::new)
                    .map_err(super::Error::Operation)?
                    .collect(),
                (Some(single), None) => single
                    .hoard_operations_iter(hoard_name_root, hoard)
                    .map_err(Box::new)
                    .map_err(super::Error::Operation)?
                    .filter(|item| !matches!(item, ItemOperation::Delete(_)))
                    .collect(),
                (Some(left), Some(right)) => {
                    let left = left
                        .hoard_operations_iter(hoard_name_root, hoard)
                        .map(Box::new)
                        .map_err(Box::new)
                        .map_err(super::Error::Operation)?
                        .filter(|item| !matches!(item, ItemOperation::Delete(_)));
                    let right = right
                        .hoard_operations_iter(hoard_name_root, hoard)
                        .map(Box::new)
                        .map_err(Box::new)
                        .map_err(super::Error::Operation)?;
                    left.chain(right).collect()
                }
            }
        };

        let list = from_logs
            .into_iter()
            .map(|item| RootPathItem {
                hoard_file: item.into_inner(),
                filters: Filters::default(),
            })
            .collect::<Vec<_>>();

        Ok(list)
    }

    #[tracing::instrument(name = "new_all_files_iter")]
    pub(crate) async fn new(
        hoards_root: &HoardPath,
        hoard_name: &HoardName,
        hoard: &Hoard,
    ) -> Result<Self, super::Error> {
        let hoard_name_root = hoards_root.join(&RelativePath::from(hoard_name));
        let mut root_paths = Self::paths_from_hoard(hoard, &hoard_name_root)?;
        let from_logs = Self::paths_from_logs(hoard, hoard_name, &hoard_name_root).await?;

        root_paths.extend(from_logs);
        root_paths.sort_unstable();
        root_paths.dedup();
        tracing::trace!(?root_paths);

        Ok(Self {
            seen_paths: BTreeSet::new(),
            root_paths,
            system_entries: None,
            hoard_entries: None,
            current_root: None,
        })
    }
}

impl AllFilesIter {
    async fn has_dir_entries(&mut self) -> bool {
        if let Some(system_entries) = self.system_entries.as_mut() {
            if Pin::new(system_entries).peek().await.is_some() {
                return true;
            }
        }

        if let Some(hoard_entries) = self.hoard_entries.as_mut() {
            if Pin::new(hoard_entries).peek().await.is_some() {
                return true;
            }
        }

        false
    }

    fn has_seen_path(&mut self, path: &SystemPath) -> bool {
        if self.seen_paths.contains(path) {
            true
        } else {
            self.seen_paths.insert(path.clone());
            false
        }
    }

    async fn get_next_entry_with_prefix(&mut self) -> Option<(io::Result<fs::DirEntry>, PathBuf)> {
        if let Some(stream) = self.system_entries.as_mut() {
            let prefix = self
                .current_root
                .as_ref()
                .unwrap()
                .hoard_file
                .system_prefix()
                .to_path_buf();
            if let Some(entry) = stream.next().await {
                return Some((entry, prefix));
            }
        }

        if let Some(stream) = self.hoard_entries.as_mut() {
            let prefix = self
                .current_root
                .as_ref()
                .unwrap()
                .hoard_file
                .hoard_prefix()
                .to_path_buf();
            if let Some(entry) = stream.next().await {
                return Some((entry, prefix));
            }
        }

        None
    }

    async fn get_next_relative_path(&mut self) -> io::Result<Option<RelativePath>> {
        match self.get_next_entry_with_prefix().await {
            None => Ok(None),
            Some((Ok(entry), prefix)) => {
                let rel_path = RelativePath::try_from(
                    entry
                        .path()
                        .strip_prefix(prefix)
                        .expect("prefix should always match path")
                        .to_path_buf(),
                )
                .expect("path created with strip_prefix should always be valid RelativePath");
                Ok(Some(rel_path))
            }
            Some((Err(error), prefix)) => {
                let rel_path = self
                    .current_root
                    .as_ref()
                    .unwrap()
                    .hoard_file
                    .relative_path()
                    .to_path_buf();
                tracing::error!(
                    "could not process entry in {}/{}: {}",
                    prefix.display(),
                    rel_path.display(),
                    error
                );
                Err(error)
            }
        }
    }

    async fn process_dir_entry(&mut self) -> Result<Option<HoardItem>, io::Error> {
        let current_root = self
            .current_root
            .as_ref()
            .expect("current_root should not be None");

        let pile_name = current_root.hoard_file.pile_name().clone();
        let system_prefix = current_root.hoard_file.system_prefix().clone();
        let hoard_prefix = current_root.hoard_file.hoard_prefix().clone();
        let filters = current_root.filters.clone();

        loop {
            match self.get_next_relative_path().await? {
                None => return Ok(None),
                Some(relative_path) => {
                    let hoard_item = HoardItem::new(
                        pile_name.clone(),
                        hoard_prefix.clone(),
                        system_prefix.clone(),
                        relative_path,
                    );

                    if hoard_item.is_file() && self.has_seen_path(hoard_item.system_path()) {
                        tracing::trace!(item=?hoard_item, "ignoring");
                        continue;
                    }

                    let new_item = RootPathItem {
                        hoard_file: hoard_item,
                        filters: filters.clone(),
                    };

                    if new_item.keep() {
                        if new_item.is_dir() {
                            self.root_paths.push(new_item);
                        } else {
                            tracing::trace!(item=?new_item, "returning");
                            return Ok(Some(new_item.hoard_file));
                        }
                    } else {
                        tracing::trace!(item=?new_item, "discarding");
                    }
                }
            }
        }
    }

    #[allow(clippy::option_option)]
    async fn ensure_dir_entries(&mut self) -> Option<Option<io::Result<HoardItem>>> {
        // Attempt to create direntry iterator.
        // If a path to a file is encountered, return that.
        // Otherwise, continue until existing directory is found.
        while !self.has_dir_entries().await {
            match self.root_paths.pop() {
                None => return Some(None),
                Some(item) => {
                    if item.keep() {
                        if item.is_dir() {
                            let hoard_path = item.hoard_file.hoard_path();
                            let system_path = item.hoard_file.system_path();
                            match fs::read_dir(system_path).await {
                                Ok(iter) => {
                                    self.system_entries = Some(ReadDirStream::new(iter).peekable());
                                }
                                Err(err) => {
                                    if err.kind() == io::ErrorKind::NotFound {
                                        self.system_entries = None;
                                    } else {
                                        tracing::error!(
                                            "failed to read directory {}: {}",
                                            system_path.display(),
                                            err
                                        );
                                        return Some(Some(Err(err)));
                                    }
                                }
                            }
                            match fs::read_dir(hoard_path).await {
                                Ok(iter) => {
                                    self.hoard_entries = Some(ReadDirStream::new(iter).peekable());
                                }
                                Err(err) => {
                                    if err.kind() == io::ErrorKind::NotFound {
                                        self.hoard_entries = None;
                                    } else {
                                        tracing::error!(
                                            "failed to read directory {}: {}",
                                            hoard_path.display(),
                                            err
                                        );
                                        return Some(Some(Err(err)));
                                    }
                                }
                            }
                            self.current_root = Some(item);
                        } else {
                            return Some(Some(Ok(item.hoard_file)));
                        }
                    }
                }
            }
        }

        None
    }

    async fn next_item(&mut self) -> Option<io::Result<HoardItem>> {
        loop {
            if let Some(return_value) = self.ensure_dir_entries().await {
                match return_value.as_ref() {
                    None => tracing::trace!("no more items remaining"),
                    Some(Ok(item)) => tracing::trace!(?item, "found file among root paths"),
                    Some(Err(_)) => {}
                }
                return return_value;
            }

            let result = self.process_dir_entry().await.transpose();

            if let Some(item) = result {
                tracing::trace!(?item, "found file as child of root path");
                return Some(item);
            }
        }
    }
}

/// A [`Stream`] of all managed files under the given [`Hoard`].
///
/// # Errors
///
/// Any errors that may occur while building the stream. See [`Error`](super::Error) for more.
#[allow(clippy::module_name_repetitions)]
pub async fn all_files_stream(
    hoards_root: &HoardPath,
    hoard_name: &HoardName,
    hoard: &Hoard,
) -> Result<impl TryStream<Ok = HoardItem, Error = super::Error>, super::Error> {
    let mut all_files = AllFilesIter::new(hoards_root, hoard_name, hoard).await?;
    let stream = async_stream::try_stream! {
        while let Some(item) = all_files.next_item().await {
            yield item?;
        }
    };

    Ok(stream)
}
