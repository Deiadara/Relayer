use alloy::transports::http::reqwest::Url;
use dotenv::dotenv;
use eyre::Result;
use mockall;
use relayer::errors::RelayerError;
use relayer::queue::{self, QueueTrait};
use relayer::subscriber;
use relayer::utils::{get_src_contract_addr,setup_logging};
use std::{env, thread, time};
use tracing::{debug, error, info};

const ADDRESS_PATH: &str = "../project_eth/data/deployments.json";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    setup_logging();

    let src_rpc = env::var("SRC_RPC").expect("SRC_RPC not set");
    let rpc_url: Url = src_rpc.parse()?;
    let src_contract_address = get_src_contract_addr(ADDRESS_PATH)?;
    debug!("Loaded deposit_address: {:?}", src_contract_address);

    let queue_connection = queue::get_queue_connection().await?;

    let mut sub = subscriber::Subscriber::new(&rpc_url, src_contract_address, queue_connection)
        .await
        .unwrap();
    // move to subscriber
    
    let _res = sub.run().await;
    Ok(())

}
