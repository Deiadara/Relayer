use crate::errors::RelayerError;
use crate::queue::Queue;
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
use eyre::Result;
use redis::{AsyncCommands, Client, aio::MultiplexedConnection};
use serde::{Deserialize, Serialize};
use std::env;

type ProviderType = FillProvider<
    JoinFill<
        Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    RootProvider,
>;

#[derive(Serialize, Deserialize, Debug)]

pub struct Deposit {
    pub sender: Address,
    pub amount: i32,
}

pub struct Subscriber<C: Queue> {
    pub contract_address: Address,
    pub provider: ProviderType,
    pub event_sig: FixedBytes<32>,
    pub con: MultiplexedConnection,
    pub queue_connection: C,
}

const DEPOSIT_EVENT_SIG: &str = "Deposited(address,string)";

impl<C: Queue> Subscriber<C> {
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

//todo :
// tracing library
// try to make contract throw error and check receipt and logs if they exist

// add unit tests to subscriber : edge cases (empty, wrong format), check that they throw the correct error
// mock redis to return a certain block and test that getlogs i called with the correct filter (also mock provider)
// aka fix from and to block and check that filter filters correctly    check mockall rs  (maybe need wrappers and custom templates for providers etc eg builder needs Provider normally but mockProvider in test cases)

// make deposit_to_log it testable

// includer : mock contract and provider and see that they re called with the correct arguments

// make a queue.rs with a class that initiates a connection to the queue, it will have push and consume
// sub and incl in their main will make an instance of queue. In their constructor they will have the connection
// they will have a field C (connection) which implements the trait which implements trait queue (in queue.rs)
// check photo for queue and redis abstraction
