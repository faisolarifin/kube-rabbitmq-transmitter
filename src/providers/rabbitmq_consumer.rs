use async_trait::async_trait;
use futures::StreamExt;
use lapin::{
    options::*, types::FieldTable, Channel, Connection, ConnectionProperties,
    Consumer, Queue,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::config::AmqpConfig;
use crate::models::QueueMessage;
use crate::utils::error::{AppError, Result};
use super::logging::StructuredLogger;

#[async_trait]
pub trait RabbitMqConsumerTrait {
    async fn start_consuming(&self) -> Result<mpsc::Receiver<ProcessedMessage>>;
    async fn connect(&self) -> Result<()>;
    async fn disconnect(&self) -> Result<()>;
    async fn graceful_shutdown(&self) -> Result<()>;
    async fn is_connected(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct ProcessedMessage {
    pub message: QueueMessage,
    pub delivery_tag: u64,
    pub request_id: String,
}

#[derive(Clone)]
pub struct RabbitMqConsumer {
    config: AmqpConfig,
    connection: Arc<tokio::sync::RwLock<Option<Connection>>>,
    channel: Arc<tokio::sync::RwLock<Option<Channel>>>,
    is_shutting_down: Arc<AtomicBool>,
    active_workers: Arc<AtomicUsize>,
}

impl RabbitMqConsumer {
    pub fn new(config: AmqpConfig) -> Self {
        Self {
            config,
            connection: Arc::new(tokio::sync::RwLock::new(None)),
            channel: Arc::new(tokio::sync::RwLock::new(None)),
            is_shutting_down: Arc::new(AtomicBool::new(false)),
            active_workers: Arc::new(AtomicUsize::new(0)),
        }
    }

    async fn create_connection(&self) -> Result<Connection> {
        let vhost = "/";
        let amqp_url = format!(
            "amqp://{}:{}@{}:{}/{}",
            self.config.username,
            self.config.password,
            self.config.host,
            self.config.port,
            vhost
        );

        StructuredLogger::log_info(
            "Connecting to RabbitMQ with concurrency settings",
            None,
            None,
            Some(serde_json::json!({
                "host": self.config.host,
                "port": self.config.port,
                "vhost": vhost,
                "max_workers": self.config.concurrent,
                "prefetch_count": self.config.prefetch_count
            })),
        );

        let connection = Connection::connect(&amqp_url, ConnectionProperties::default()).await?;

        StructuredLogger::log_info(
            "Connected to RabbitMQ successfully",
            None,
            None,
            None,
        );

        Ok(connection)
    }

    async fn setup_queue(&self, channel: &Channel) -> Result<Queue> {
        let queue = channel
            .queue_declare(
                &self.config.queue_name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        StructuredLogger::log_info(
            "Queue declared successfully",
            None,
            None,
            Some(serde_json::json!({
                "queue": self.config.queue_name
            })),
        );

        Ok(queue)
    }

    async fn create_consumer(&self, channel: &Channel) -> Result<Consumer> {
        // Set global prefetch
        let prefetch_count = self.config.prefetch_count;
        let prefetch_global = self.config.global;
        
        channel
            .basic_qos(
                prefetch_count,
                BasicQosOptions {
                    global: prefetch_global,
                    ..BasicQosOptions::default()
                }
            )
            .await?;

        let consumer = channel
            .basic_consume(
                &self.config.queue_name,
                "rabbitmq-consumer",
                BasicConsumeOptions {
                    no_ack: false, // Always manual ACK for safety
                    exclusive: false,
                    no_local: false,
                    nowait: false,
                },
                FieldTable::default(),
            )
            .await?;

        StructuredLogger::log_info(
            "Consumer created with concurrency settings",
            None,
            None,
            Some(serde_json::json!({
                "queue": self.config.queue_name,
                "prefetch_count": prefetch_count,
                "prefetch_global": prefetch_global,
                "max_workers": self.config.concurrent
            })),
        );

        Ok(consumer)
    }

    pub async fn ack_message(&self, delivery_tag: u64, request_id: &str) -> Result<()> {
        let channel_guard = self.channel.read().await;
        let channel = channel_guard
            .as_ref()
            .ok_or_else(|| AppError::message_processing("Channel not available"))?;

        if let Err(e) = channel
            .basic_ack(delivery_tag, BasicAckOptions::default())
            .await
        {
            StructuredLogger::log_error(
                &format!("Failed to ACK message: {}", e),
                Some(request_id),
                Some(request_id),
            );
            return Err(AppError::message_processing(format!("ACK failed: {}", e)));
        }

        StructuredLogger::log_info(
            "Message ACKed successfully",
            Some(request_id),
            Some(request_id),
            Some(serde_json::json!({
                "delivery_tag": delivery_tag
            })),
        );

        Ok(())
    }

    pub async fn nack_message(&self, delivery_tag: u64, requeue: bool, request_id: &str) -> Result<()> {
        let channel_guard = self.channel.read().await;
        let channel = channel_guard
            .as_ref()
            .ok_or_else(|| AppError::message_processing("Channel not available"))?;

        if let Err(e) = channel
            .basic_nack(
                delivery_tag,
                BasicNackOptions {
                    multiple: false,
                    requeue,
                },
            )
            .await
        {
            StructuredLogger::log_error(
                &format!("Failed to NACK message: {}", e),
                Some(request_id),
                Some(request_id),
            );
            return Err(AppError::message_processing(format!("NACK failed: {}", e)));
        }

        StructuredLogger::log_info(
            "Message NACKed successfully",
            Some(request_id),
            Some(request_id),
            Some(serde_json::json!({
                "delivery_tag": delivery_tag,
                "requeue": requeue
            })),
        );

        Ok(())
    }
}

#[async_trait]
impl RabbitMqConsumerTrait for RabbitMqConsumer {
    async fn connect(&self) -> Result<()> {
        let connection = self.create_connection().await?;
        let channel = connection.create_channel().await?;

        self.setup_queue(&channel).await?;

        {
            let mut conn_guard = self.connection.write().await;
            *conn_guard = Some(connection);
        }

        {
            let mut channel_guard = self.channel.write().await;
            *channel_guard = Some(channel);
        }

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.is_shutting_down.store(true, Ordering::SeqCst);

        // Wait for active workers to finish
        let mut retry_count = 0;
        while self.active_workers.load(Ordering::SeqCst) > 0 && retry_count < 50 {
            StructuredLogger::log_info(
                "Waiting for active workers to finish",
                None,
                None,
                Some(serde_json::json!({
                    "active_workers": self.active_workers.load(Ordering::SeqCst),
                    "retry_count": retry_count,
                    "max_retries": 50
                })),
            );
            sleep(Duration::from_millis(100)).await;
            retry_count += 1;
        }

        {
            let mut channel_guard = self.channel.write().await;
            if let Some(channel) = channel_guard.take() {
                if let Err(e) = channel.close(200, "Normal shutdown").await {
                    StructuredLogger::log_error(
                        &format!("Error closing channel: {}", e),
                        None,
                        None,
                    );
                }
            }
        }

        {
            let mut conn_guard = self.connection.write().await;
            if let Some(connection) = conn_guard.take() {
                if let Err(e) = connection.close(200, "Normal shutdown").await {
                    StructuredLogger::log_error(
                        &format!("Error closing connection: {}", e),
                        None,
                        None,
                    );
                }
            }
        }

        StructuredLogger::log_info(
            "Disconnected from RabbitMQ gracefully",
            None,
            None,
            Some(serde_json::json!({
                "final_active_workers": self.active_workers.load(Ordering::SeqCst)
            })),
        );

        Ok(())
    }

    async fn graceful_shutdown(&self) -> Result<()> {
        StructuredLogger::log_info(
            "Starting graceful shutdown",
            None,
            None,
            None,
        );

        self.disconnect().await
    }

    async fn is_connected(&self) -> bool {
        let conn_guard = self.connection.read().await;
        conn_guard.is_some() && !self.is_shutting_down.load(Ordering::SeqCst)
    }

    async fn start_consuming(&self) -> Result<mpsc::Receiver<ProcessedMessage>> {
        let channel_guard = self.channel.read().await;
        let channel = channel_guard
            .as_ref()
            .ok_or_else(|| AppError::message_processing("Channel not initialized"))?;

        let consumer = self.create_consumer(channel).await?;
        
        let max_workers = self.config.concurrent as usize;
        let semaphore = Arc::new(Semaphore::new(max_workers));
        let (tx, rx) = mpsc::channel(max_workers * 2);

        let is_shutting_down = Arc::clone(&self.is_shutting_down);
        let active_workers = Arc::clone(&self.active_workers);

        tokio::spawn(async move {
            let mut consumer_stream = consumer;
            
            StructuredLogger::log_info(
                "Consumer stream started with high concurrency",
                None,
                None,
                Some(serde_json::json!({
                    "max_workers": max_workers,
                    "semaphore_permits": semaphore.available_permits()
                })),
            );

            while let Some(delivery_result) = consumer_stream.next().await {
                if is_shutting_down.load(Ordering::SeqCst) {
                    StructuredLogger::log_info(
                        "Shutdown requested, stopping consumer stream",
                        None,
                        None,
                        None,
                    );
                    break;
                }

                match delivery_result {
                    Ok(delivery) => {
                        let tx_clone = tx.clone();
                        let semaphore_clone = Arc::clone(&semaphore);
                        let active_workers_clone = Arc::clone(&active_workers);
                        
                        // Spawn concurrent worker
                        tokio::spawn(async move {
                            let _permit = semaphore_clone.acquire().await.unwrap();
                            active_workers_clone.fetch_add(1, Ordering::SeqCst);
                            
                            let request_id = format!("req-{}", Uuid::new_v4());
                            let delivery_tag = delivery.delivery_tag;
                            
                            match serde_json::from_slice::<QueueMessage>(&delivery.data) {
                                Ok(message) => {
                                    StructuredLogger::log_info(
                                        "Message received and parsed successfully",
                                        Some(&request_id),
                                        Some(&request_id),
                                        Some(serde_json::json!({
                                            "message_size": delivery.data.len(),
                                            "delivery_tag": delivery_tag,
                                            "client_url": message.client_url
                                        })),
                                    );

                                    let processed_msg = ProcessedMessage {
                                        message,
                                        delivery_tag,
                                        request_id: request_id.clone(),
                                    };

                                    if tx_clone.send(processed_msg).await.is_err() {
                                        StructuredLogger::log_error(
                                            "Failed to send processed message to channel",
                                            Some(&request_id),
                                            Some(&request_id),
                                        );
                                    }
                                }
                                Err(e) => {
                                    StructuredLogger::log_error(
                                        &format!("Failed to parse message JSON: {}", e),
                                        Some(&request_id),
                                        Some(&request_id),
                                    );
                                    
                                    // Don't send malformed messages to processing
                                    // They will be handled by timeout and eventual NACK
                                }
                            }
                            
                            active_workers_clone.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Err(e) => {
                        StructuredLogger::log_error(
                            &format!("Consumer error: {}", e),
                            None,
                            None,
                        );
                        
                        // In case of consumer error, wait before retrying
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }

            StructuredLogger::log_info(
                "Consumer stream ended gracefully",
                None,
                None,
                None,
            );
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> AmqpConfig {
        AmqpConfig {
            scheme: "amqp".to_string(),
            host: "localhost".to_string(),
            port: 5672,
            username: "guest".to_string(),
            password: "guest".to_string(),
            concurrent: 4,
            prefetch_count: 10,
            prefetch_size: 0,
            global: true,
            queue_name: "test_queue".to_string(),
        }
    }

    #[test]
    fn test_create_consumer_with_concurrency() {
        let config = create_test_config();
        let consumer = RabbitMqConsumer::new(config.clone());
        
        assert_eq!(consumer.config.queue_name, "test_queue");
        assert_eq!(consumer.config.host, "localhost");
        assert_eq!(consumer.config.port, 5672);
        assert_eq!(consumer.config.concurrent, 4);
        assert_eq!(consumer.config.global, true);
        assert!(!consumer.is_shutting_down.load(Ordering::SeqCst));
        assert_eq!(consumer.active_workers.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let config = create_test_config();
        let consumer = RabbitMqConsumer::new(config);
        
        assert!(!consumer.is_shutting_down.load(Ordering::SeqCst));
        
        // Test graceful shutdown
        let result = consumer.graceful_shutdown().await;
        assert!(result.is_ok());
        assert!(consumer.is_shutting_down.load(Ordering::SeqCst));
    }
}