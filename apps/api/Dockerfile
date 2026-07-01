FROM rust:1-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml ./
COPY Cargo.lock ./
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY apps/api/assets apps/api/assets
COPY apps/api/src apps/api/src
COPY apps/api/tests apps/api/tests
COPY apps/api/migrations apps/api/migrations
COPY packages/api-contracts packages/api-contracts

RUN cargo build --release -p apmtrack-api

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/apmtrack-api /usr/local/bin/apmtrack-api
COPY apps/api/migrations /app/migrations

EXPOSE 8080

ENTRYPOINT ["apmtrack-api"]
