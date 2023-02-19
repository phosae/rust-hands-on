# docker buildx build --platform linux/amd64,linux/arm64 -t zengxu/car-rust-up --push .
FROM rust:1.67.0 AS build
WORKDIR /workspace
ARG TARGETARCH

COPY platform.sh platform.sh
COPY Cargo.toml Cargo.toml
COPY src/ src/

RUN ./platform.sh
RUN rustup target add $(cat /.platform) 
RUN rustup toolchain install \
    stable-$(cat /.platform) 

RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/cache \
    --mount=type=cache,target=/usr/local/cargo/registry/index \
    RUST_BACKTRACE=1 cargo build --target $(cat /.platform) --release

RUN cp  /workspace/target/$(cat /.platform)/release/qappctl-shim-rs /usr/local/bin/qappctl-shim-rs

FROM debian:bullseye-20230208-slim
COPY --from=build /usr/local/bin/qappctl-shim-rs /qappctl-shim-rs
ENTRYPOINT [ "/qappctl-shim-rs"]