use alloy::providers::ProviderBuilder;
use alloy::transports::http::reqwest::Url;
use dotenv::dotenv;
use eyre::Result;
use relayer::queue;
use relayer::subscriber::{ProviderType, RedisCache, Subscriber};
use relayer::utils::{get_src_contract_addr, setup_logging};
use std::env;
use tracing::debug;

const ADDRESS_PATH: &str = "../project_eth/data/deployments.json";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    setup_logging();

    let src_rpc = env::var("SRC_RPC").expect("SRC_RPC not set");
    let rpc_url: Url = src_rpc.parse()?;
    let src_contract_address = get_src_contract_addr(ADDRESS_PATH)?;
    debug!("Loaded deposit_address: {:?}", src_contract_address);

    let queue_connection = queue::get_queue_connection(false).await?;
    let db_url = env::var("DB_URL").expect("DB_URL not set");

    let redis_connection = RedisCache::new(db_url).await?;
    let provider: ProviderType = ProviderBuilder::new().on_http(rpc_url.clone());

    let mut sub = Subscriber::new(
        src_contract_address,
        queue_connection,
        redis_connection,
        provider,
    )
    .await
    .unwrap();

    let _res = sub.run().await;
    Ok(())
}
