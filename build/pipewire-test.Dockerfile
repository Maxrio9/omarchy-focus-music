FROM rust:1.99.0-slim@sha256:458b145eb2406e832adc9c9536d487c5d264401a4ce63adbeced3fe91292616c
RUN apt-get update && apt-get install -y --no-install-recommends pipewire-bin wireplumber dbus \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --uid 1000 --create-home focus-test
