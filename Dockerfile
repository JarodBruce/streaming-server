# Stage 1: Build the application
FROM rust:1.82 as builder

WORKDIR /usr/src/app

# Copy dependencies and build them
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
RUN cargo build --release

EXPOSE 8080

CMD ["./streaming-server"]
