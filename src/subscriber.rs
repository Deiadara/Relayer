use eyre::Result;
use std::{convert::TryInto,str};

use alloy::{
    primitives::{Address, Bytes, FixedBytes, B256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{eth::BlockNumberOrTag, Filter},
    dyn_abi::{DynSolType, DynSolValue}
};


#[derive(Debug)]
pub struct Deposit {
    pub sender: Address,
    pub amount: i32,
}

pub async fn get_deposits(rpc_url: &alloy::transports::http::reqwest::Url , event_sig : FixedBytes<32>, contract_address : Address, save_block : BlockNumberOrTag) -> Result<(Vec<Deposit>,BlockNumberOrTag)> {

    let mut deposits = Vec::new();

    let provider = ProviderBuilder::new().on_http(rpc_url.clone());

    let last_block= BlockNumberOrTag::Latest;

    let filter = Filter::new().address(contract_address).from_block(save_block)
        .to_block(last_block);   

        // if we want to check just the final block
        //let filter = Filter::new().address(contract_address).from_block(latest_block);

    println!("Scanning from earliest to latest...");
    println!("Filter topic0: {:?}", B256::from(event_sig));

    let logs = provider.get_logs(&filter).await?;

    println!("Latest block: {:?}", save_block);   
    println!("Got {} logs", logs.len());

    for log in logs {
        println!("Transfer event: {log:?}");
        // extract address and amount
        let topics = log.topics();
        let raw_topic = topics.get(1).expect("Expected at least 2 topics");

        let topic_bytes = raw_topic.as_ref();

        let decoded = DynSolType::Address.abi_decode(topic_bytes)?;
        let sender = match decoded {
            DynSolValue::Address(addr) => addr,
            _ => return Err(eyre::eyre!("Expected address in topic")),
        };

        println!("Sender address: {:?}", sender);

        let raw_data = log.data().data.clone();

        let decoded = DynSolType::String.abi_decode(&raw_data.clone())?;

        let amount_str = match decoded {
            DynSolValue::String(s) => s,
            _ => return Err(eyre::eyre!("Expected string in event data")),
        };

        let my_int = amount_str.parse::<i32>().unwrap();

        deposits.push(Deposit { sender : sender, amount: my_int });

        }
    Ok((deposits,last_block))
    
    
}