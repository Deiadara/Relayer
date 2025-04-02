use alloy::rpc::types::eth::TransactionReceipt;
use alloy::primitives::keccak256;

use crate::errors::RelayerError;
use eyre::Result;

pub fn verify_minted_log(receipt: &TransactionReceipt) -> Result<(), RelayerError> {
    let first_log = receipt.logs().get(0).ok_or(RelayerError::NoLogs)?;

    let topic = first_log.topics().get(0).ok_or(RelayerError::NoTopics)?;
    let expected_hash = keccak256("Minted(address,string)");

    if topic != &expected_hash {
        return Err(RelayerError::EventHashMismatch.into());
    }
    println!("Tokens minted succesfully!");
    Ok(())
}