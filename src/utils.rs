use alloy::primitives::keccak256;
use alloy::rpc::types::eth::TransactionReceipt;
use alloy::primitives::Address;
use serde_json::Value;
use std::fs;
use crate::errors::RelayerError;
use eyre::Result;

const MINT_EVENT_SIG: &str = "Minted(address,string)";
const ADDRESS_PATH: &str = "../project_eth/data/deployments.json";

pub fn verify_minted_log(receipt: &TransactionReceipt) -> Result<(), RelayerError> {
    let first_log = receipt.logs().first().ok_or(RelayerError::NoLogs)?;

    let topic = first_log.topics().first().ok_or(RelayerError::NoTopics)?;
    let expected_hash = keccak256(MINT_EVENT_SIG);

    if topic != &expected_hash {
        return Err(RelayerError::EventHashMismatch);
    }
    Ok(())
}

pub fn get_src_contract_addr() -> Result<Address,RelayerError> {
    let address_str = fs::read_to_string(ADDRESS_PATH)?;
    let json: Value = serde_json::from_str(&address_str)?;
    let contract_addr = json["Deposit"].as_str().expect("Deposit address not found");
    let contract_address: Address = contract_addr.parse()?; 
    Ok(contract_address)
}

pub fn get_dst_contract_addr() -> Result<Address, RelayerError> {
    let address_str = fs::read_to_string(ADDRESS_PATH)?;
    let json: Value = serde_json::from_str(&address_str)?;
    let contract_addr = json["Token"].as_str().expect("Mint address not found");
    let contract_address: Address = contract_addr.parse()?; 
    Ok(contract_address)
}

pub fn get_dst_contract_addr_with_path(addr_path : &str) -> Result<Address, RelayerError> {
    let address_str = fs::read_to_string(addr_path)?;
    let json: Value = serde_json::from_str(&address_str)?;
    let contract_addr = json["Token"].as_str().expect("Mint address not found");
    let contract_address: Address = contract_addr.parse()?; 
    Ok(contract_address)
}

pub fn get_dst_contract_addr_with_wrong_parse() -> Result<Address, RelayerError> {
    let address_str = fs::read_to_string(ADDRESS_PATH)?;
    let json: Value = serde_json::from_str(&address_str)?;
    let contract_addr = json["Token"].as_str().expect("Mint address not found");
    let res = String::from(contract_addr) + "test";
    let contract_address: Address = res.parse()?; 
    Ok(contract_address)
}

mod tests {
    use super::*;
    #[test]
    fn test_get_src_contract_addr() {
        let src_contract_address = get_src_contract_addr().unwrap();
        assert_eq!(src_contract_address.to_string(), "0x5FbDB2315678afecb367f032d93F642f64180aa3");
    }

    #[test]
    fn test_get_dst_contract_addr() {
        let dst_contract_address = get_dst_contract_addr().unwrap();
        assert_eq!(dst_contract_address.to_string(), "0x5FbDB2315678afecb367f032d93F642f64180aa3");
    }

    #[test]
    fn test_wrong_addr_path() {
        let result = get_dst_contract_addr_with_path("wrong_path");
        assert!(!result.is_ok());
    }

    #[test]
    fn test_wrong_addr_parse() {
        let result = get_dst_contract_addr_with_wrong_parse();
        assert!(!result.is_ok());
    }

}