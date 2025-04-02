mod subscriber;
mod includer;
mod utils;
mod errors;
use std::{fs,thread, time};
use dotenv::dotenv;
use std::env;
use alloy::primitives::Address;
use eyre::Result;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<()> {

    dotenv().ok();

    let src_rpc = env::var("SRC_RPC").expect("SRC_RPC not set");
    let dst_rpc = env::var("DST_RPC").expect("DST_RPC not set");

    let address_path = "../project_eth/data/deployments.json";
    let address_str = fs::read_to_string(address_path)?;
    let json: Value = serde_json::from_str(&address_str)?;

    let contract_addr = json["Deposit"].as_str().expect("Deposit address not found");
    let contract_address: Address = contract_addr.parse()?;

    let dst_contract_addr = json["Token"].as_str().expect("Deposit address not found");
    let dst_contract_address: Address = dst_contract_addr.parse()?;

    println!("Loaded deposit_address: {:?}", contract_address);

    let rpc_url:alloy::transports::http::reqwest::Url  = src_rpc.parse()?;
    let rpc_url_dst: alloy::transports::http::reqwest::Url = dst_rpc.parse()?;

    let mut from_block = 0; // read from redis key value store, if doesnt exist start from zero

    let sub = subscriber::Subscriber::new(&rpc_url, contract_address);
    let incl = includer::Includer::new(&rpc_url_dst, dst_contract_address).unwrap();

    loop {
        let deposits_tuple = sub.get_deposits(from_block).await?;
        let deposits = deposits_tuple.0;
        from_block = deposits_tuple.1;
        for dep in deposits {
            println!("Event emitted from sender : {:?}", dep.sender);
            match incl.mint(dep.amount).await {
                Ok(Some(receipt)) => {
                    println!("Transaction successful! Receipt: {:?}", receipt);
                    utils::verify_minted_log(&receipt)?;
                }
                Ok(None) => {
                    println!("Transaction sent, but no receipt found.");
                    //return Err(RelayerError::NoReceipt.into()); 
                }
                Err(e) => {
                    eprintln!("Minting failed: {:?}", e);
                }
            }
        }

        let two_sec = time::Duration::from_millis(2000);
        thread::sleep(two_sec);
    }
}
// read redis (Non Relational Database chapter) and when relayer goes down, keep last block checked so that when restarted it can start from there
// add redis to gitbook