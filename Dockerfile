# syntax=docker/dockerfile:1
FROM rust:1.85-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN useradd --system --uid 10001 --create-home agentloom \
    && apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/agent-loom /usr/local/bin/agent-loom
USER agentloom
WORKDIR /var/lib/agent-loom
ENTRYPOINT ["/usr/local/bin/agent-loom"]
