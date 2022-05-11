use thiserror::Error;

use crate::checkers::history::operation::util::upgrade_operations;
use crate::checkers::history::operation::Error as OperationError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to upgrade operation logs: {0}")]
    Operations(OperationError),
}

#[tracing::instrument]
pub(crate) async fn run_upgrade() -> Result<(), super::Error> {
    tracing::info!("Upgrading operation logs to the latest format...");
    upgrade_operations()
        .await
        .map_err(Error::Operations)
        .map_err(super::Error::Upgrade)?;
    tracing::info!("Successfully upgraded all operation logs");
    Ok(())
}
