# syntax=docker/dockerfile:1
FROM alpine:3 as builder

WORKDIR /usr/src/screenly-cli
RUN apk add --no-cache wget tar
ARG RELEASE=v1.1.0
RUN wget "https://github.com/Screenly/cli/releases/download/$RELEASE/screenly-cli-x86_64-unknown-linux-musl.tar.gz"
RUN tar xfz screenly-cli-x86_64-unknown-linux-musl.tar.gz

FROM alpine:3
COPY --from=builder /usr/src/screenly-cli/screenly /usr/bin/
ENTRYPOINT ["/usr/bin/screenly"]
