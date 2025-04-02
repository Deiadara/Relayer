use std::{fs,env};
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

pub struct Includer {
    dst_rpc_url : alloy::transports::http::reqwest::Url,
    contract_address : Address
}

impl Includer {
    pub fn new(dst_rpc_url: &alloy::transports::http::reqwest::Url ,contract_address : Address) -> Self {
        Self {
            dst_rpc_url: dst_rpc_url.clone(),
            contract_address
        }
    }

    pub async fn mint(&self, amount : i32) -> Result<Option<TransactionReceipt>> {
        println!("New deposit of amount {}",amount);
    
        let pk_str= env::var("PRIVATE_KEY").expect("Private key not set");
        let pk: PrivateKeySigner = pk_str.parse()?;
        
        let wallet = EthereumWallet::from(pk);
     
        let provider = ProviderBuilder::new().wallet(wallet).on_http(self.dst_rpc_url.clone());
    
        let token_data_path = "../project_eth/data/TokenData.json";
        let data_str = fs::read_to_string(token_data_path)?;
        let data_json: Value = serde_json::from_str(&data_str)?;
        let dst_abi = &data_json["abi"];
    
        let abi = serde_json::from_str(&dst_abi.to_string())?;
    
        let str_amount = amount.to_string();
        let number_value = DynSolValue::from(String::from(str_amount.clone()));
    
        let contract = ContractInstance::new(self.contract_address, provider.clone(), Interface::new(abi));
        let tx_hash = contract.function("mint", &[number_value])?.send().await?.watch().await?;
        println!("tx_hash: {tx_hash}");
        let receipt = provider.get_transaction_receipt(tx_hash).await?;
        Ok(receipt)
    }
}