# Stage 1: Build the application
FROM rust:1.82 as builder

WORKDIR /usr/src/app

# Copy dependencies and build them
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
RUN cargo build --release

# Stage 2: Create the final image
FROM debian:bookworm-slim

# Install Python and pip
RUN apt-get update && apt-get install -y python3 python3-pip && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy application and assets
COPY --from=builder /usr/src/app/target/release/streaming-server .
COPY static ./static
COPY av ./av

EXPOSE 8080

CMD ["./streaming-server"]
