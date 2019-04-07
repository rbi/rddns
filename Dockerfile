FROM ekidd/rust-musl-builder:stable as builder
LABEL maintainer "raik@voidnode.de"

# copy source
COPY --chown=rust . .

# build
RUN cargo build --release --target=x86_64-unknown-linux-musl && \
    strip target/x86_64-unknown-linux-musl/release/rddns && \
    cargo test --release

FROM scratch
LABEL maintainer "raik@voidnode.de"

# install
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/rddns /rddns

VOLUME /config/config.toml

ENTRYPOINT ["/rddns", "-c", "/config/config.toml"]
CMD ["server"]
