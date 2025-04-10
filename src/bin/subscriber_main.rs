use alloy::{primitives::Address, transports::http::reqwest::Url};
use dotenv::dotenv;
use eyre::Result;
use relayer::queue::{self, Queue};
use relayer::subscriber;
use serde_json::Value;
use std::{env, fs, thread, time};
const ADDRESS_PATH: &str = "../project_eth/data/deployments.json";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let src_rpc = env::var("SRC_RPC").expect("SRC_RPC not set");

    let address_str = fs::read_to_string(ADDRESS_PATH)?;
    let json: Value = serde_json::from_str(&address_str)?;

    let contract_addr = json["Deposit"].as_str().expect("Deposit address not found");
    let contract_address: Address = contract_addr.parse()?;

    println!("Loaded deposit_address: {:?}", contract_address);

    let rpc_url: Url = src_rpc.parse()?;

    let queue_connection = queue::get_queue_connection_writer().await?;

    let mut sub = subscriber::Subscriber::new(&rpc_url, contract_address, queue_connection)
        .await
        .unwrap();

    loop {
        let deposits = sub.get_deposits().await?;
        for dep in deposits {
            match sub.queue_connection.push(dep).await {
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
