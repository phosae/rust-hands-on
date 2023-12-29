# docker buildx build --platform linux/amd64,linux/arm64 -t zengxu/car-rust-up --push .
FROM rust:1.75.0-slim-bullseye AS build
WORKDIR /workspace
ARG TARGETARCH

# start of mirror settings for Chinese users
ENV RUSTUP_DIST_SERVER https://mirrors.ustc.edu.cn/rust-static 
ENV RUSTUP_UPDATE_ROOT https://mirrors.ustc.edu.cn/rust-static/rustup
RUN <<EOF bash
printf '[source.crates-io]
replace-with = "ustc"
[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
' | tee -a ${CARGO_HOME:-$HOME/.cargo}/config
EOF
# end of mirror settings for Chinese users

COPY platform.sh platform.sh
COPY Cargo.toml Cargo.toml
COPY src/ src/

RUN ./platform.sh 
RUN rustup target add $(cat /.platform) 
RUN rustup toolchain install stable-$(cat /.platform) 

RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/cache \
    --mount=type=cache,target=/usr/local/cargo/registry/index \
    RUST_BACKTRACE=1 cargo build --target $(cat /.platform) --release

RUN cp /workspace/target/$(cat /.platform)/release/rust-hands-on /usr/local/bin/rust-hands-on

FROM debian:bullseye-20231030-slim
COPY --from=build /usr/local/bin/rust-hands-on /rust-hands-on
ENTRYPOINT ["/rust-hands-on"]