use eyre::Result;
use std::{convert::TryInto,str};

use alloy::{
    primitives::{Address, Bytes, FixedBytes, B256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{eth::BlockNumberOrTag, Filter}
};

#[derive(Debug)]
pub struct Deposit {
    pub sender: Address,
    pub amount: i32,
}


fn decode_string_from_hex(data: &Bytes) -> Result<String> {
    let raw = data.as_ref();

    if raw.len() < 64 {
        return Err(eyre::eyre!("Log data too short to be valid ABI-encoded string"));
    }

    // The string length is at bytes 32..64 (second word)
    let length_bytes = &raw[32..64];
    let str_len = u32::from_be_bytes(length_bytes[28..32].try_into().unwrap()) as usize;

    // The string data starts at byte 64
    let string_data = &raw[64..64 + str_len];

    // Convert to UTF-8 string
    let decoded = str::from_utf8(string_data)?.to_string();
    Ok(decoded)
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

        // topic is 32 bytes, we need the last 20 bytes
        let raw_bytes: &[u8] = raw_topic.as_ref();
        let address_bytes: [u8; 20] = raw_bytes[12..].try_into().expect("Expected 20-byte slice");

        // abi decode alloy

        let sender = Address::from(address_bytes);

        println!("Sender address: {:?}", sender);

        let raw_data = log.data().data.clone();

        let string_from_hex = decode_string_from_hex(&raw_data)?;
        let my_int = string_from_hex.parse::<i32>().unwrap();

        deposits.push(Deposit { sender : sender, amount: my_int });


        }
    Ok((deposits,last_block))
    
    
}