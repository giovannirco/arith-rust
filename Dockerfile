# syntax=docker/dockerfile:1

# Stage 1: a static binary for the platform the image is built on.
FROM rust:1.99.1-alpine3.23@sha256:51e1b72c7f236093c084f6e3051239f033fba9e4c0c1430cc2ed479d0a3bd14c AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web
# The two cache mounts keep the registry and the compiled dependencies between
# builds, so a one-line change rebuilds in seconds rather than minutes.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked && cp target/release/arith /arith

# Stage 2: nothing but the binary, CA certificates and a non-root user.
# distroless/static has no shell and no package manager. Pinned by digest.
FROM gcr.io/distroless/static-debian13:nonroot@sha256:25ac8f0c3b2334a9eb05452266bd4697864a8c8ef0ef71e5e6480d7f92868329
LABEL org.opencontainers.image.source="https://github.com/giovannirco/arith-rust" \
      org.opencontainers.image.description="Integer arithmetic over HTTP: four endpoints, a page, metrics, traces and logs." \
      org.opencontainers.image.licenses="MIT"
COPY --from=build /arith /arith
EXPOSE 8000
# distroless' nonroot user, by number so `runAsNonRoot: true` can verify it.
USER 65532:65532
ENTRYPOINT ["/arith"]
