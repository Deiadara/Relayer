use crate::errors::RelayerError;
use alloy::primitives::Address;
use alloy::primitives::keccak256;
use alloy::rpc::types::eth::TransactionReceipt;
use eyre::Result;
use serde_json::Value;
use std::fs;
use std::str::FromStr;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::Registry;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;

const MINT_EVENT_SIG: &str = "Minted(address,string)";
#[derive(Debug)]
pub struct Deployments {
    pub deposit: Address,
    pub token: Address,
}

impl Deployments {
    pub fn from_file(path: &str) -> Result<Deployments, RelayerError> {
        let address_str = fs::read_to_string(path)?;
        let json: Value = serde_json::from_str(&address_str)?;
        let final_deployments = deployments_from_json(json)?;
        Ok(final_deployments)
    }
}

pub fn setup_logging() {
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    let level = LevelFilter::from_str(&level).unwrap_or(LevelFilter::DEBUG);

    let fmt_layer = fmt::layer().with_filter(level);

    let subscriber = Registry::default().with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber).expect("failed to set tracing subscriber");
}

pub fn deployments_from_json(json: Value) -> Result<Deployments, RelayerError> {
    let deposit_str = json
        .get("Deposit")
        .and_then(Value::as_str)
        .ok_or_else(|| RelayerError::Other("Deposit key missing".into()))?;
    let deposit = deposit_str.parse()?;

    let token_str = json
        .get("Token")
        .and_then(Value::as_str)
        .ok_or_else(|| RelayerError::Other("Token key missing".into()))?;
    let token = token_str.parse()?;

    Ok(Deployments { deposit, token })
}

pub fn verify_minted_log(receipt: &TransactionReceipt) -> Result<(), RelayerError> {
    let first_log = receipt.logs().first().ok_or(RelayerError::NoLogs)?;

    let topic = first_log.topics().first().ok_or(RelayerError::NoTopics)?;
    let expected_hash = keccak256(MINT_EVENT_SIG);

    if topic != &expected_hash {
        return Err(RelayerError::EventHashMismatch);
    }
    Ok(())
}

pub fn get_src_contract_addr(addr_path: &str) -> Result<Address, RelayerError> {
    let contract_address = Deployments::from_file(addr_path)?;
    Ok(contract_address.deposit)
}

pub fn get_dst_contract_addr(addr_path: &str) -> Result<Address, RelayerError> {
    let contract_address = Deployments::from_file(addr_path)?;
    Ok(contract_address.token)
}

mod tests {
    use super::*;
    use crate::errors::RelayerError;

    #[test]
    fn test_get_src_contract_addr() {
        let src_contract_address =
            get_src_contract_addr("../project_eth/data/deployments.json").unwrap();
        assert_eq!(
            src_contract_address.to_string(),
            "0x5FbDB2315678afecb367f032d93F642f64180aa3"
        );
    }

    #[test]
    fn test_get_src_contract_addr_file_not_found() {
        let err = get_src_contract_addr("this_file_does_not_exist").unwrap_err();
        assert!(matches!(err, RelayerError::FsStdIOError(_)));
    }

    #[test]
    fn test_get_dst_contract_addr() {
        let dst_contract_address =
            get_dst_contract_addr("../project_eth/data/deployments.json").unwrap();
        assert_eq!(
            dst_contract_address.to_string(),
            "0x5FbDB2315678afecb367f032d93F642f64180aa3"
        );
    }

    #[test]
    fn test_from_file_invalid_json() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        write!(f, "this is not json").unwrap();

        let err = Deployments::from_file(f.path().to_str().unwrap()).unwrap_err();
        assert!(matches!(err, RelayerError::SerdeError(_)));
    }

    #[test]
    fn test_get_dst_contract_addr_file_not_found() {
        let err = get_dst_contract_addr("this_file_does_not_exist").unwrap_err();
        assert!(matches!(err, RelayerError::FsStdIOError(_)));
    }

    #[test]
    fn test_deployments_from_json() {
        let json_str = r#"
        {
            "Deposit": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
            "Token":   "0x5FbDB2315678afecb367f032d93F642f64180aa3"
        }
        "#;
        let json: Value = serde_json::from_str(&json_str).unwrap();
        let res = deployments_from_json(json).unwrap();
        assert_eq!(
            res.deposit.to_string(),
            "0x5FbDB2315678afecb367f032d93F642f64180aa3"
        );
        assert_eq!(
            res.token.to_string(),
            "0x5FbDB2315678afecb367f032d93F642f64180aa3"
        );
    }

    #[test]
    fn test_deployments_from_json_err() {
        let json_str = r#"
        {
            "NotDeposit": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
            "NotToken":   "0x5FbDB2315678afecb367f032d93F642f64180aa3"
        }
        "#;
        let json: Value = serde_json::from_str(&json_str).unwrap();
        let err = deployments_from_json(json).unwrap_err();
        assert!(matches!(err, RelayerError::Other(_)));
    }
    #[test]
    fn test_deployments_from_json_err_2() {
        let json_str = r#"
        {
            "Deposit": "NotAnAddress",
            "Token":   123
        }
        "#;
        let json: Value = serde_json::from_str(&json_str).unwrap();
        let err = deployments_from_json(json).unwrap_err();
        assert!(matches!(err, RelayerError::FromHexError(_)));
    }
}
