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

#[tokio::main]
async fn main() -> Result<()> {

    dotenv().ok();

    let src_rpc = env::var("SRC_RPC").expect("SRC_RPC not set");
    let dst_rpc = env::var("DST_RPC").expect("DST_RPC not set");

    let address_path = "../project_eth/data/deployments.json";
    let address_str = fs::read_to_string(address_path)?;
    let json: Value = serde_json::from_str(&address_str)?;
    let contract_addr = json["Deposit"]
    .as_str()
    .expect("Deposit address not found");
    let contract_address: Address = contract_addr.parse()?;
    let dst_contract_addr = json["Token"]
    .as_str()
    .expect("Deposit address not found");
    let dst_contract_address: Address = dst_contract_addr.parse()?;

    println!("Loaded deposit_address: {:?}", contract_address);

    let rpc_url:alloy::transports::http::reqwest::Url  = src_rpc.parse()?;
    let rpc_url_dst: alloy::transports::http::reqwest::Url = dst_rpc.parse()?;

    let mut from_block = BlockNumberOrTag::Earliest; // read from redis key value store, if doesnt exist start from zero

    let sub = subscriber::Subscriber::new(&rpc_url, contract_address);
    let incl = includer::Includer::new(&rpc_url_dst, dst_contract_address);

    loop {
        // Make a utils.rs file that has verify minted log in it
        // sub.get_deposits(block)
        let deposits_tuple = sub.get_deposits(from_block).await?; // rpc_url, contract_addr
        let deposits = deposits_tuple.0;
        from_block = deposits_tuple.1;
        for dep in deposits {
            println!("Event emitted from sender : {:?}", dep.sender);
            match incl.mint(dep.amount).await { // hard core rpc and abi
                Ok(Some(receipt)) => {
                    println!("Transaction successful! Receipt: {:?}", receipt);
                    // Receipt's logs : https://www.quicknode.com/docs/ethereum/eth_getTransactionReceipt

                    // make func verify minted log which throws errors (custom made)  call with ?
                    // let first_log = receipt.logs().get(0).ok_or_else(err)? // ? means unwrap or throw
                    // returns Result<(), custom_err_wrapper>
                    if let Some(first_log) = receipt.logs().get(0) {
                        if let Some(event_hash) = first_log.topics().get(0) { // same for this
                            println!("Event signature hash (topic[0]): {:?}", event_hash);
                            let sig = keccak256("Minted(address,string)");
                            println!("{:?}", sig);
                            if sig == event_hash.clone() {
                                println!("Comparisson successful : Tokens were minted"); // return Ok()
                            }
                            else
                                {println!("Hashes were not the same");} // return err NotEqual
                        } else {
                            println!("No topics in first log."); // error instead
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

// make includer and subscriber into a class
// make an instance of each so that we dont pass arguments all the time (rpcurl , event sig etc)
// make the custom errors
// read redis (Non Relational Database chapter) and when relayer goes down, keep last block checked so that when restarted it can start from there
// add redis to gitbook