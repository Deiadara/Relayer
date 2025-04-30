use crate::errors::RelayerError;
use crate::subscriber::Deposit;
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

mod tests {
    use std::env;

    use alloy::transports::http::reqwest::Url;

    use crate::{
        includer::{self, Includer},
        utils::get_dst_contract_addr,
    };

    use super::*;
    #[tokio::test]
    async fn test_publish_and_consume() {
        const ADDRESS_PATH: &str = "../project_eth/data/deployments.json";
        unsafe {
            std::env::set_var(
                "PRIVATE_KEY",
                "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            );
        }
        let mut con = get_queue_connection().await.unwrap();
        let test_deposit = Deposit {
            sender: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
                .parse()
                .unwrap(),
            amount: 42,
        };
        let test_item = serde_json::to_vec(&test_deposit).unwrap();
        let resp = con.publish(&test_item).await;
        assert!(resp.is_ok());
        let mut consumer = con.consumer().await.unwrap();
        let dst_rpc = "http://localhost:8546";
        let rpc_url_dst: Url = dst_rpc.parse().unwrap();
        let dst_contract_address = get_dst_contract_addr(ADDRESS_PATH).unwrap();
        let incl_res =
            includer::Includer::new(&rpc_url_dst, dst_contract_address, con.clone()).await;
        assert!(incl_res.is_ok());
        let incl = incl_res.unwrap();
        let res = incl.consume(&mut consumer).await;
        assert!(res.is_ok());
        let tuple: (Deposit, lapin::message::Delivery) = res.unwrap();
        let (received_deposit, delivery) = tuple;
        assert_eq!(received_deposit, test_deposit);
        let res = incl.ack_deposit(delivery).await;
        assert!(res.is_ok());
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
