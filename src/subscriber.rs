use alloy::{
    dyn_abi::{DynSolType, DynSolValue}, primitives::{keccak256, Address, FixedBytes, B256}, providers::{
        fillers::{BlobGasFiller,ChainIdFiller,FillProvider,GasFiller,JoinFill,NonceFiller}, Identity, Provider, ProviderBuilder, RootProvider
    }, rpc::types::Filter
};

use eyre::Result;

type ProviderType = FillProvider<JoinFill<Identity,JoinFill<GasFiller,JoinFill<BlobGasFiller,JoinFill<NonceFiller, ChainIdFiller>>>>,RootProvider>;

#[derive(Debug)]
pub struct Deposit {
    pub sender: Address,
    pub amount: i32,
}

pub struct Subscriber {
    contract_address : Address,
    provider : ProviderType,
    event_sig : FixedBytes<32>
}

impl Subscriber {
    pub fn new(rpc_url: &alloy::transports::http::reqwest::Url ,contract_address : Address) -> Self {

        let event_sig = keccak256("Deposited(address,string)");
        let provider: ProviderType = ProviderBuilder::new().on_http(rpc_url.clone());

        Self {
            contract_address,
            provider,
            event_sig
        }
    }

    pub async fn get_deposits(&self, from_block : u64) -> Result<(Vec<Deposit>,u64)> {    

        let mut deposits = Vec::new();
        let to_block = self.provider.get_block_number().await?;
        let filter = Filter::new().address(self.contract_address).from_block(from_block + 1).to_block(to_block);   
    
        println!("Scanning from {} to {to_block}...", from_block+1);
        println!("Filter topic0: {:?}", B256::from(self.event_sig));
    
        let logs = self.provider.get_logs(&filter).await?;
    
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
