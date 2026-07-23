# Multi-stage Dockerfile for metasearch2
FROM rust:slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    cmake \
    golang \
    perl \
    clang \
    libclang-dev \
    llvm \
    make \
    g++ \
    pkg-config \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY . .

ENV RUST_BACKTRACE=1
RUN cargo build --release

# Runtime stage using standard Debian slim
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/metasearch /usr/local/bin/metasearch

EXPOSE 28019

CMD ["/usr/local/bin/metasearch"]

