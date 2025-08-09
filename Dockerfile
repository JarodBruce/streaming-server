# Stage 1: Build the application
FROM rust:1.82 as builder

WORKDIR /usr/src/app

COPY . .

RUN cargo install --path .

# Stage 2: Create the final image
FROM debian:bookworm-slim

# Install Python and pip
RUN apt-get update && apt-get install -y python3 python3-pip && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy application and assets
COPY --from=builder /usr/local/cargo/bin/streaming-server .
COPY static ./static
COPY av ./av

# Copy and install bot dependencies
COPY requirements.txt .
COPY viewer_bot.py .
RUN pip3 install --no-cache-dir -r requirements.txt

EXPOSE 8080

CMD ["./streaming-server"]