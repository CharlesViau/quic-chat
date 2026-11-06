# syntax=docker/dockerfile:1

# Build Linux binaries using the same Rust version as the local toolchain.
FROM rust:1.99.1-bookworm AS build
WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release --bins

# The running container needs the binaries, not Cargo or the compiler.
FROM debian:bookworm-slim AS runtime

RUN groupadd --gid 10001 chat \
    && useradd --uid 10001 --gid chat --no-create-home --shell /usr/sbin/nologin chat \
    && mkdir -p /var/lib/quic-chat \
    && chown chat:chat /var/lib/quic-chat \
    && chmod 0700 /var/lib/quic-chat

COPY --from=build /build/target/release/server /usr/local/bin/server
COPY --from=build /build/target/release/client /usr/local/bin/client
COPY --from=build /build/target/release/setup /usr/local/bin/setup

# The Rust programs will read this when certificate persistence is implemented.
ENV QUIC_CHAT_IDENTITY_DIR=/var/lib/quic-chat
WORKDIR /var/lib/quic-chat
USER 10001:10001

# QUIC runs over UDP. Publishing the port is configured in compose.yaml.
EXPOSE 5000/udp
CMD ["server"]
