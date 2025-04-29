use crate::{
    errors::RelayerError, queue::QueueTrait, subscriber::Deposit, utils::verify_minted_log,
};
use alloy::{
    contract::{ContractInstance, Interface},
    dyn_abi::DynSolValue,
    json_abi::JsonAbi,
    network::{Ethereum, EthereumWallet},
    primitives::Address,
    providers::{
        Identity, Provider, ProviderBuilder, RootProvider,
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
    },
    rpc::types::TransactionReceipt,
    signers::local::PrivateKeySigner,
    transports::http::reqwest::Url,
};
use eyre::Result;
use futures_lite::StreamExt;
use lapin::{
    Consumer,
    message::Delivery,
    options::{BasicAckOptions, BasicNackOptions},
};
use serde_json::Value;
use std::{env, fs, thread, time};
use tracing::{debug, error, info, warn};
type ProviderType = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider,
>;
type ContractType = ContractInstance<ProviderType, Ethereum>;

pub struct Includer<C: QueueTrait> {
    pub provider: ProviderType,
    pub contract: ContractType,
    pub queue_connection: C,
}

const TOKEN_DATA_PATH: &str = "../project_eth/data/TokenData.json";

impl<C: QueueTrait> Includer<C> {
    pub async fn new(
        dst_rpc_url: &Url,
        contract_address: Address,
        queue_connection: C,
    ) -> Result<Self> {
        let data_str = fs::read_to_string(TOKEN_DATA_PATH)?;
        let data_json: Value = serde_json::from_str(&data_str)?;
        let abi: JsonAbi = serde_json::from_str(&data_json["abi"].to_string())?;
        let pk_str = env::var("PRIVATE_KEY").expect("Private key not set");
        let pk: PrivateKeySigner = pk_str.parse()?;
        let wallet = EthereumWallet::from(pk);
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .on_http(dst_rpc_url.clone());
        let contract: ContractType = ContractInstance::new(
            contract_address,
            provider.clone(),
            Interface::new(abi.clone()),
        );
        Ok(Self {
            provider,
            contract,
            queue_connection,
        })
    }

    pub async fn mint(&self, amount: i32) -> Result<Option<TransactionReceipt>> {
        info!("New deposit of amount {}", amount);
        let str_amount = amount.to_string();
        let number_value = DynSolValue::from(str_amount.clone());
        let tx_hash = self
            .contract
            .function("mint", &[number_value])?
            .send()
            .await?
            .watch()
            .await?;
        debug!("tx_hash: {tx_hash}");
        let receipt = self.provider.get_transaction_receipt(tx_hash).await?;

        Ok(receipt)
    }

    pub async fn consume(
        &self,
        consumer: &mut Consumer,
    ) -> Result<(Deposit, Delivery), RelayerError> {
        info!("Waiting for a deposit message...");
        match consumer.next().await {
            None => {
                warn!("Stream Ended");
                return Err(RelayerError::Other(
                    "Consumer stream ended unexpectedly".into(),
                ));
            }
            Some(Err(e)) => {
                return Err(RelayerError::AmqpError(e));
            }
            Some(Ok(delivery)) => match serde_json::from_slice::<Deposit>(&delivery.data) {
                Ok(deposit) => {
                    debug!(
                        "Got deposit from {:?}, amount {}",
                        deposit.sender, deposit.amount
                    );
                    return Ok((deposit, delivery));
                }
                Err(_) => {
                    return Err(RelayerError::Other(String::from(
                        "Failed to parse Deposit, skipping...",
                    )));
                }
            },
        }
    }

    pub async fn run(&mut self) {
        let mut consumer = self.queue_connection.consumer().await.unwrap();
        debug!("Includer is alive.");
        loop {
            let res = self.process_deposit(&mut consumer).await;
            match res {
                Ok(_) => {
                    info!("Successfully processed Deposit");
                }
                Err(e) => {
                    error!("Error : {:?}", e);
                }
            }
            let two_sec = time::Duration::from_millis(2000);
            thread::sleep(two_sec);
        }
    }

    pub async fn process_deposit(&mut self, consumer: &mut Consumer) -> Result<(), RelayerError> {
        match self.consume(consumer).await {
            Ok(dep) => {
                debug!("Successfully received");
                match self.mint(dep.0.amount).await {
                    Ok(Some(receipt)) => {
                        debug!("Transaction successful! Receipt: {:?}", receipt);
                        if !receipt.status() {
                            warn!("Transaction failed, status is 0");
                        } else {
                            match verify_minted_log(&receipt) {
                                Ok(_) => {
                                    info!("Tokens minted succesfully!");
                                    Self::ack_deposit(dep.1).await?;

                                }
                                Err(e) => {
                                    error!("Couldn't verify minted log : {}", e);
                                    Self::nack_deposit(dep.1).await?;
                                    return Err(RelayerError::Other(e.to_string()))
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        warn!("Transaction sent, but no receipt found.");
                        Self::nack_deposit(dep.1).await?;
                    }
                    Err(e) => {
                        error!("Error minting : {:?}", e);
                        Self::nack_deposit(dep.1).await?;
                        return Err(RelayerError::Other(e.to_string()))
                    }
                }
            }
            Err(e) => {
                error!("Error processing receive: {:?}", e);
            }
        }
        Ok(())
    }

    pub async fn nack_deposit(delivery : Delivery) -> Result<(), RelayerError>{
        delivery
            .nack(BasicNackOptions {
                multiple: false,
                requeue: false,
            })
            .await
            .map_err(RelayerError::AmqpError)
    }

    pub async fn ack_deposit(delivery : Delivery) -> Result<(), RelayerError>{
        delivery
            .ack(BasicAckOptions::default())
            .await
            .map_err(RelayerError::AmqpError)
    }
}
