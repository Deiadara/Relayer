use crate::errors::RelayerError;
use alloy::{
    dyn_abi::{DynSolType, DynSolValue}, primitives::{keccak256, Address, FixedBytes, B256}, providers::{
    fillers::{BlobGasFiller,ChainIdFiller,FillProvider,GasFiller,JoinFill,NonceFiller}, Identity, Provider, ProviderBuilder, RootProvider
    }, rpc::types::Filter
};
use eyre::Result;
use redis::{aio::MultiplexedConnection, AsyncCommands, Client};


type ProviderType = FillProvider<JoinFill<Identity,JoinFill<GasFiller,JoinFill<BlobGasFiller,JoinFill<NonceFiller, ChainIdFiller>>>>,RootProvider>;

#[derive(Debug)]
pub struct Deposit {
    pub sender: Address,
    pub amount: i32,
}

pub struct Subscriber {
    contract_address : Address,
    provider : ProviderType,
    event_sig : FixedBytes<32>,
    con : MultiplexedConnection
}

impl Subscriber {
    pub async fn new(rpc_url: &alloy::transports::http::reqwest::Url ,contract_address : Address) -> Result<Self> {

        let client = Client::open("redis://127.0.0.1/")?;
        let con = client.get_multiplexed_async_connection().await?;

        let event_sig = keccak256("Deposited(address,string)");
        let provider: ProviderType = ProviderBuilder::new().on_http(rpc_url.clone());

            Ok(Self {
                contract_address,
                provider,
                event_sig,
                con
            })
    }

    pub async fn get_deposits(&self) -> Result<Vec<Deposit>,RelayerError> {

        let mut from_block : u64 = 0;
        let from_block_response: Option<u64> = self.con.clone().get("from_block").await.map_err(|e| RelayerError::RedisError(e.to_string()))?;
        if from_block_response.is_some() {
            from_block = from_block_response.unwrap();
        }
        let mut deposits = Vec::new();
        let to_block = self.provider.get_block_number().await.map_err(|e| RelayerError::ProviderError(e.to_string()))?;
        let filter = Filter::new().address(self.contract_address).from_block(from_block + 1).to_block(to_block);   
    
        println!("Scanning from {} to {to_block}...", from_block+1);
        println!("Filter topic0: {:?}", B256::from(self.event_sig));
    
        let logs = self.provider.get_logs(&filter).await.map_err(|e| RelayerError::ProviderError(e.to_string()))?;
    
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
    
            deposits.push(Deposit{sender, amount});

            }

        let response: String = self.con.clone().set("from_block", to_block).await.map_err(|e| RelayerError::RedisError(e.to_string()))?;
        println!("Response: {}", response);
        Ok(deposits)
        
    }

}