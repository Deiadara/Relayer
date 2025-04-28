use crate::errors::RelayerError;
use crate::subscriber::{Deposit, RedisClient};
use async_trait::async_trait;
use futures::StreamExt;
use lapin::message::Delivery;
use mockall::automock;
use mockall::predicate::eq;
use redis::Client;
use std::{env, env::set_var};

//use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties,
    Channel,
    Connection,
    ConnectionProperties, //Result,
    Consumer,
    Queue,
    options::*,
    publisher_confirm::Confirmation,
    types::FieldTable,
};
use tracing::info;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait QueueTrait {
    async fn publish(&mut self, dep: Deposit) -> Result<(), RelayerError>;
    async fn consumer(&mut self) -> Result<lapin::Consumer, RelayerError>;
}
#[derive(Clone)]

pub struct QueueConnection {
    channel: Channel,
    //queue: Queue,
}

#[async_trait]
impl QueueTrait for QueueConnection {
    async fn publish(&mut self, dep: Deposit) -> Result<(), RelayerError> {
        println!("Event emitted from sender: {:?}", dep.sender);

        let serialized_deposit = serde_json::to_vec(&dep).map_err(RelayerError::SerdeError)?;

        let _confirm = self
            .channel
            .basic_publish(
                "",
                "relayer",
                BasicPublishOptions::default(),
                &serialized_deposit,
                BasicProperties::default(),
            )
            .await?
            .await?;

        println!("Wrote in queue successfully!");
        Ok(())
    }

    async fn consumer(&mut self) -> Result<Consumer, RelayerError> {
        println!("Waiting for a deposit message...");
        let consumer = self
            .channel
            .basic_consume(
                "relayer",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        Ok(consumer)
    }
}

impl QueueConnection {
    pub async fn new() -> Result<Self, RelayerError> {
        //tracing_subscriber::fmt::init();

        let addr =
            std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

        let conn = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .map_err(|e| RelayerError::Other(e.to_string()))?;

        println!("CONNECTED");

        let channel = conn
            .create_channel()
            .await
            .map_err(|e| RelayerError::Other(e.to_string()))?;

        let queue = channel
            .queue_declare(
                "relayer",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| RelayerError::Other(e.to_string()))?;

        Ok(QueueConnection { channel/* ,queue*/  })
    }
}

pub async fn get_queue_connection() -> Result<QueueConnection, RelayerError> {
    let queue_connection = QueueConnection::new().await?;
    Ok(queue_connection)
}

pub async fn consume(consumer: &mut Consumer) -> Result<Deposit, RelayerError> {
    println!("Waiting for a deposit message…");

    loop {
        match consumer.next().await {
            None => {
                return Err(RelayerError::Other(
                    "Consumer stream ended unexpectedly".into(),
                ))
            }
            Some(Err(e)) => {
                eprintln!("Delivery error: {:?}", e);
                continue;
            }
            Some(Ok(delivery)) => {
                match serde_json::from_slice::<Deposit>(&delivery.data) {
                    Ok(deposit) => {
                        delivery
                            .ack(BasicAckOptions::default())
                            .await
                            .map_err(RelayerError::AmqpError)?;
                        println!("Got deposit from {:?}, amount {}", deposit.sender, deposit.amount);
                        return Ok(deposit);
                    }
                    Err(_) => {
                        eprintln!("Failed to parse Deposit, skipping");
                        continue;
                    }
                }
            }
        }
    }
}

// mod tests {
//     use super::*;
//     #[tokio::test]
//     async fn test_publish() {
//         let mut mock_queue_connection = MockQueue::new();
//         let deposit = Deposit {
//             sender: "0x1234567890123456789012345678901234567890"
//                 .parse()
//                 .unwrap(),
//             amount: 100,
//         };
//         mock_queue_connection
//             .expect_publish()
//             .with(eq(deposit.clone()))
//             .once()
//             .returning(|_| Ok(()));
//         let result = mock_queue_connection.publish(deposit).await;
//         assert!(result.is_ok());
//     }

//     #[tokio::test]
//     async fn test_consume() {
//         let mut mock_queue_connection = MockQueue::new();
//         mock_queue_connection.expect_consume().once().returning(|| {
//             Ok(Deposit {
//                 sender: "0x1234567890123456789012345678901234567890"
//                     .parse()
//                     .unwrap(),
//                 amount: 100,
//             })
//         });
//         let result = mock_queue_connection.consume().await;
//         assert!(result.is_ok());
//         assert_eq!(
//             result.unwrap(),
//             Deposit {
//                 sender: "0x1234567890123456789012345678901234567890"
//                     .parse()
//                     .unwrap(),
//                 amount: 100
//             }
//         );
//     }
// }
