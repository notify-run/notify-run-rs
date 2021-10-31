# =========================================
# Build Frontend
# =========================================
FROM node:alpine as frontend-builder

WORKDIR /work

ADD frontend .

RUN ./build.sh

# =========================================
# Build Rust Codebase
# =========================================
FROM rust:latest as backend-builder

RUN echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] http://packages.cloud.google.com/apt cloud-sdk main" \
    | tee -a /etc/apt/sources.list.d/google-cloud-sdk.list \
    && curl https://packages.cloud.google.com/apt/doc/apt-key.gpg \
    | apt-key --keyring /usr/share/keyrings/cloud.google.gpg \
    add - && apt-get update -y && apt-get install google-cloud-sdk -y

WORKDIR /work

COPY . .

RUN /bin/sh deployment/build.sh

# =========================================
# Run
# =========================================
FROM debian:bullseye-slim

WORKDIR /work

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates

COPY --from=backend-builder /work/notify-run ./
COPY --from=frontend-builder /work/public ./static

ENTRYPOINT ["/work/notify-run", "serve"]
