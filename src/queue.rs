use std::env;
use futures::StreamExt;
use rabbitmq_stream_client::error::StreamCreateError;
use rabbitmq_stream_client::types::{ByteCapacity, OffsetSpecification, ResponseCode};
use rabbitmq_stream_client::{Consumer, Producer,Environment, NoDedup};
use crate::errors::RelayerError;
use redis::{aio::MultiplexedConnection, AsyncCommands, Client};
use crate::subscriber::Deposit;
use rabbitmq_stream_client::types::Message;
use async_trait::async_trait;

const STREAM : &str = "relayer-stream-102";

#[async_trait]

pub trait Queue {
    async fn push(&mut self,dep: Deposit) -> Result<(), RelayerError>;
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
    pub dbcon : MultiplexedConnection,
    pub offset : u64
}

#[async_trait]
impl Queue for QueueConnectionWriter {
    async fn push(&mut self, dep: Deposit) -> Result<(), RelayerError> {
        println!("Event emitted from sender: {:?}", dep.sender);

        let serialized_deposit = serde_json::to_vec(&dep)
            .map_err(RelayerError::SerdeError)?;
        
        self.producer
            .send_with_confirm(Message::builder().body(serialized_deposit).build())
            .await
            .map_err(RelayerError::QueueProducerPublishError)?;
        
        println!("Wrote in queue successfully!");
        Ok(())
    }

    async fn consume(&mut self) -> Result<Deposit, RelayerError> {
        Err(RelayerError::Other("Writer cannot consume messages".to_string()))
    }
}

#[async_trait]
impl Queue for QueueConnectionConsumer {
    async fn push(&mut self, _dep: Deposit) -> Result<(), RelayerError> {
        Err(RelayerError::Other("Consumer cannot push messages".to_string()))
    }

    async fn consume(&mut self) -> Result<Deposit, RelayerError> {
        println!("Waiting for a deposit message...");

        while let Some(delivery_result) = self.consumer.next().await {
            match delivery_result {
                Ok(delivery) => {
                    if let Some(data_bytes) = delivery.message().data() {
                        match serde_json::from_slice::<Deposit>(data_bytes) {
                            Ok(deposit) => {
                                println!("Got deposit: {:?} at offset {}", deposit, delivery.offset());
                                let _ : () = self.dbcon
                                    .set("last_offset", delivery.offset() + 1)
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

        Err(RelayerError::Other("Consumer stream ended unexpectedly".to_string()))
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
    
    // pub async fn push(&mut self,dep: Deposit) -> Result<(), RelayerError> {
    //     println!("Event emitted from sender: {:?}", dep.sender);
    
    //     let serialized_deposit = serde_json::to_vec(&dep)
    //         .map_err( RelayerError::SerdeError)?;
        
    //     self.producer
    //         .send_with_confirm(Message::builder().body(serialized_deposit).build())
    //         .await
    //         .map_err( RelayerError::QueueProducerPublishError)?;
        
    //     println!("Wrote in queue successfully!");
    //     Ok(())
    // }
    
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


    // pub async fn consume(&mut self) -> Result<Deposit, RelayerError> {
    //     println!("Waiting for a deposit message...");

    //     while let Some(delivery_result) = self.consumer.next().await {
    //         match delivery_result {
    //             Ok(delivery) => {
    //                 if let Some(data_bytes) = delivery.message().data() {
    //                     match serde_json::from_slice::<Deposit>(data_bytes) {
    //                         Ok(deposit) => {
    //                             println!("Got deposit: {:?} at offset {}", deposit, delivery.offset());
    //                             let _response: String = self.dbcon
    //                                 .set("last_offset", delivery.offset() + 1)
    //                                 .await
    //                                 .map_err(|e| RelayerError::RedisError(e.to_string()))?;
    //                             return Ok(deposit);
    //                         }
    //                         Err(_e) => {
    //                             continue;
    //                         }
    //                     }
    //                 } else {
    //                     eprintln!("No data in message");
    //                 }
    //             }
    //             Err(e) => {
    //                 eprintln!("Delivery error: {:?}", e);
    //             }
    //         }    
    //     }
    //     Err(RelayerError::Other("Consumer stream ended unexpectedly".into()))
    // }
}

pub async fn get_queue_connection_writer() -> Result<QueueConnectionWriter,RelayerError>{
    let queue_connection = QueueConnectionWriter::new().await?;
    Ok(queue_connection)
}

pub async fn get_queue_connection_consumer() -> Result<QueueConnectionConsumer,RelayerError>{
    let queue_connection = QueueConnectionConsumer::new().await?;
    Ok(queue_connection)
}