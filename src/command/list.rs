use crate::newtypes::HoardName;

#[allow(single_use_lifetimes)]
#[tracing::instrument(skip_all)]
pub(crate) fn run_list<'a>(hoard_names_iter: impl IntoIterator<Item = &'a HoardName>) {
    let mut hoards: Vec<_> = hoard_names_iter.into_iter().map(AsRef::as_ref).collect();
    hoards.sort_unstable();
    let list = hoards.join("\n");
    tracing::info!("{}", list);
}
