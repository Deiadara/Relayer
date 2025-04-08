mod subscriber;
mod includer;
mod utils;
mod errors;
mod queue;
use std::{fs,thread, time, env};
use dotenv::dotenv;
use alloy::{primitives::Address,
    transports::http::reqwest::Url};
use eyre::Result;
use serde_json::Value;
use utils::log_to_deposit;

const ADDRESS_PATH : &str = "../project_eth/data/deployments.json";

#[tokio::main]
async fn main() -> Result<()> {

    dotenv().ok();

    let src_rpc = env::var("SRC_RPC").expect("SRC_RPC not set");
    let dst_rpc = env::var("DST_RPC").expect("DST_RPC not set");

    let address_str = fs::read_to_string(ADDRESS_PATH)?;
    let json: Value = serde_json::from_str(&address_str)?;

    let contract_addr = json["Deposit"].as_str().expect("Deposit address not found");
    let contract_address: Address = contract_addr.parse()?;

    let dst_contract_addr = json["Token"].as_str().expect("Deposit address not found");
    let dst_contract_address: Address = dst_contract_addr.parse()?;

    println!("Loaded deposit_address: {:?}", contract_address);

    let rpc_url : Url  = src_rpc.parse()?;
    let rpc_url_dst: Url = dst_rpc.parse()?;

    let mut sub = subscriber::Subscriber::new(&rpc_url, contract_address).await.unwrap();
    let incl = includer::Includer::new(&rpc_url_dst, dst_contract_address).unwrap();

    match  queue::test_queue_send().await {
        Ok(_) => {
            println!("Successfully sent");
        }
        Err(e) => {
            eprintln!("Error processing sent: {:?}", e);
        }
    }

    match queue::test_queue_receive().await {
        Ok(_) => {
            println!("Successfully received");
        }
        Err(e) => {
            eprintln!("Error processing receive: {:?}", e);
        }
    }

    loop {
        let deposits = sub.get_deposits().await?;
        for dep in deposits {
            match log_to_deposit(dep, &incl).await {
                Ok(_) => {
                    println!("Successfully processed deposit");
                }
                Err(e) => {
                    eprintln!("Error processing deposit: {:?}", e);
                }
            }
        }
        
        let two_sec = time::Duration::from_millis(2000);
        thread::sleep(two_sec);
    }
}
