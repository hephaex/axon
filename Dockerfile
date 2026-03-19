# Axon - LLM-to-LLM Communication Framework
# Multi-stage build for minimal image size

# ============================================
# Stage 1: Build
# ============================================
FROM rust:1.82-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

# Create a new empty shell project
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy source to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > src/lib.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build the actual binary
RUN touch src/main.rs src/lib.rs && \
    cargo build --release --locked

# ============================================
# Stage 2: Runtime
# ============================================
FROM alpine:3.20 AS runtime

# Install runtime dependencies
RUN apk add --no-cache ca-certificates tzdata

# Create non-root user
RUN addgroup -g 1000 axon && \
    adduser -u 1000 -G axon -h /home/axon -D axon

# Copy binary from builder
COPY --from=builder /app/target/release/axon /usr/local/bin/axon

# Create config directory
RUN mkdir -p /home/axon/.axon && \
    chown -R axon:axon /home/axon

# Switch to non-root user
USER axon
WORKDIR /home/axon

# Expose default port
EXPOSE 8090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget -q --spider http://localhost:8090/health || exit 1

# Default command: start server
ENTRYPOINT ["axon"]
CMD ["serve", "--host", "0.0.0.0", "--port", "8090"]
