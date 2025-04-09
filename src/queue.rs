use std::env;
use rabbitmq_stream_client::error::StreamCreateError;
use rabbitmq_stream_client::types::{ByteCapacity, OffsetSpecification, ResponseCode};
use rabbitmq_stream_client::{Consumer, Producer,Environment, NoDedup};
use crate::errors::RelayerError;
use redis::{aio::MultiplexedConnection, AsyncCommands, Client};


const STREAM : &str = "relayer-stream-xt";

pub struct QueueConnectionWriter {
    pub environment: Environment,
    pub stream: String,
    pub producer: Producer<NoDedup>,
}

pub struct QueueConnectionConsumer {
    pub environment: Environment,
    pub stream: String,
    pub consumer: Consumer,
    pub dbcon : MultiplexedConnection,
    pub offset : u64
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
                ResponseCode::StreamAlreadyExists => {},
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
            producer
        })
    }
}


impl QueueConnectionConsumer {
    pub async fn new() -> Result<Self, RelayerError> {
        
        let db_url = env::var("DB_URL").expect("DB_URL not set");

        let client = Client::open(db_url).map_err(|e| RelayerError::RedisError(e.to_string()))?;
        let mut dbcon = client.get_multiplexed_async_connection().await.map_err(|e| RelayerError::RedisError(e.to_string()))?;

        let offset: u64 = dbcon
            .get("last_offset")
            .await
            .unwrap_or(0);

        let offset_spec = OffsetSpecification::Offset(offset);

        println!("Starting from offset: {}", offset);

        let environment = Environment::builder()
            .build()
            .await
            .map_err( RelayerError::QueueClientError)?;
        let stream = String::from(STREAM);
        
        let create_response = environment
            .stream_creator()
            .max_length(ByteCapacity::GB(5))
            .create(&stream)
            .await;

        
        if let Err(StreamCreateError::Create { stream: _, status }) = create_response {
            match status {
                ResponseCode::StreamAlreadyExists => {},
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
            .map_err( RelayerError::QueueConsumerCreateError)?;
        
        Ok(Self {
            environment,
            stream,
            consumer,
            dbcon,
            offset
        })
    }
}

pub async fn get_queue_connection_writer() -> Result<QueueConnectionWriter,RelayerError>{
    let queue_connection = QueueConnectionWriter::new().await?;
    Ok(queue_connection)
}

pub async fn get_queue_connection_consumer() -> Result<QueueConnectionConsumer,RelayerError>{
    let queue_connection = QueueConnectionConsumer::new().await?;
    Ok(queue_connection)
}