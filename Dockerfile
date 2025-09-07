# Simple Rust build
FROM rust:1.85 AS builder

WORKDIR /app

# Copy everything
COPY . .

# Build the application
RUN cargo build --release --bin ck

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/ck /usr/local/bin/ck

# Create non-root user
RUN useradd -r -s /bin/false ck
USER ck

ENTRYPOINT ["ck"]
CMD ["--help"]