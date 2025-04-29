use alloy::transports::http::reqwest::Url;
use dotenv::dotenv;
use eyre::Result;
use mockall;
use relayer::errors::RelayerError;
use relayer::queue::{self, QueueTrait};
use relayer::subscriber;
use relayer::utils::get_src_contract_addr;
use std::{env, thread, time};

const ADDRESS_PATH: &str = "../project_eth/data/deployments.json";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let src_rpc = env::var("SRC_RPC").expect("SRC_RPC not set");
    let rpc_url: Url = src_rpc.parse()?;
    let src_contract_address = get_src_contract_addr(ADDRESS_PATH)?;
    println!("Loaded deposit_address: {:?}", src_contract_address);

    let queue_connection = queue::get_queue_connection().await?;

    let mut sub = subscriber::Subscriber::new(&rpc_url, src_contract_address, queue_connection)
        .await
        .unwrap();
    // move to subscriber
    loop {
        let deposits = sub.get_deposits().await?;
        for dep in deposits {
            let serialized_deposit = serde_json::to_vec(&dep).map_err(RelayerError::SerdeError)?;
            println!("Event emitted from sender: {:?}", dep.sender);
            match sub.queue_connection.publish(&serialized_deposit).await {
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
