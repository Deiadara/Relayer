use crate::errors::RelayerError;
use crate::subscriber::{Deposit, RedisClient};
use async_trait::async_trait;
use futures::StreamExt;
use mockall::automock;
use mockall::predicate::eq;
use rabbitmq_stream_client::error::StreamCreateError;
use rabbitmq_stream_client::types::Message;
use rabbitmq_stream_client::types::{ByteCapacity, OffsetSpecification, ResponseCode};
use rabbitmq_stream_client::{Consumer, Environment, NoDedup, Producer};
use redis::Client;
use std::env;

const STREAM: &str = "relayer-stream-105";

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Queue {
    async fn push(&mut self, dep: Deposit) -> Result<(), RelayerError>;
    async fn consume(&mut self) -> Result<Deposit, RelayerError>;
}

pub struct QueueConnectionWriter {
    pub environment: Environment,
    pub stream: String,
    pub producer: Producer<NoDedup>,
}

pub struct QueueConnectionConsumer {
    pub environment: Environment,
    pub stream: String,
    pub consumer: Consumer,
    pub redis: Box<dyn RedisClient>,
    pub offset: u64,
}

#[async_trait]
impl Queue for QueueConnectionWriter {
    async fn push(&mut self, dep: Deposit) -> Result<(), RelayerError> {
        println!("Event emitted from sender: {:?}", dep.sender);

        let serialized_deposit = serde_json::to_vec(&dep).map_err(RelayerError::SerdeError)?;

        self.producer
            .send_with_confirm(Message::builder().body(serialized_deposit).build())
            .await
            .map_err(RelayerError::QueueProducerPublishError)?;

        println!("Wrote in queue successfully!");
        Ok(())
    }

    async fn consume(&mut self) -> Result<Deposit, RelayerError> {
        Err(RelayerError::Other(
            "Writer cannot consume messages".to_string(),
        ))
    }
}

#[async_trait]
impl Queue for QueueConnectionConsumer {
    async fn push(&mut self, _dep: Deposit) -> Result<(), RelayerError> {
        Err(RelayerError::Other(
            "Consumer cannot push messages".to_string(),
        ))
    }

    async fn consume(&mut self) -> Result<Deposit, RelayerError> {
        println!("Waiting for a deposit message...");

        while let Some(delivery_result) = self.consumer.next().await {
            match delivery_result {
                Ok(delivery) => {
                    if let Some(data_bytes) = delivery.message().data() {
                        match serde_json::from_slice::<Deposit>(data_bytes) {
                            Ok(deposit) => {
                                println!(
                                    "Got deposit: {:?} at offset {}",
                                    deposit,
                                    delivery.offset()
                                );
                                let _: () = self
                                    .redis
                                    .set_last_offset("last_offset", delivery.offset() + 1)
                                    .await
                                    .map_err(|e| RelayerError::RedisError(e.to_string()))?;
                                return Ok(deposit);
                            }
                            Err(_) => continue,
                        }
                    } else {
                        eprintln!("No data in message");
                    }
                }
                Err(e) => {
                    eprintln!("Delivery error: {:?}", e);
                }
            }
        }

        Err(RelayerError::Other(
            "Consumer stream ended unexpectedly".to_string(),
        ))
    }
}

impl QueueConnectionWriter {
    pub async fn new() -> Result<Self, RelayerError> {
        let environment = Environment::builder()
            .build()
            .await
            .map_err(RelayerError::QueueClientError)?;
        let stream = String::from(STREAM);

        let create_response = environment
            .stream_creator()
            .max_length(ByteCapacity::GB(5))
            .create(&stream)
            .await;

        if let Err(StreamCreateError::Create { stream: _, status }) = create_response {
            match status {
                ResponseCode::StreamAlreadyExists => {}
                err => {
                    println!("Error creating stream: {:?} {:?}", stream, err);
                }
            }
        }

        let producer: Producer<NoDedup> = environment
            .producer()
            .build(&stream)
            .await
            .map_err(RelayerError::QueueProducerCreateError)?;

        Ok(Self {
            environment,
            stream,
            producer,
        })
    }
}

impl QueueConnectionConsumer {
    pub async fn new() -> Result<Self, RelayerError> {
        let db_url = env::var("DB_URL").expect("DB_URL not set");

        let client = Client::open(db_url).map_err(|e| RelayerError::RedisError(e.to_string()))?;
        let dbcon = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RelayerError::RedisError(e.to_string()))?;
        let mut redis: Box<dyn RedisClient> = Box::new(dbcon);

        let offset: u64 = redis.get_last_offset("last_offset").await.unwrap_or(0);

        let offset_spec = OffsetSpecification::Offset(offset);

        println!("Starting from offset: {}", offset);

        let environment = Environment::builder()
            .build()
            .await
            .map_err(RelayerError::QueueClientError)?;
        let stream = String::from(STREAM);

        let create_response = environment
            .stream_creator()
            .max_length(ByteCapacity::GB(5))
            .create(&stream)
            .await;

        if let Err(StreamCreateError::Create { stream: _, status }) = create_response {
            match status {
                ResponseCode::StreamAlreadyExists => {}
                err => {
                    println!("Error creating stream: {:?} {:?}", stream, err);
                }
            }
        }

        let consumer: Consumer = environment
            .consumer()
            .offset(offset_spec)
            .build(&stream)
            .await
            .map_err(RelayerError::QueueConsumerCreateError)?;

        Ok(Self {
            environment,
            stream,
            consumer,
            redis,
            offset,
        })
    }
    pub async fn new_with_redis(redis: Box<dyn RedisClient>) -> Result<Self, RelayerError> {
        let mut redis = redis; // gotta be some better way?
        let offset: u64 = redis.get_last_offset("last_offset").await.unwrap_or(0);

        let offset_spec = OffsetSpecification::Offset(offset);

        println!("Starting from offset: {}", offset);

        let environment = Environment::builder()
            .build()
            .await
            .map_err(RelayerError::QueueClientError)?;
        let stream = String::from(STREAM);

        let create_response = environment
            .stream_creator()
            .max_length(ByteCapacity::GB(5))
            .create(&stream)
            .await;

        if let Err(StreamCreateError::Create { stream: _, status }) = create_response {
            match status {
                ResponseCode::StreamAlreadyExists => {}
                err => {
                    println!("Error creating stream: {:?} {:?}", stream, err);
                }
            }
        }

        let consumer: Consumer = environment
            .consumer()
            .offset(offset_spec)
            .build(&stream)
            .await
            .map_err(RelayerError::QueueConsumerCreateError)?;

        Ok(Self {
            environment,
            stream,
            consumer,
            redis,
            offset,
        })
    }
}

pub async fn get_queue_connection_writer() -> Result<QueueConnectionWriter, RelayerError> {
    let queue_connection = QueueConnectionWriter::new().await?;
    Ok(queue_connection)
}

pub async fn get_queue_connection_consumer() -> Result<QueueConnectionConsumer, RelayerError> {
    let queue_connection = QueueConnectionConsumer::new().await?;
    Ok(queue_connection)
}

pub async fn get_queue_connection_consumer_with_redis(
    redis: Box<dyn RedisClient>,
) -> Result<QueueConnectionConsumer, RelayerError> {
    let queue_connection = QueueConnectionConsumer::new_with_redis(redis).await?;
    Ok(queue_connection)
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_queue_connection_writer() {
        let queue_connection: QueueConnectionWriter = get_queue_connection_writer().await.unwrap();
        assert_eq!(queue_connection.stream, "relayer-stream-105");
    }

    #[tokio::test]
    async fn test_push() {
        let mut mock_queue_connection = MockQueue::new();
        let deposit = Deposit {
            sender: "0x1234567890123456789012345678901234567890"
                .parse()
                .unwrap(),
            amount: 100,
        };
        mock_queue_connection
            .expect_push()
            .with(eq(deposit.clone()))
            .once()
            .returning(|_| Ok(()));
        let result = mock_queue_connection.push(deposit).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_consume() {
        let mut mock_queue_connection = MockQueue::new();
        mock_queue_connection.expect_consume().once().returning(|| {
            Ok(Deposit {
                sender: "0x1234567890123456789012345678901234567890"
                    .parse()
                    .unwrap(),
                amount: 100,
            })
        });
        let result = mock_queue_connection.consume().await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Deposit {
                sender: "0x1234567890123456789012345678901234567890"
                    .parse()
                    .unwrap(),
                amount: 100
            }
        );
    }
}
