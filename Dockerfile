# syntax=docker/dockerfile:1
FROM rust:1-alpine3.15 as builder

WORKDIR /usr/src/screenly-cli
COPY . .
RUN apk --no-cache add ca-certificates openssl-dev
RUN cargo build --release

FROM rust:3.15
COPY --from=builder /usr/src/screenly-cli/target/release/screenly /usr/bin/
ENTRYPOINT ["/usr/bin/screenly"]
