use thiserror::Error;
use rabbitmq_stream_client::error::{ProducerCreateError,ProducerPublishError, ClientError, ConsumerCloseError};


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

    #[error("Redis call failed: {0}")]
    RedisError(String),

    #[error("Failed to create message: {0}")]
    QueueProducerCreateError(#[from] ProducerCreateError),

    #[error("Failed to publish message: {0}")]
    QueueProducerPublishError(#[from] ProducerPublishError),

    #[error("Client creation failed: {0:?}")]
    QueueClientError(#[from] ClientError),

    #[error("Consumer close failed: {0}")]
    QueueConsumerCloseError(#[from] ConsumerCloseError), 

    #[error("Unhandled error: {0}")]
    Other(String)
}
