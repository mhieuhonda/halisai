# Build
FROM rust:1.99.1-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

COPY . .
RUN touch src/main.rs && cargo build --release

# Runtime
FROM debian:stable-slim
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/halisai /app/halisai
COPY --from=builder /app/templates /app/templates
COPY --from=builder /app/static /app/static
COPY --from=builder /app/migrations /app/migrations

ENV BIND_ADDR=0.0.0.0:3000
EXPOSE 3000

CMD ["/app/halisai"]
