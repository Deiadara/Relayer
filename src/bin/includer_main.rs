use alloy::{primitives::Address, transports::http::reqwest::Url};
use dotenv::dotenv;
use eyre::Result;
use relayer::includer;
use relayer::queue::{self, Queue};
use relayer::utils::verify_minted_log;
use serde_json::Value;
use std::{env, fs, thread, time};
const ADDRESS_PATH: &str = "../project_eth/data/deployments.json";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let dst_rpc = env::var("DST_RPC").expect("DST_RPC not set");

    let address_str = fs::read_to_string(ADDRESS_PATH)?;
    let json: Value = serde_json::from_str(&address_str)?;

    let contract_addr = json["Deposit"].as_str().expect("Deposit address not found");
    let contract_address: Address = contract_addr.parse()?;

    let dst_contract_addr = json["Token"].as_str().expect("Deposit address not found");
    let dst_contract_address: Address = dst_contract_addr.parse()?;

    println!("Loaded deposit_address: {:?}", contract_address);

    let rpc_url_dst: Url = dst_rpc.parse()?;

    let queue_connection = queue::get_queue_connection_consumer().await?;

    let mut incl =
        includer::Includer::new(&rpc_url_dst, dst_contract_address, queue_connection).await?;

    loop {
        match incl.queue_connection.consume().await {
            Ok(dep) => {
                println!("Successfully received");
                match incl.mint(dep.amount).await {
                    Ok(Some(receipt)) => {
                        println!("Transaction successful! Receipt: {:?}", receipt);
                        if !receipt.status() {
                            println!("Transaction failed, status is 0");
                        } else {
                            match verify_minted_log(&receipt) {
                                Ok(_) => {
                                    println!("Tokens minted succesfully!");
                                }
                                Err(e) => {
                                    eprint!("Couldn't verify minted log : {}", e)
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        println!("Transaction sent, but no receipt found.");
                    }
                    Err(e) => {
                        eprint!("Error minting : {:?}", e)
                    }
                }
            }
            Err(e) => {
                eprintln!("Error processing receive: {:?}", e);
            }
        }
        let two_sec = time::Duration::from_millis(2000);
        thread::sleep(two_sec);
    }
}
