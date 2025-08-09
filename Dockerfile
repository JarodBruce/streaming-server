# Stage 1: Build the application using the official Rust image
FROM rust:1.82 AS builder

WORKDIR /usr/src/app

# Copy the dependency definitions
COPY Cargo.toml Cargo.lock ./

# Create a dummy src/main.rs to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

# Copy the actual source code and build
COPY src ./src
COPY static ./static
RUN touch src/main.rs && cargo build --release

# Stage 2: Create a small, final image
FROM debian:bookworm-slim

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/streaming-server /usr/local/bin/streaming-server

# Copy static files and video assets
WORKDIR /usr/src/app
COPY static ./static
COPY av ./av

EXPOSE 8080

# Set the command to run the application
CMD ["streaming-server"]