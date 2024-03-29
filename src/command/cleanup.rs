use crate::checkers::history::operation::cleanup_operations;

#[tracing::instrument]
pub(crate) async fn run_cleanup() -> Result<(), super::Error> {
    match cleanup_operations().await {
        Ok(count) => {
            tracing::info!("cleaned up {} log files", count);
            Ok(())
        }
        Err((count, error)) => Err(super::Error::Cleanup {
            success_count: count,
            error,
        }),
    }
}
