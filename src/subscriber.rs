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
use std::env;
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
    async fn get_last_offset(&mut self, key: &str) -> redis::RedisResult<u64>; // block ,    needs to return number and work for any cache
    async fn set_last_offset(&mut self, key: &str, value: u64) -> redis::RedisResult<()>;
}

pub struct RedisCache {
    pub connection: MultiplexedConnection,
}

// impl CacheTrait for RedisCache {
//     fun1
//     fun2
// }

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
    ) -> Result<Self> {
        let db_url = env::var("DB_URL").expect("DB_URL not set");

        let client = Client::open(db_url)?;
        let con = client.get_multiplexed_async_connection().await?;

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
        let mut from_block: u64 = 0;
        // this is a function of RedisCache
        let from_block_response: Option<u64> = self
            .con
            .get("from_block")
            .await
            .map_err(|e| RelayerError::RedisError(e.to_string()))?;
        if from_block_response.is_some() {
            from_block = from_block_response.unwrap();
        }
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

        println!("Scanning from {} to {to_block}...", from_block + 1);
        println!("Filter topic0: {:?}", B256::from(self.event_sig));

        let logs = self
            .provider
            .get_logs(&filter)
            .await
            .map_err(|e| RelayerError::ProviderError(e.to_string()))?;

        println!("Got {} logs", logs.len());

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

        let response: String = self
            .con
            .set("from_block", to_block)
            .await
            .map_err(|e| RelayerError::RedisError(e.to_string()))?;
        println!("Response: {}", response);

        Ok(deposits)
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
// tracing library
// try to make contract throw error and check receipt and logs if they exist

// mock redis to return a certain block and test that getlogs i called with the correct filter (also mock provider)
// aka fix from and to block and check that filter filters correctly    check mockall rs  (maybe need wrappers and custom templates for providers etc eg builder needs Provider normally but mockProvider in test cases)

// make push testable

// includer : mock contract and provider and see that they re called with the correct arguments

// check photo for queue and redis abstraction
