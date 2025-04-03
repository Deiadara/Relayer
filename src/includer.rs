use std::{fs,env};
use alloy::{
    contract::{ContractInstance, Interface}, 
    dyn_abi::DynSolValue, 
    network::{EthereumWallet,Ethereum}, 
    primitives::Address, 
    providers::{Provider, ProviderBuilder, fillers::{BlobGasFiller,GasFiller,FillProvider, JoinFill, NonceFiller}, Identity}, 
    rpc::types::TransactionReceipt,
    signers::local::PrivateKeySigner
};
use eyre::Result;
use serde_json::Value;
type ProviderType = FillProvider<JoinFill<JoinFill<Identity, JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, alloy::providers::fillers::ChainIdFiller>>>>, alloy::providers::fillers::WalletFiller<EthereumWallet>>, alloy::providers::RootProvider>;
type ContractType = ContractInstance<ProviderType, Ethereum>;

pub struct Includer {
    provider : ProviderType,
    contract : ContractType
}

impl Includer {
    pub fn new(dst_rpc_url: &alloy::transports::http::reqwest::Url ,contract_address : Address) -> Result<Self> {
        
        
        let token_data_path = "../project_eth/data/TokenData.json";
        let data_str = fs::read_to_string(token_data_path)?;
        let data_json: Value = serde_json::from_str(&data_str)?;
        let abi : alloy::json_abi::JsonAbi = serde_json::from_str(&data_json["abi"].to_string())?;
        let pk_str= env::var("PRIVATE_KEY").expect("Private key not set");
        let pk: PrivateKeySigner = pk_str.parse()?;
        let wallet = EthereumWallet::from(pk);
        let provider = ProviderBuilder::new().wallet(wallet).on_http(dst_rpc_url.clone());
        let contract: ContractType = ContractInstance::new(contract_address, provider.clone(), Interface::new(abi.clone()));
    
        Ok(Self {
            provider,
            contract
        })
    }

    pub async fn mint(&self, amount : i32) -> Result<Option<TransactionReceipt>> {

        println!("New deposit of amount {}",amount);
        let str_amount = amount.to_string();
        let number_value = DynSolValue::from(String::from(str_amount.clone()));
        let tx_hash = self.contract.function("mint", &[number_value])?.send().await?.watch().await?;
        println!("tx_hash: {tx_hash}");
        let receipt = self.provider.get_transaction_receipt(tx_hash).await?;

        Ok(receipt)
    }
}