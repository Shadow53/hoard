use thiserror::Error;
use crate::checkers::history::operation::Error as OperationError;
use crate::checkers::history::operation::util::upgrade_operations;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("failed to upgrade operation logs: {0}")]
    Operations(OperationError),
}

pub(crate) fn run_upgrade() -> Result<(), super::Error> {
    let _span = tracing::trace_span!("run_upgrade").entered();
    tracing::info!("Upgrading operation logs to the latest format...");
    match upgrade_operations() {
        Ok(_) => {
            tracing::info!("Successfully upgraded all operation logs");
            Ok(())
        },
        Err(err) => {
            tracing::error!("Failed to upgrade operation logs: {}", err);
            Err(super::Error::Upgrade(Error::Operations(err)))
        },
    }
}