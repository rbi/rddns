FROM debian
LABEL maintainer "raik@voidnode.de"

COPY --from=sirabien/rddns:dev /rddns /rddns

CMD mkdir -p target && cp /rddns target/
