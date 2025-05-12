use alloy::transports::http::reqwest::Url;
use relayer::includer;
use relayer::queue::{QueueTrait, get_queue_connection};
use relayer::subscriber::Deposit;
use relayer::utils::get_dst_contract_addr;

#[tokio::test]
async fn test_publish_and_consume() {
    const ADDRESS_PATH: &str = "../project_eth/data/deployments.json";
    unsafe {
        std::env::set_var(
            "PRIVATE_KEY",
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        );
    }
    let mut con = get_queue_connection(true).await.unwrap();
    let test_deposit = Deposit {
        sender: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
            .parse()
            .unwrap(),
        amount: 42,
    };
    let test_item = serde_json::to_vec(&test_deposit).unwrap();
    let resp = con.publish(&test_item).await;
    assert!(resp.is_ok());
    let mut consumer = con.consumer().await.unwrap();
    let dst_rpc = "http://localhost:8546";
    let rpc_url_dst: Url = dst_rpc.parse().unwrap();
    let dst_contract_address = get_dst_contract_addr(ADDRESS_PATH).unwrap();
    let incl_res = includer::Includer::new(&rpc_url_dst, dst_contract_address, con.clone()).await;
    assert!(incl_res.is_ok());
    let incl = incl_res.unwrap();
    let res = incl.consume(&mut consumer).await;
    assert!(res.is_ok());
    let tuple = res.unwrap();
    let (received_deposit, delivery) = tuple;
    assert_eq!(received_deposit, test_deposit);
    let res = incl.ack_deposit(delivery).await;
    assert!(res.is_ok());
}
