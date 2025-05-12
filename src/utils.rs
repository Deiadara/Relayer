use crate::errors::RelayerError;
use crate::subscriber::Deposit;
use alloy::primitives::Address;
use alloy::primitives::keccak256;
use alloy::rpc::types::Log;
use alloy::rpc::types::eth::TransactionReceipt;
use alloy_dyn_abi::DynSolType;
use alloy_dyn_abi::DynSolValue;
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

pub async fn push_deposits(
    logs: Vec<Log>,
    mut deposits: Vec<Deposit>,
) -> Result<Vec<Deposit>, RelayerError> {
    for log in logs {
        println!("Transfer event: {log:?}");
        let topics = log.topics();
        let raw_topic = topics.get(1).expect("Expected at least 2 topics");

        let topic_bytes = raw_topic.as_ref();

        let decoded = DynSolType::Address.abi_decode(topic_bytes)?;
        let sender = match decoded {
            DynSolValue::Address(addr) => addr,
            _ => return Err(RelayerError::NoAddress),
        };

        let raw_data = log.data().data.clone();

        let decoded = DynSolType::String.abi_decode(&raw_data.clone())?;

        let amount_str = match decoded {
            DynSolValue::String(s) => s,
            _ => return Err(RelayerError::NotString),
        };

        let amount = amount_str.parse::<i32>().unwrap();

        deposits.push(Deposit { sender, amount });
    }
    Ok(deposits)
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
    use std::default;

    use alloy::primitives::LogData;

    use super::*;
    use crate::errors::RelayerError;
    use alloy::primitives::{Address, B256, Bytes, Log as RawLog};
    use alloy::rpc::types::Log as RpcLog;
    use alloy_dyn_abi::{DynSolType, DynSolValue};

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

    #[tokio::test]
    async fn test_push_deposits() {
        let sender: Address = "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap();
        let sender2: Address = "0x1111111111111111111111111111111111111112"
            .parse()
            .unwrap();

        let topic0 = B256::from(keccak256("Deposited(address,string)"));
        let topic1_bytes = DynSolValue::Address(sender).abi_encode();
        let topic1_bytes_2 = DynSolValue::Address(sender2).abi_encode();
        let topic1_2 = B256::from_slice(&topic1_bytes_2);

        let topic1 = B256::from_slice(&topic1_bytes);
        let raw_data = DynSolValue::String("42".to_string()).abi_encode();
        let data_bytes: Bytes = raw_data.into();

        let log_data = LogData::new_unchecked(vec![topic0, topic1], data_bytes.clone());
        let log_data_2 = LogData::new_unchecked(vec![topic0, topic1_2], data_bytes.clone());
        let primitive: RawLog<LogData> = RawLog {
            address: Address::default(),
            data: log_data,
        };

        let primitive_2: RawLog<LogData> = RawLog {
            address: Address::default(),
            data: log_data_2,
        };

        let rpc_log: RpcLog<LogData> = RpcLog {
            inner: primitive,
            block_hash: None,
            block_number: None,
            block_timestamp: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        };

        let rpc_log_2: RpcLog<LogData> = RpcLog {
            inner: primitive_2,
            block_hash: None,
            block_number: None,
            block_timestamp: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        };

        let deposits = push_deposits(vec![rpc_log, rpc_log_2], Vec::new())
            .await
            .unwrap();

        assert_eq!(deposits.len(), 2);
        assert_eq!(deposits[0].sender, sender);
        assert_eq!(deposits[1].sender, sender2);
        assert_eq!(deposits[0].amount, 42);
    }

    #[tokio::test]
    #[should_panic(expected = "Expected at least 2 topics")]
    async fn test_push_deposits_missing_topic1_panics() {
        // log with only topic0
        let topic0 = B256::from(keccak256("Deposited(address,string)"));
        let data_bytes = Bytes::from_static(&[0u8; 0]); // empty, won't get that far
        let log_data = LogData::new_unchecked(vec![topic0], data_bytes);
        let primitive = RawLog {
            address: Address::default(),
            data: log_data,
        };
        let rpc_log = RpcLog {
            inner: primitive,
            block_hash: None,
            block_number: None,
            block_timestamp: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        };
        // should panic here
        let _ = push_deposits(vec![rpc_log], Vec::new()).await;
    }

    #[tokio::test]
    async fn test_push_deposits_bad_data_returns_abi_error() {
        let sender: Address = Address::default();
        let topic0 = B256::from(keccak256("Deposited(address,string)"));
        let topic1_bytes = DynSolValue::Address(sender).abi_encode();
        let topic1 = B256::from_slice(&topic1_bytes);

        // Invalid string data
        let bad_data = Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef]);
        let log_data = LogData::new_unchecked(vec![topic0, topic1], bad_data);
        let primitive = RawLog {
            address: Address::default(),
            data: log_data,
        };
        let rpc_log = RpcLog {
            inner: primitive,
            block_hash: None,
            block_number: None,
            block_timestamp: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        };

        // Should produce ABIerror
        let err = push_deposits(vec![rpc_log], Vec::new()).await.unwrap_err();
        assert!(matches!(err, RelayerError::AbiError(_)));
    }
}
