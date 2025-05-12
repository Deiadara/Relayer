use crate::errors::RelayerError;
use crate::queue::QueueTrait;
use alloy::{
    dyn_abi::{DynSolType, DynSolValue},
    primitives::{Address, B256, FixedBytes, keccak256},
    providers::{
        Identity, Provider, ProviderBuilder, RootProvider,
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    },
    rpc::types::{Filter,Log},
    transports::http::reqwest::Url,
};
use async_trait::async_trait;
use eyre::Result;
use mockall::predicate::*;
use redis::{AsyncCommands, Client, aio::MultiplexedConnection}; // make connection pool at some point
use serde::{Deserialize, Serialize};
use std::{env, thread, time};
use tracing::{debug, error, info};
pub type ProviderType = FillProvider<
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

pub struct Subscriber<C: QueueTrait, R: CacheTrait> {
    pub contract_address: Address,
    pub provider: ProviderType,
    pub event_sig: FixedBytes<32>,
    pub queue_connection: C,
    pub cache_connection: R
}
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CacheTrait {
    async fn get_last_offset(&mut self, key: &str) -> Result<u64, RelayerError>;
    async fn set_last_offset(&mut self, key: &str, value: u64) -> Result<(), RelayerError>;
}

pub struct RedisCache {
    connection: MultiplexedConnection,
}

impl RedisCache {
    pub async fn new(db_url : String) -> Result<Self, RelayerError>{
        let client =
            Client::open(db_url).map_err(|e| RelayerError::RedisError(e.to_string()))?;
        let connection = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RelayerError::RedisError(e.to_string()))?;
        Ok(RedisCache {
            connection
        })
    }
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

const DEPOSIT_EVENT_SIG: &str = "Deposited(address,string)";

impl<C: QueueTrait, R: CacheTrait> Subscriber<C,R> {
    pub async fn new(
        contract_address: Address,
        queue_connection: C,
        cache_connection : R,
        provider : ProviderType
    ) -> Result<Self, RelayerError> {
        // let db_url = env::var("DB_URL").expect("DB_URL not set");

        // let client: Client =
        //     Client::open(db_url).map_err(|e| RelayerError::RedisError(e.to_string()))?;
        // let con: MultiplexedConnection = client
        //     .get_multiplexed_async_connection()
        //     .await
        //     .map_err(|e| RelayerError::RedisError(e.to_string()))?;
        
        let event_sig = keccak256(DEPOSIT_EVENT_SIG);
        // .on_mocked_client
        Ok(Self {
            contract_address,
            provider,
            event_sig,
            queue_connection,
            cache_connection
        })
    }

    pub async fn get_deposits(&mut self, from_block : u64, to_block : u64) -> Result<Vec<Deposit>, RelayerError> {
        if from_block >= to_block {
            return Err(RelayerError::Other(String::from("No blocks to scan")));
        }
        let deposits = Vec::new();
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

        let deposits_res = self.push_deposits(logs,deposits).await?;

        debug!("{:?}", deposits_res);
        Ok(deposits_res)
    }

    pub async fn push_deposits(&self,logs:Vec<Log>,mut deposits : Vec<Deposit>) -> Result<Vec<Deposit>, RelayerError> {
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
        let from_block = self.cache_connection.get_last_offset("from_block").await?;
        let to_block = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| RelayerError::ProviderError(e.to_string()))?;
        let deposits = self.get_deposits(from_block,to_block).await?;
        if let Err(e) = self.cache_connection.set_last_offset("from_block", to_block).await {
            error!("Failed to set last_offset: {:?}", e);
        } else {
            debug!("last_offset updated successfully");
        }
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

#[cfg(test)]
mod tests {
    use alloy::providers::mock::Asserter;

    use crate::{queue::{self, LapinConnection}, utils::get_src_contract_addr};

    use super::*;

    async fn setup_tests() -> (ProviderType, LapinConnection, MockCacheTrait){
        let asserter = Asserter::new();
        let provider: ProviderType = ProviderBuilder::new().on_mocked_client(asserter);
        let queue_connection = queue::get_queue_connection(true).await.unwrap();
        let cache_connection = MockCacheTrait::new();
        (provider, queue_connection, cache_connection)
    }

    #[tokio::test]
    async fn test_get_deposits_err() {
        let src_contract_address = get_src_contract_addr("../project_eth/data/deployments.json").unwrap();
        let (provider, queue_connection, cache_connection) = setup_tests().await;
        let mut sub = Subscriber::new(src_contract_address, queue_connection, cache_connection, provider)
            .await
            .unwrap();

        let res = sub.get_deposits(4,3).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_get_deposits() {
        let src_contract_address = get_src_contract_addr("../project_eth/data/deployments.json").unwrap();
        let (provider, queue_connection, cache_connection) = setup_tests().await;
        let mut sub = Subscriber::new(src_contract_address, queue_connection, cache_connection, provider)
            .await
            .unwrap();

        let res = sub.get_deposits(0,100).await;
        assert!(res.is_err());

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

// check with mockall that functions are called with the correct arguments and what they return is what is expected