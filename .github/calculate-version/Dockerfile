FROM debian:9

LABEL maintainer="Raik Bieniek <raik.bieniek@gmail.com>"
LABEL com.github.actions.name="Calculate Version"
LABEL com.github.actions.description="Calculates an indivdual version number based on the base version in the Cargo.toml file."
LABEL com.github.actions.icon="tag"
LABEL com.github.actions.color="blue"

COPY "entrypoint.sh" /
ENTRYPOINT ["/entrypoint.sh"]
