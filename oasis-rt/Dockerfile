# OASIS-RT — bio-inspired Rust kernel + drone_bridge binary
# Multi-stage: compile Rust → minimal runtime image

FROM rust:1.82-slim AS builder
WORKDIR /build
# Build oasis-rt standalone (outside the workspace): copy only its manifest +
# src. The per-member Cargo.lock was removed when oasis-rt joined the workspace
# (the lock now lives at the repo root), so we let cargo resolve a fresh lock here.
COPY oasis-rt/Cargo.toml ./Cargo.toml
COPY oasis-rt/src ./src
RUN cargo build --release --bin drone_bridge
RUN cargo test --release --lib

FROM debian:bookworm-slim AS runtime
LABEL org.opencontainers.image.title="OASIS-RT"
LABEL org.opencontainers.image.description="Bio-inspired agentic kernel (HyperState, Hebbian, reflex, federation)"
RUN groupadd -r oasis && useradd -r -g oasis -s /sbin/nologin oasis \
    && mkdir -p /var/oasis/factory_shared \
    && chown -R oasis:oasis /var/oasis
WORKDIR /var/oasis
COPY --from=builder /build/target/release/drone_bridge /usr/local/bin/drone_bridge
USER oasis
ENV OASIS_SHARED=/var/oasis/factory_shared/

# Default: smoke test — 100 synthetic sensor packets. Override with real stream.
# Real usage: docker run -i oasis-rt drone_bridge patrol1 0 < sensors.jsonl
HEALTHCHECK --interval=30s --timeout=3s CMD test -d "$OASIS_SHARED" || exit 1
CMD ["sh", "-c", "\
  echo 'OASIS-RT drone_bridge smoke test — kernel outputs should appear below:';\
  for i in $(seq 1 100); do\
    echo '{\"tick\":'$i',\"x\":-2.0,\"y\":-2.0,\"alt\":1.3,\"roll\":0,\"pitch\":0,\"gz\":0,\"rf\":2.0,\"rl\":2.0,\"rr\":2.0,\"rb\":2.0,\"vx\":0,\"vy\":0}';\
  done | drone_bridge patrol1 0 2>&1 | head -30;\
  echo 'Federation files in $OASIS_SHARED:'; ls -la $OASIS_SHARED"]
