# Stage 1: Build the application
FROM rust:1.82 as builder

WORKDIR /usr/src/app

# Copy dependencies and build them to leverage Docker layer caching
COPY Cargo.toml Cargo.lock ./
# Create a dummy main.rs to build only dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

# Copy the actual source code and build the application
COPY src ./src
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