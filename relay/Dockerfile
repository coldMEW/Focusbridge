FROM rust:1.75-slim-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/focusbridge-relay /app/relay
COPY config.toml /app/config.toml
EXPOSE 8443
CMD ["/app/relay", "--config", "/app/config.toml"]
