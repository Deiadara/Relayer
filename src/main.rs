mod subscriber;
mod includer;
use std::{fs,thread, time};
use dotenv::dotenv;
use std::env;
use alloy::{
    primitives::{keccak256, Address}, rpc::types::eth::BlockNumberOrTag
};
use eyre::Result;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<()> {

    dotenv().ok();

    let src_rpc = env::var("SRC_RPC").expect("SRC_RPC not set");
    let dst_rpc = env::var("DST_RPC").expect("DST_RPC not set");

    let token_data_path = "../project_eth/data/TokenData.json";
    let data_str = fs::read_to_string(token_data_path)?;
    let data_json: Value = serde_json::from_str(&data_str)?;
    let dst_abi = &data_json["abi"];
    println!("Loaded token_abi: {:?}", dst_abi);

    let dst_bytecode = data_json["evm"]["bytecode"]["object"]
    .as_str()
    .expect("Bytecode not found");
    println!("Loaded dst_bytecode: {:?}", dst_bytecode);

    let address_path = "../project_eth/data/deployments.json";
    let address_str = fs::read_to_string(address_path)?;
    let json: Value = serde_json::from_str(&address_str)?;
    let contract_addr = json["Deposit"]
    .as_str()
    .expect("Deposit address not found");
    let contract_address: Address = contract_addr.parse()?;
    println!("Loaded deposit_address: {:?}", contract_address);

    let rpc_url:alloy::transports::http::reqwest::Url  = src_rpc.parse()?;
    let rpc_url_dst: alloy::transports::http::reqwest::Url = dst_rpc.parse()?;

    let event_sig = keccak256("Deposited(address,string)");
    let mut save_block = BlockNumberOrTag::Earliest;

    loop {
        let deposits_tuple = subscriber::get_deposits(&rpc_url, event_sig, contract_address, save_block).await?;
        let deposits = deposits_tuple.0;
        save_block = deposits_tuple.1;

        for dep in deposits {
            match includer::mint(&rpc_url_dst, dep.amount, dst_abi).await {
                Ok(Some(receipt)) => {
                    println!("Transaction successful! Receipt: {:?}", receipt);
                    // Receipt's logs : https://www.quicknode.com/docs/ethereum/eth_getTransactionReceipt
                    if let Some(first_log) = receipt.logs().get(0) {
                        if let Some(event_hash) = first_log.topics().get(0) {
                            println!("Event signature hash (topic[0]): {:?}", event_hash);
                            let sig = keccak256("Minted(address,string)");
                            println!("{:?}", sig);
                            if sig == event_hash.clone() {
                                println!("Comparisson successful : Tokens were minted");
                            }
                            else {
                                println!("Hashes were not the same");
                            }
                        } else {
                            println!("No topics in first log.");
                        }
                    }
                    else {
                        println!("No logs found in the receipt.");
                    }
                }
                Ok(None) => {
                    println!("Transaction sent, but no receipt found yet.");
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