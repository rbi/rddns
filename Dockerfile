FROM rust:1.34.2 as builder
LABEL maintainer "raik@voidnode.de"

ARG TARGET_PLATFORM=x86_64-unknown-linux-musl

RUN apt-get update && \
    apt-get install --no-install-recommends -y musl-tools && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

RUN rustup target add ${TARGET_PLATFORM}

# copy source
COPY . /work
WORKDIR /work

# build
RUN cargo build --release --target=${TARGET_PLATFORM} && \
    strip target/${TARGET_PLATFORM}/release/rddns && \
    cargo test --target=${TARGET_PLATFORM} --release

FROM scratch
LABEL maintainer "raik@voidnode.de"

ARG TARGET_PLATFORM=x86_64-unknown-linux-musl

# install
COPY --from=builder /work/target/${TARGET_PLATFORM}/release/rddns /rddns

VOLUME /config/config.toml

ENTRYPOINT ["/rddns", "-c", "/config/config.toml"]
CMD ["trigger"]
