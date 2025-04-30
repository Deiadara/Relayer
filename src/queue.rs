use crate::errors::RelayerError;
use async_trait::async_trait;
use mockall::automock;
use mockall::predicate::eq;
use tracing::debug;

use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties, Consumer, options::*,
    types::FieldTable,
};

#[async_trait]
pub trait QueueTrait {
    type Consumer;
    async fn publish(&mut self, dep: &Vec<u8>) -> Result<(), RelayerError>;
    async fn consumer(&mut self) -> Result<lapin::Consumer, RelayerError>;
}
#[derive(Clone)]

pub struct LapinConnection {
    channel: Channel,
}

#[async_trait]
impl QueueTrait for LapinConnection {
    type Consumer = lapin::Consumer;

    // test : call new for a connection, call  publish with test_item, consume item and check that it is the same item
    async fn publish(&mut self, serialized_item: &Vec<u8>) -> Result<(), RelayerError> {
        let confirm = self
            .channel
            .basic_publish(
                "",
                "relayer",
                BasicPublishOptions::default(),
                serialized_item,
                BasicProperties::default(),
            )
            .await?
            .await?;

        if confirm.is_ack() {
            return Ok(());
        } else {
            return Err(RelayerError::Other(String::from(
                "Failed to publish to Queue",
            )));
        }
    }
    // new connection in queue, put something in and try to consume it using this function and check that it is returned
    async fn consumer(&mut self) -> Result<Consumer, RelayerError> {
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

impl LapinConnection {
    pub async fn new() -> Result<Self, RelayerError> {
        let addr =
            std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

        let conn = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .map_err(|e| RelayerError::Other(e.to_string()))?;

        debug!("CONNECTED");

        let channel = conn
            .create_channel()
            .await
            .map_err(|e| RelayerError::Other(e.to_string()))?;

        channel
            .confirm_select(ConfirmSelectOptions { nowait: false })
            .await?;

        let _queue = channel
            .queue_declare(
                "relayer",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| RelayerError::Other(e.to_string()))?;

        Ok(LapinConnection {
            channel, /* ,queue*/
        })
    }
}

pub async fn get_queue_connection() -> Result<LapinConnection, RelayerError> {
    let queue_connection = LapinConnection::new().await?;
    Ok(queue_connection)
}
// move to includer

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
