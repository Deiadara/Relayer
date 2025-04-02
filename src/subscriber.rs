use eyre::Result;
use alloy::{
    primitives::{Address, B256,keccak256,},
    providers::{Provider, ProviderBuilder},
    rpc::types::{eth::BlockNumberOrTag, Filter},
    dyn_abi::{DynSolType, DynSolValue}
};


#[derive(Debug)]
pub struct Deposit {
    pub sender: Address,
    pub amount: i32,
}

pub struct Subscriber {
    rpc_url : alloy::transports::http::reqwest::Url,
    contract_address : Address
}

impl Subscriber {
    pub fn new(rpc_url: &alloy::transports::http::reqwest::Url ,contract_address : Address) -> Self {
        Self {
            rpc_url: rpc_url.clone(),
            contract_address
        }
    }

    pub async fn get_deposits(&self, from_block : BlockNumberOrTag) -> Result<(Vec<Deposit>,BlockNumberOrTag)> {

        let event_sig = keccak256("Deposited(address,string)");
    
        let mut deposits = Vec::new();
    
        let provider = ProviderBuilder::new().on_http(self.rpc_url.clone());
    
        let to_block= BlockNumberOrTag::Latest; // move to constructor and initiate redis, keep the url inside the constructor and connect to it (like provider)
    
        let filter = Filter::new().address(self.contract_address).from_block(from_block)
            .to_block(to_block);   
    
        println!("Scanning from {from_block} to {to_block}...");
        println!("Filter topic0: {:?}", B256::from(event_sig));
    
        let logs = provider.get_logs(&filter).await?;
    
        println!("Got {} logs", logs.len());
    
        for log in logs {
            println!("Transfer event: {log:?}");
            let topics = log.topics();
            let raw_topic = topics.get(1).expect("Expected at least 2 topics");
    
            let topic_bytes = raw_topic.as_ref();
    
            let decoded = DynSolType::Address.abi_decode(topic_bytes)?;
            let sender = match decoded {
                DynSolValue::Address(addr) => addr,
                _ => return Err(eyre::eyre!("Expected address in topic")),
            };
    
            let raw_data = log.data().data.clone();
    
            let decoded = DynSolType::String.abi_decode(&raw_data.clone())?;
    
            let amount_str = match decoded {
                DynSolValue::String(s) => s,
                _ => return Err(eyre::eyre!("Expected string in event data")),
            };
    
            let amount = amount_str.parse::<i32>().unwrap();
    
            deposits.push(Deposit { sender, amount });
    
            }
        Ok((deposits,to_block))
        
    }

}

// get_deposits should have no arguments everything is in the state
