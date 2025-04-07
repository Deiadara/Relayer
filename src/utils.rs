use alloy::rpc::types::eth::TransactionReceipt;
use alloy::primitives::keccak256;

use crate::includer::Includer;
use crate::subscriber::Deposit;

use crate::errors::RelayerError;
use eyre::Result;

const MINT_EVENT_SIG : &str = "Minted(address,string)";

pub fn verify_minted_log(receipt: &TransactionReceipt) -> Result<(), RelayerError> {
    let first_log = receipt.logs().first().ok_or(RelayerError::NoLogs)?;

    let topic = first_log.topics().first().ok_or(RelayerError::NoTopics)?;
    let expected_hash = keccak256(MINT_EVENT_SIG);

    if topic != &expected_hash {
        return Err(RelayerError::EventHashMismatch);
    }
    println!("Tokens minted succesfully!");
    Ok(())
}

pub async fn log_to_deposit(dep : Deposit, incl : &Includer) -> Result<(), RelayerError>  {
    println!("Event emitted from sender : {:?}", dep.sender);
    match incl.mint(dep.amount).await {
        Ok(Some(receipt)) => {
            println!("Transaction successful! Receipt: {:?}", receipt);
            if !receipt.status() {
                println!("Transaction failed, status is 0");
                Err(RelayerError::EventHashMismatch)
            }
            else {
                verify_minted_log(&receipt)
            }
        }
        Ok(None) => {
            println!("Transaction sent, but no receipt found.");
            Err(RelayerError::EventHashMismatch)
        }
        Err(e) => {
            Err(RelayerError::Other(e.to_string()))
        }
    }
}