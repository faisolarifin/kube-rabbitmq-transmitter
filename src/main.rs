use anyhow::Result;
use std::sync::Arc;
use tokio::signal;
use tracing::info;

use rabbitmq_consumer::{
    config::AppConfig,
    services::{AuthService, HttpClientService, MessageProcessor, MessageProcessorTrait},
    providers::{RabbitMqConsumer, RabbitMqConsumerTrait, StructuredLogger},
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load()?;
    
    StructuredLogger::init("info", Some(config.logger.clone()))?;
    
    info!("Starting RabbitMQ Consumer Application");
    
    let http_client = reqwest::Client::new();
    let auth_service = Arc::new(AuthService::new(config.auth.clone(), http_client.clone()));
    let http_client_service = HttpClientService::new(
        config.http.clone(),
        auth_service,
    );
    
    let rabbitmq_consumer = RabbitMqConsumer::new(config.amqp.clone());
    let message_processor = MessageProcessor::new(Arc::new(http_client_service));

    if let Err(e) = rabbitmq_consumer.connect().await {
        StructuredLogger::log_error(
            &format!("Failed to connect to RabbitMQ: {}", e),
            None,
            None,
        );
        return Err(e.into());
    }

    let mut receiver = match rabbitmq_consumer.start_consuming().await {
        Ok(receiver) => receiver,
        Err(e) => {
            StructuredLogger::log_error(
                &format!("Failed to start consuming: {}", e),
                None,
                None,
            );
            return Err(e.into());
        }
    };

    let consumer_handle = tokio::spawn({
        let processor = message_processor.clone();
        async move {
            StructuredLogger::log_info(
                "Starting concurrent message processing",
                None,
                None,
                None,
            );

            while let Some(processed_msg) = receiver.recv().await {
                let processor_clone = processor.clone();
                let request_id = processed_msg.request_id.clone();
                
                // Process each message concurrently
                tokio::spawn(async move {
                    if let Err(e) = processor_clone
                        .process_single_message(processed_msg.message, &request_id)
                        .await
                    {
                        StructuredLogger::log_error(
                            &format!("Failed to process message: {}", e),
                            Some(&request_id),
                            Some(&request_id),
                        );
                    }
                });
            }

            StructuredLogger::log_info(
                "Message processing loop ended",
                None,
                None,
                None,
            );
        }
    });

    StructuredLogger::log_info(
        "RabbitMQ Consumer Application started successfully",
        None,
        None,
        None,
    );

    match signal::ctrl_c().await {
        Ok(()) => {
            StructuredLogger::log_info(
                "Shutdown signal received, initiating graceful shutdown",
                None,
                None,
                None,
            );
        }
        Err(e) => {
            StructuredLogger::log_error(
                &format!("Failed to listen for shutdown signal: {}", e),
                None,
                None,
            );
        }
    }

    // Graceful shutdown sequence
    StructuredLogger::log_info(
        "Starting graceful shutdown sequence",
        None,
        None,
        None,
    );

    // 1. Stop accepting new messages
    if let Err(e) = rabbitmq_consumer.graceful_shutdown().await {
        StructuredLogger::log_error(
            &format!("Error during graceful RabbitMQ shutdown: {}", e),
            None,
            None,
        );
    }

    // 2. Wait for processing to complete (with timeout)
    let shutdown_timeout = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        consumer_handle,
    );

    match shutdown_timeout.await {
        Ok(_) => {
            StructuredLogger::log_info(
                "Message processing completed gracefully",
                None,
                None,
                None,
            );
        }
        Err(_) => {
            StructuredLogger::log_warning(
                "Shutdown timeout reached, forcing termination",
                None,
                None,
            );
        }
    }

    StructuredLogger::log_info(
        "RabbitMQ Consumer Application stopped",
        None,
        None,
        None,
    );

    Ok(())
}
