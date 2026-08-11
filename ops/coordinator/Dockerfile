# Build context = repo root:  docker build -f ops/coordinator/Dockerfile -t ducktape-coordinator .
FROM rust:1.96-bookworm AS build
WORKDIR /src
COPY . .
# Only the coordinator crate and its (workspace) deps are compiled.
RUN cargo build --release -p coordinator-bin --bin coordinator

# Distroless cc runtime: glibc + libgcc for the dynamically-linked binary, no
# shell, no package manager, runs as the built-in non-root uid 65532 (nonroot).
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
COPY --from=build /src/target/release/coordinator /usr/local/bin/ducktape-coordinator
ENV MALLOC_ARENA_MAX=1
USER nonroot:nonroot
# STUN/rendezvous control port. Publish with:
# -p 3478:3478/udp
EXPOSE 3478/udp
ENTRYPOINT ["/usr/local/bin/ducktape-coordinator"]
# The default image exposes UDP rendezvous only. Opt into the sealed-intro TCP
# fallback explicitly, mapping host 443 to an unprivileged container port.
CMD ["--listen", "0.0.0.0:3478", "--relay-listen", "none", "--workers", "4"]
