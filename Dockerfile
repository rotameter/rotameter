# syntax=docker/dockerfile:1.7

FROM rust:1.99-alpine AS builder
WORKDIR /src

RUN apk add --no-cache musl-dev

COPY Cargo.toml Cargo.lock ./
COPY assets ./assets
COPY crates ./crates

RUN cargo build --release --locked \
    -p rotameter-cli \
    --bin rotameter \
    --no-default-features \
    --features server,sqlite \
 && strip /src/target/release/rotameter

FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=builder /src/target/release/rotameter /usr/local/bin/rotameter

VOLUME ["/var/lib/rotameter"]
EXPOSE 8000

ENTRYPOINT ["/usr/local/bin/rotameter"]
CMD ["serve", "--bind", "0.0.0.0:8000", "--db", "/var/lib/rotameter/rotameter.db"]
