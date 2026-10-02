FROM rust:1.75-slim as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY tools/ ./tools/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libasound2-dev libudev-dev libx11-dev libxcb1-dev libxcomposite-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/screenbuddy /usr/local/bin/
CMD ["screenbuddy"]
