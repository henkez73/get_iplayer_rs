# Multi-stage build for minimal and secure Docker image
FROM rust:1.99-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev

# Set working directory
WORKDIR /usr/src/get_iplayer_rs

# Copy source files
COPY . .

# Build the binary in release mode
RUN cargo build --release

# Add a safety check for the binary
RUN if [ ! -f target/release/get_iplayer_rs ]; then echo "Binary build failed"; exit 1; fi

# Final stage
FROM alpine:latest

RUN apk --update add \
    ffmpeg \
    openssl

RUN apk add atomicparsley --repository http://dl-3.alpinelinux.org/alpine/edge/testing/ --allow-untrusted && ln -s `which atomicparsley` /usr/local/bin/AtomicParsley

RUN mkdir -p /data/output /data/config

# Create non-root user
RUN adduser -D -u 1000 appuser

# Copy the binary from builder stage
COPY --from=builder /usr/src/get_iplayer_rs/target/release/get_iplayer_rs /usr/local/bin/get_iplayer_rs

# Switch to non-root user
USER appuser
WORKDIR /app

# Run the binary
ENTRYPOINT ["/usr/local/bin/get_iplayer_rs", "--ffmpeg", "/usr/bin/ffmpeg", "--profile-dir", "/data/config", "--output", "/data/output"]
