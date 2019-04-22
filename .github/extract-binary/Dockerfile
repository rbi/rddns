FROM debian
LABEL maintainer "raik@voidnode.de"

COPY --from=rddns:dev /rddns /rddns

CMD mkdir -p target && cp /rddns target/
