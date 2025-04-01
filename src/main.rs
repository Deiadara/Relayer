mod subscriber;
mod includer;
use std::{fs,thread, time};
use dotenv::dotenv;
use std::env;

use alloy::{
    primitives::{keccak256, Address},
    rpc::types::eth::BlockNumberOrTag
};
use eyre::Result;
use serde_json::Value;
// use alloy_dyn_abi::{DynSolType, DynSolValue};

// do it with library



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
            let _ = includer::mint(&rpc_url_dst, dep.amount, dst_bytecode, dst_abi).await;
        }

        let two_sec = time::Duration::from_millis(2000);
        thread::sleep(two_sec);

        //provider.get_transaction_receipt(hash)
        // check result's logs https://www.quicknode.com/docs/ethereum/eth_getTransactionReceipt

    }
}


// check if alloy can call contracts on other end
// provider (eth rpc) can check with the tx hash of the tx of the emitted event, check that the logs contain the Mint(...)