# =========================================
# Build Rust Codebase
# =========================================
FROM rust:latest as backend-builder

WORKDIR /work

COPY . .

RUN /bin/sh scripts/build.sh

# =========================================
# Build Frontend
# =========================================
FROM node:latest-alpine as frontend-builder

WORKDIR /work

ADD frontend .

RUN npm install
RUN npm build

# =========================================
# Run
# =========================================
FROM debian:bullseye-slim

WORKDIR /work

COPY --from=backend-builder notify-run ./
COPY --from=frontend-builder public ./static

ENTRYPOINT ["/work/notify-run", "serve"]
