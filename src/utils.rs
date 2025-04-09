use alloy::rpc::types::eth::TransactionReceipt;
use alloy::primitives::keccak256;
use rabbitmq_stream_client::types::Message;

use crate::queue::{QueueConnectionConsumer, QueueConnectionWriter};
use crate::subscriber::Deposit;
use redis::AsyncCommands;

use futures::StreamExt;

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
    Ok(())
}

pub async fn log_to_deposit(dep: Deposit, queue_connection: &QueueConnectionWriter) -> Result<(), RelayerError> {
    println!("Event emitted from sender: {:?}", dep.sender);

    let serialized_deposit = serde_json::to_vec(&dep)
        .map_err( RelayerError::SerdeError)?;
    
    queue_connection.producer
        .send_with_confirm(Message::builder().body(serialized_deposit).build())
        .await
        .map_err( RelayerError::QueueProducerPublishError)?;
    
    println!("Wrote in queue successfully!");
    Ok(())
}


pub async fn log_to_mint(queue_connection: &mut QueueConnectionConsumer) -> Result<Deposit, RelayerError> {
    println!("Waiting for a deposit message...");

    while let Some(delivery_result) = queue_connection.consumer.next().await {
        match delivery_result {
            Ok(delivery) => {
                if let Some(data_bytes) = delivery.message().data() {
                    match serde_json::from_slice::<Deposit>(data_bytes) {
                        Ok(deposit) => {
                            println!("Got deposit: {:?} at offset {}", deposit, delivery.offset());
                            let _response: String = queue_connection.dbcon
                                .set("last_offset", delivery.offset() + 1)
                                .await
                                .map_err(|e| RelayerError::RedisError(e.to_string()))?;
                            return Ok(deposit);
                        }
                        Err(_e) => {
                            continue;
                        }
                    }
                } else {
                    eprintln!("No data in message");
                }
            }
            Err(e) => {
                eprintln!("Delivery error: {:?}", e);
            }
        }
    }

    Err(RelayerError::Other("Consumer stream ended unexpectedly".into()))
}

// pub async fn log_to_mint(queue_connection: &mut QueueConnectionConsumer) -> Result<Deposit, RelayerError> {
//     println!("Waiting for a deposit message...");

//     loop {

//         println!("Starting from offset: {}", queue_connection.offset);

//         if let Some(delivery_result) = queue_connection.consumer.next().await {
//             match delivery_result {
//                 Ok(delivery) => {
//                     if let Some(data_bytes) = delivery.message().data() {
//                         match serde_json::from_slice::<Deposit>(data_bytes) {
//                             Ok(deposit) => {
//                                 println!("Got deposit: {:?} at offset {}", deposit, delivery.offset());
//                                 let _response : String = queue_connection.dbcon
//                                     .set("last_offset", delivery.offset() + 1)
//                                     .await
//                                     .map_err(|e| RelayerError::RedisError(e.to_string()))?;
//                                 return Ok(deposit);
//                             }
//                             Err(e) => {
//                                 eprintln!("Failed to parse Deposit: {:?}", e);
//                                 continue; // skip bad entries
//                             }
//                         }
//                     } else {
//                         // Message had no data; print a warning and wait for the next one.
//                         eprintln!("Received message with no data; skipping.");
//                     }
//                 }
//                 Err(e) => {
//                     // Log delivery errors and continue waiting.
//                     eprintln!("Delivery error: {:?}", e);
//                 }
//             }
//         } else {
//             // If the consumer stream ends unexpectedly, return an error.
//             return Err(RelayerError::Other("Consumer stream ended unexpectedly".into()));
//         }
//     }
// }

// match incl.mint(dep.amount).await {
    //     Ok(Some(receipt)) => {
    //         println!("Transaction successful! Receipt: {:?}", receipt);
    //         if !receipt.status() {
    //             println!("Transaction failed, status is 0");
    //             Err(RelayerError::EventHashMismatch)
    //         }
    //         else {
    //             verify_minted_log(&receipt)
    //         }
    //     }
    //     Ok(None) => {
    //         println!("Transaction sent, but no receipt found.");
    //         Err(RelayerError::EventHashMismatch)
    //     }
    //     Err(e) => {
    //         Err(RelayerError::Other(e.to_string()))
    //     }
    // }