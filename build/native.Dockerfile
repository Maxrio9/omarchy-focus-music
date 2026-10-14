# Pin the compiler, host libraries and build tools, including the platform.
FROM rust:1.99.0-slim@sha256:458b145eb2406e832adc9c9536d487c5d264401a4ce63adbeced3fe91292616c
RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
