# sinterklaas: one Rust binary serving the pages, plus the stylesheet and the
# decoration script. Pushed as xinutec/sinterklaas:latest by CI.

FROM rust:1.98-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Prime the dependency cache with a stub crate, then build for real.
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs \
    && cargo build --release && rm -rf src
COPY src/ src/
COPY migrations/ migrations/
RUN touch src/main.rs src/lib.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
# Reads only; never runs as root. uid/gid 65532 is the conventional nonroot id.
RUN groupadd --gid 65532 sinterklaas \
    && useradd --uid 65532 --gid sinterklaas --no-create-home --shell /usr/sbin/nologin sinterklaas
WORKDIR /app
COPY --from=build /app/target/release/sinterklaas /usr/local/bin/sinterklaas
COPY static/ ./static/
ENV STATIC_DIR=/app/static \
    BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
USER sinterklaas
CMD ["sinterklaas"]
