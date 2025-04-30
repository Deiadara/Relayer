use crate::errors::RelayerError;
use crate::queue::QueueTrait;
use alloy::{
    dyn_abi::{DynSolType, DynSolValue},
    primitives::{Address, B256, FixedBytes, keccak256},
    providers::{
        Identity, Provider, ProviderBuilder, RootProvider,
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    },
    rpc::types::Filter,
    transports::http::reqwest::Url,
};
use async_trait::async_trait;
use eyre::Result;
use mockall::predicate::*;
use redis::{AsyncCommands, Client, aio::MultiplexedConnection}; // make connection pool at some point
use serde::{Deserialize, Serialize};
use std::{env, thread, time};
use tracing::{debug, error, info};
type ProviderType = FillProvider<
    JoinFill<
        Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    RootProvider,
>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]

pub struct Deposit {
    pub sender: Address,
    pub amount: i32,
}

pub struct Subscriber<C: QueueTrait> {
    pub contract_address: Address,
    pub provider: ProviderType,
    pub event_sig: FixedBytes<32>,
    pub con: MultiplexedConnection,
    pub queue_connection: C,
}
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CacheTrait {
    async fn get_last_offset(&mut self, key: &str) -> Result<u64, RelayerError>;
    async fn set_last_offset(&mut self, key: &str, value: u64) -> Result<(), RelayerError>;
}

pub struct RedisCache {
    pub connection: MultiplexedConnection,
}

#[async_trait]
impl CacheTrait for RedisCache {
    async fn get_last_offset(&mut self, key: &str) -> Result<u64, RelayerError> {
        let mut from_block: u64 = 0;
        let from_block_response: Option<u64> = self
            .connection
            .get(key)
            .await
            .map_err(|e| RelayerError::RedisError(e.to_string()))?;
        if from_block_response.is_some() {
            from_block = from_block_response.unwrap();
        }
        Ok(from_block)
    }

    async fn set_last_offset(&mut self, key: &str, value: u64) -> Result<(), RelayerError> {
        let _res: () = self
            .connection
            .set(key, value)
            .await
            .map_err(|e| RelayerError::RedisError(e.to_string()))?;
        Ok(())
    }
}

// #[async_trait]
// impl CacheTrait for MultiplexedConnection {
//     async fn get_last_offset(&mut self, key: &str) -> redis::RedisResult<u64> {
//         AsyncCommands::get(self, key).await
//     }

//     async fn set_last_offset(&mut self, key: &str, value: u64) -> redis::RedisResult<()> {
//         AsyncCommands::set(self, key, value).await
//     }
// }

const DEPOSIT_EVENT_SIG: &str = "Deposited(address,string)";

impl<C: QueueTrait> Subscriber<C> {
    pub async fn new(
        rpc_url: &Url,
        contract_address: Address,
        queue_connection: C,
    ) -> Result<Self, RelayerError> {
        let db_url = env::var("DB_URL").expect("DB_URL not set");

        let client: Client =
            Client::open(db_url).map_err(|e| RelayerError::RedisError(e.to_string()))?;
        let con: MultiplexedConnection = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RelayerError::RedisError(e.to_string()))?;

        let event_sig = keccak256(DEPOSIT_EVENT_SIG);
        let provider: ProviderType = ProviderBuilder::new().on_http(rpc_url.clone());

        Ok(Self {
            contract_address,
            provider,
            event_sig,
            con,
            queue_connection,
        })
    }

    pub async fn get_deposits(&mut self) -> Result<Vec<Deposit>, RelayerError> {
        let mut cache = RedisCache {
            connection: self.con.clone(),
        };
        let from_block = cache.get_last_offset("from_block").await?;
        let mut deposits = Vec::new();
        let to_block = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| RelayerError::ProviderError(e.to_string()))?;
        let filter = Filter::new()
            .address(self.contract_address)
            .from_block(from_block + 1)
            .to_block(to_block);

        info!("Scanning from {} to {to_block}...", from_block + 1);
        debug!("Filter topic0: {:?}", B256::from(self.event_sig));

        let logs = self
            .provider
            .get_logs(&filter)
            .await
            .map_err(|e| RelayerError::ProviderError(e.to_string()))?;

        info!("Got {} logs", logs.len());

        for log in logs {
            println!("Transfer event: {log:?}");
            let topics = log.topics();
            let raw_topic = topics.get(1).expect("Expected at least 2 topics");

            let topic_bytes = raw_topic.as_ref();

            let decoded = DynSolType::Address.abi_decode(topic_bytes)?;
            let sender = match decoded {
                DynSolValue::Address(addr) => addr,
                _ => return Err(RelayerError::NoAddress),
            };

            let raw_data = log.data().data.clone();

            let decoded = DynSolType::String.abi_decode(&raw_data.clone())?;

            let amount_str = match decoded {
                DynSolValue::String(s) => s,
                _ => return Err(RelayerError::NotString),
            };

            let amount = amount_str.parse::<i32>().unwrap();

            deposits.push(Deposit { sender, amount });
        }

        if let Err(e) = cache.set_last_offset("from_block", to_block).await {
            error!("Failed to set last_offset: {:?}", e);
        } else {
            debug!("last_offset updated successfully");
        }

        Ok(deposits)
    }

    pub async fn run(&mut self) {
        loop {
            if let Err(e) = self.work().await {
                error!("Error: {:?}", e);
            }
            let two_sec = time::Duration::from_millis(2000);
            thread::sleep(two_sec);
        }
    }

    async fn work(&mut self) -> Result<(), RelayerError> {
        let deposits = self.get_deposits().await?;
        for dep in deposits {
            let serialized_deposit = serde_json::to_vec(&dep).map_err(RelayerError::SerdeError)?;
            info!("Event emitted from sender: {:?}", dep.sender);
            match self.queue_connection.publish(&serialized_deposit).await {
                Ok(_) => {
                    info!("Successfully processed deposit");
                }
                Err(e) => {
                    error!("Error processing deposit: {:?}", e);
                    return Err(RelayerError::Other(e.to_string()));
                }
            }
        }
        Ok(())
    }
}

// mod tests {
//     use super::*;
//     #[tokio::test]
//     async fn test_get_queue_connection_consumer() {
//         let mut mock_redis = MockRedisClient::new();
//         mock_redis
//             .expect_get_last_offset()
//             .with(eq("last_offset"))
//             .times(1)
//             .returning(|_| Ok(0));
//         let queue_connection: QueueConnectionConsumer =
//             get_queue_connection_consumer_with_redis(Box::new(mock_redis))
//                 .await
//                 .unwrap();
//         assert_eq!(queue_connection.stream, "relayer-stream-105");
//     }
// }

//todo :
// try to make contract throw error and check receipt and logs if they exist

// mock redis to return a certain block and test that getlogs i called with the correct filter (also mock provider)
// aka fix from and to block and check that filter filters correctly    check mockall rs  (maybe need wrappers and custom templates for providers etc eg builder needs Provider normally but mockProvider in test cases)

// make push testable

// includer : mock contract and provider and see that they re called with the correct arguments
