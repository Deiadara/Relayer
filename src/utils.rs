use alloy::primitives::keccak256;
use alloy::rpc::types::eth::TransactionReceipt;

use crate::errors::RelayerError;
use eyre::Result;

const MINT_EVENT_SIG: &str = "Minted(address,string)";

pub fn verify_minted_log(receipt: &TransactionReceipt) -> Result<(), RelayerError> {
    let first_log = receipt.logs().first().ok_or(RelayerError::NoLogs)?;

    let topic = first_log.topics().first().ok_or(RelayerError::NoTopics)?;
    let expected_hash = keccak256(MINT_EVENT_SIG);

    if topic != &expected_hash {
        return Err(RelayerError::EventHashMismatch);
    }
    Ok(())
}
