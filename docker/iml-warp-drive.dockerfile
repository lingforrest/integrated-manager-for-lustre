FROM rust-iml-base as builder
FROM imlteam/rust-service-base:5.1.1-dev

COPY --from=builder /build/target/release/iml-warp-drive /usr/local/bin
COPY docker/wait-for-dependencies-postgres.sh /usr/local/bin

ENTRYPOINT [ "wait-for-dependencies-postgres.sh" ]
CMD ["iml-warp-drive"]
