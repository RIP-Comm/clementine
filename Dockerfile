FROM rust:1.63 as builder
WORKDIR /usr/src/clementine
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
COPY src ./src
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update && apt-get install -y libfreetype6 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/clementine /usr/local/bin/clementine
ENTRYPOINT ["clementine"]