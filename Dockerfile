FROM alpine
LABEL maintainer "raik@voidnode.de"

WORKDIR /rddns
COPY . .

# install build tools
RUN apk add --no-cache cargo

# build
RUN cargo build --release

FROM alpine
LABEL maintainer "raik@voidnode.de"

# install
COPY --from=0 /rddns/target/release/rddns /rddns

RUN chown root:root /rddns && \
    apk add --no-cache libgcc && \
    adduser -S rddns

VOLUME /config
USER rddns

ENTRYPOINT ["/rddns", "-c", "/config/config.toml"]
CMD ["server"]
