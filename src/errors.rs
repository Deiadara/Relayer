use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayerError {
    #[error("Event hash does not match signature.")]
    EventHashMismatch,

    #[error("No logs found in transaction receipt.")]
    NoLogs,

    #[error("No topics found.")]
    NoTopics,

    #[error("No receipt found")]
    NoReceipt
}

