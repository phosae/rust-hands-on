# syntax=docker/dockerfile:1
# docker buildx build --platform wasi/wasm32 -t zengxu/car-demo-wasm .

FROM --platform=$BUILDPLATFORM zengxu/rust-wasmedge:1.67.0 AS build
COPY Cargo.toml Cargo.toml
COPY src/ src/ 
# Build the Wasm binary
RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/cache \
    --mount=type=cache,target=/usr/local/cargo/registry/index \
    cargo build --target wasm32-wasi --release
# This line builds the AOT Wasm binary
RUN /root/.wasmedge/bin/wasmedgec target/wasm32-wasi/release/car_demo_service.wasm car_demo_service.wasm

FROM scratch
ENTRYPOINT [ "car_demo_service.wasm" ]
COPY --link --from=build /src/car_demo_service.wasm /car_demo_service.wasm