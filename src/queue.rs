use rabbitmq_stream_client::error::StreamCreateError;
use rabbitmq_stream_client::types::{ByteCapacity, Message, ResponseCode};
use rabbitmq_stream_client::Environment;
use std::io::stdin;
use rabbitmq_stream_client::types::OffsetSpecification;
use tokio_stream::StreamExt;
use tokio::task;


use crate::errors::RelayerError;


pub async fn test_queue_send() -> Result<(),RelayerError>{

    let environment = Environment::builder().build().await.map_err(|e| RelayerError::QueueClientError(e))?;
    let stream = "hello-rust-stream";
    let create_response = environment
        .stream_creator()
        .max_length(ByteCapacity::GB(5))
        .create(stream)
        .await;

    if let Err(e) = create_response {
        if let StreamCreateError::Create { stream, status } = e {
            match status {
                // we can ignore this error because the stream already exists
                ResponseCode::StreamAlreadyExists => {}
                err => {
                    println!("Error creating stream: {:?} {:?}", stream, err);
                }
            }
        }
    }
    let producer = environment.producer().build(stream).await.map_err(|e| RelayerError::QueueProducerCreateError(e))?;
    producer
        .send_with_confirm(Message::builder().body("Hello, World!").build())
        .await
        .map_err(|e| RelayerError::QueueProducerPublishError(e))?;

    Ok(())
}

pub async fn test_queue_receive() -> Result<(),RelayerError>{
    use rabbitmq_stream_client::Environment;
    let environment = Environment::builder().build().await.map_err(|e| RelayerError::QueueClientError(e))?;
    let stream = "hello-rust-stream";
    let create_response = environment
        .stream_creator()
        .max_length(ByteCapacity::GB(5))
        .create(stream)
        .await;

    if let Err(e) = create_response {
        if let StreamCreateError::Create { stream, status } = e {
            match status {
                // we can ignore this error because the stream already exists
                ResponseCode::StreamAlreadyExists => {}
                err => {
                    println!("Error creating stream: {:?} {:?}", stream, err);
                }
            }
        }
    }

    let mut consumer = environment
        .consumer()
        .offset(OffsetSpecification::First)
        .build(stream)
        .await
        .unwrap();

    let handle = consumer.handle();
    task::spawn(async move {
        while let Some(delivery) = consumer.next().await {
            let d = delivery.unwrap();
            println!("Got message: {:#?} with offset: {}",
                     d.message().data().map(|data| String::from_utf8(data.to_vec()).unwrap()),
                     d.offset(),);
        }
    });


    println!("Press any key to close the consumer");
     _ = stdin().read_line(&mut "".to_string());


    handle.close().await?;
    println!("consumer closed successfully");
    Ok(())
}
