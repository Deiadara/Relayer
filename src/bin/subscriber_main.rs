use alloy::transports::http::reqwest::Url;
use dotenv::dotenv;
use eyre::Result;
use mockall;
use relayer::queue::{self, Queue};
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

    let queue_connection = queue::get_queue_connection_writer().await?;

    let mut sub = subscriber::Subscriber::new(&rpc_url, src_contract_address, queue_connection)
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
