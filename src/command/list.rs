#[allow(single_use_lifetimes)]
pub(crate) fn run_list<'a>(hoard_names_iter: impl IntoIterator<Item = &'a str>) {
    let mut hoards: Vec<_> = hoard_names_iter.into_iter().collect();
    hoards.sort_unstable();
    let list = hoards.join("\n");
    tracing::info!("{}", list);
}
