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

    #[error("Failed to create message: {0}")]
    FsStdIOError(#[from] std::io::Error),

    #[error("Data is not string")]
    NotString,

    #[error(transparent)]
    AbiError(#[from] alloy::dyn_abi::Error),

    #[error(transparent)]
    FromHexError(#[from] alloy::hex::FromHexError),

    #[error("Provider call failed: {0}")]
    ProviderError(String),

    #[error("Redis call failed: {0}")]
    RedisError(String),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("AMQP error: {0}")]
    AmqpError(#[from] lapin::Error),

    #[error("Unhandled error: {0}")]
    Other(String),
}
