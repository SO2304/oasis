# OASIS-RT — Rust kernel + drone_bridge binary
# Multi-stage: compile Rust → minimal runtime image
#
# Built from the REPOSITORY ROOT, not from oasis-rt/: `docker build -f oasis-rt/Dockerfile .`
#
# The previous version copied only `oasis-rt/Cargo.toml` and `oasis-rt/src`, and that was
# the one job failing in CI. `oasis-rt` gained a **path** dependency on
# `oasis-operator-key` (the dev-dependency the k-of-n quorum tests need), and that crate
# depends on `oasis-rt` back by path — a cycle Cargo allows only inside the workspace.
# Cargo resolves path dependencies when it parses the manifest, so a missing sibling
# breaks the build before a single crate compiles, dev-dependency or not.
#
# So the whole workspace is copied and the build runs from its root, which is how the
# repository is actually built. The builder stage's size does not matter: nothing from it
# reaches the runtime image but one binary.

FROM rust:1.95-slim AS builder
WORKDIR /build
# The virtual manifest and the lockfile first, so one resolver and one lock govern the
# build exactly as they do locally.
COPY Cargo.toml Cargo.lock ./
# The four workspace members. The three MCU crates are excluded from the workspace on
# purpose (see the root manifest) and are not needed here.
COPY oasis-rt ./oasis-rt
COPY oasis-operator-key ./oasis-operator-key
COPY oasis-secure-element ./oasis-secure-element
COPY oasis-trl-harness ./oasis-trl-harness
RUN cargo build --release -p oasis-rt --bin drone_bridge
RUN cargo test --release -p oasis-rt --lib

FROM debian:bookworm-slim AS runtime
LABEL org.opencontainers.image.title="OASIS-RT"
LABEL org.opencontainers.image.description="Command-authority layer for autonomous machine fleets"
RUN groupadd -r oasis && useradd -r -g oasis -s /sbin/nologin oasis \
    && mkdir -p /var/oasis/factory_shared \
    && chown -R oasis:oasis /var/oasis
WORKDIR /var/oasis
COPY --from=builder /build/target/release/drone_bridge /usr/local/bin/drone_bridge
USER oasis
ENV OASIS_SHARED=/var/oasis/factory_shared/

# Default: smoke test — 100 synthetic sensor packets. Override with a real stream.
# Real usage: docker run -i oasis-rt drone_bridge patrol1 0 < sensors.jsonl
HEALTHCHECK --interval=30s --timeout=3s CMD test -d "$OASIS_SHARED" || exit 1
CMD ["sh", "-c", "\
  echo 'OASIS-RT drone_bridge smoke test — kernel outputs should appear below:';\
  for i in $(seq 1 100); do\
    echo '{\"tick\":'$i',\"x\":-2.0,\"y\":-2.0,\"alt\":1.3,\"roll\":0,\"pitch\":0,\"gz\":0,\"rf\":2.0,\"rl\":2.0,\"rr\":2.0,\"rb\":2.0,\"vx\":0,\"vy\":0}';\
  done | drone_bridge patrol1 0 2>&1 | head -30;\
  echo 'Federation files in $OASIS_SHARED:'; ls -la $OASIS_SHARED"]
