use std::{fs,env};
use alloy::{
    json_abi::JsonAbi,
    transports::http::reqwest::Url,
    contract::{ContractInstance, Interface}, 
    dyn_abi::DynSolValue, 
    network::{EthereumWallet,Ethereum}, 
    primitives::Address, 
    providers::{Provider, ProviderBuilder, fillers::{BlobGasFiller,GasFiller,FillProvider, JoinFill, NonceFiller, ChainIdFiller, WalletFiller}, Identity, RootProvider}, 
    rpc::types::TransactionReceipt,
    signers::local::PrivateKeySigner
};
use crate::queue::QueueConnectionConsumer;
use eyre::Result;
use serde_json::Value;
type ProviderType = FillProvider<JoinFill<JoinFill<Identity, JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller,ChainIdFiller>>>>, WalletFiller<EthereumWallet>>, RootProvider>;
type ContractType = ContractInstance<ProviderType, Ethereum>;

pub struct Includer {
    pub provider : ProviderType,
    pub contract : ContractType,
    pub queue_connection : QueueConnectionConsumer
}

const TOKEN_DATA_PATH : &str = "../project_eth/data/TokenData.json";

impl Includer {
    pub async fn new(dst_rpc_url: &Url ,contract_address : Address, queue_connection : QueueConnectionConsumer) -> Result<Self> {
        
        let data_str = fs::read_to_string(TOKEN_DATA_PATH)?;
        let data_json: Value = serde_json::from_str(&data_str)?;
        let abi : JsonAbi = serde_json::from_str(&data_json["abi"].to_string())?;
        let pk_str= env::var("PRIVATE_KEY").expect("Private key not set");
        let pk: PrivateKeySigner = pk_str.parse()?;
        let wallet = EthereumWallet::from(pk);
        let provider = ProviderBuilder::new().wallet(wallet).on_http(dst_rpc_url.clone());
        let contract: ContractType = ContractInstance::new(contract_address, provider.clone(), Interface::new(abi.clone()));
        Ok(Self {
            provider,
            contract,
            queue_connection
        })
    }

    pub async fn mint(&self, amount : i32) -> Result<Option<TransactionReceipt>> {

        println!("New deposit of amount {}",amount);
        let str_amount = amount.to_string();
        let number_value = DynSolValue::from(str_amount.clone());
        let tx_hash = self.contract.function("mint", &[number_value])?.send().await?.watch().await?;
        println!("tx_hash: {tx_hash}");
        let receipt = self.provider.get_transaction_receipt(tx_hash).await?;

        Ok(receipt)
    }
}