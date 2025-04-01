use std::{str,fs};

use alloy::{
    contract::{ContractInstance, Interface}, 
    dyn_abi::DynSolValue, 
    network::{Network, TransactionBuilder}, 
    primitives::{hex, Address, Bytes}, 
    providers::{Provider, ProviderBuilder}, 
    rpc::types::{eth, TransactionReceipt, TransactionRequest}
};
use eyre::Result;
use serde_json::Value;


pub async fn mint(rpc_url : &alloy::transports::http::reqwest::Url, amount : i32, bytecode_str : &str, dst_abi : &Value) -> Result<Option<TransactionReceipt>> {
    println!("New deposit of amount {}",amount);

    let provider = ProviderBuilder::new().on_http(rpc_url.clone());
    let address_path = "../project_eth/data/deployments.json";
    let address_str = fs::read_to_string(address_path)?;
    let json: Value = serde_json::from_str(&address_str)?;
    let contract_addr = json["Token"]
    .as_str()
    .expect("Token address not found");
    let contract_address: Address = contract_addr.parse()?;
    println!("Loaded token_address: {:?}", contract_address);

    let abi = serde_json::from_str(&dst_abi.to_string())?;
    println!("ABI : {:?}", abi);

    let str_amount = amount.to_string();
    let number_value = DynSolValue::from(String::from(str_amount));
    
    let contract = ContractInstance::new(contract_address, provider.clone(), Interface::new(abi));
    let tx_hash = contract.function("mint", &[number_value])?.send().await?.watch().await?;
    println!("tx_hash: {tx_hash}");
    let receipt = provider.get_transaction_receipt(tx_hash).await?;
    println!("receipt: {:?}",receipt);
    Ok(receipt)
}