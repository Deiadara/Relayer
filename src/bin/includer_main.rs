use alloy::transports::http::reqwest::Url;
use dotenv::dotenv;
use eyre::Result;
use relayer::includer;
use relayer::queue;
use relayer::utils::{get_dst_contract_addr,setup_logging};
use std::env;

const ADDRESS_PATH: &str = "../project_eth/data/deployments.json";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    setup_logging();

    let dst_rpc = env::var("DST_RPC").expect("DST_RPC not set");
    let rpc_url_dst: Url = dst_rpc.parse()?;
    let dst_contract_address = get_dst_contract_addr(ADDRESS_PATH)?;
    let queue_connection = queue::get_queue_connection().await?;

    let mut incl =
        includer::Includer::new(&rpc_url_dst, dst_contract_address, queue_connection.clone())
            .await?;

    let _res = incl.run().await;

    Ok(())
}
