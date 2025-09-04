FROM rust:1.75 AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm src/main.rs

COPY src ./src
RUN touch src/main.rs
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/rabbitmq-consumer /app/rabbitmq-consumer
COPY config.yaml /app/config.yaml

RUN useradd -r -u 1001 appuser
RUN chown -R appuser:appuser /app
USER appuser

EXPOSE 8080

CMD ["./rabbitmq-consumer"]