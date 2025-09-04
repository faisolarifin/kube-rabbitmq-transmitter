pub mod logging;
pub mod rabbitmq_consumer;

pub use logging::*;
pub use rabbitmq_consumer::{ProcessedMessage, RabbitMqConsumer, RabbitMqConsumerTrait};