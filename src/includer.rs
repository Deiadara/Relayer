use std::str;

use alloy::{
    network::TransactionBuilder, primitives::{hex,Bytes}, 
    providers::{Provider, ProviderBuilder}, 
    rpc::types::TransactionRequest,
    contract::{ContractInstance, Interface},
    dyn_abi::DynSolValue
};
use eyre::Result;
use serde_json::Value;

pub async fn mint(rpc_url : &alloy::transports::http::reqwest::Url, amount : i32, bytecode_str : &str, dst_abi : &Value) -> Result<i32> {
    println!("New deposit of amount {}",amount);
    let bytecode = hex::decode(
        bytecode_str)?;

    let provider = ProviderBuilder::new().on_http(rpc_url.clone());
    let mut tx = TransactionRequest::default();
    tx.set_deploy_code(Bytes::from(bytecode));

    let contract_address = provider
        .send_transaction(tx)
        .await?
        .get_receipt()
        .await?
        .contract_address
        .expect("Failed to get contract address");

    let abi = serde_json::from_str(&dst_abi.to_string())?;

    let contract = ContractInstance::new(contract_address, provider.clone(), Interface::new(abi));

    let str_amount = amount.to_string();
    let number_value = DynSolValue::from(String::from(str_amount));
    let tx_hash = contract.function("mint", &[number_value])?.send().await?.watch().await?;
    println!("{}", tx_hash);
    let result = provider.get_transaction_receipt(tx_hash).await?;
    println!("{:?}", result);
    Ok(32)
}