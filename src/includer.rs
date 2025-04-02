use std::{fs,env};
use dotenv::dotenv;

use alloy::{
    contract::{ContractInstance, Interface}, 
    dyn_abi::DynSolValue, 
    network::EthereumWallet, 
    primitives::Address, 
    providers::{Provider, ProviderBuilder}, 
    rpc::types::TransactionReceipt,
    signers::local::PrivateKeySigner
};
use eyre::Result;
use serde_json::Value;

pub async fn mint(rpc_url : &alloy::transports::http::reqwest::Url, amount : i32, dst_abi : &Value) -> Result<Option<TransactionReceipt>> {
    println!("New deposit of amount {}",amount);
    
    dotenv().ok();

    let pk_str= env::var("PRIVATE_KEY").expect("Private key not set");
    let pk: PrivateKeySigner = pk_str.parse()?;
    
    let wallet = EthereumWallet::from(pk);
 
    let provider = ProviderBuilder::new().wallet(wallet).on_http(rpc_url.clone());

    let address_path = "../project_eth/data/deployments.json";
    let address_str = fs::read_to_string(address_path)?;
    let json: Value = serde_json::from_str(&address_str)?;
    let contract_addr = json["Token"]
    .as_str()
    .expect("Token address not found");
    let contract_address: Address = contract_addr.parse()?;
    println!("Loaded token_address: {:?}", contract_address);

    let abi = serde_json::from_str(&dst_abi.to_string())?;

    let str_amount = amount.to_string();
    let number_value = DynSolValue::from(String::from(str_amount.clone()));

    let contract = ContractInstance::new(contract_address, provider.clone(), Interface::new(abi));
    let tx_hash = contract.function("mint", &[number_value])?.send().await?.watch().await?;
    println!("tx_hash: {tx_hash}");
    let receipt = provider.get_transaction_receipt(tx_hash).await?;
    Ok(receipt)
}