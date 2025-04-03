use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayerError {
    #[error("Event hash does not match signature.")]
    EventHashMismatch,

    #[error("No logs found in transaction receipt.")]
    NoLogs,

    #[error("No topics found.")]
    NoTopics,

    #[error("No address found in topics.")]
    NoAddress,

    #[error("Data is not string")]
    NotString,

    #[error(transparent)]
    AbiError(#[from] alloy::dyn_abi::Error),

    #[error("Provider call failed: {0}")]
    ProviderError(String),

    #[error("Radis call failed: {0}")]
    RadisError(String),
}

