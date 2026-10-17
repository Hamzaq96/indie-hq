FROM rust:1.88-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build -p server --release

FROM debian:bookworm-slim
COPY --from=build /app/target/release/server /usr/local/bin/server
USER nobody
CMD ["server"]
