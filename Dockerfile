FROM rust:alpine AS build

RUN apk add build-base
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY src src
RUN cargo build

FROM ubuntu:latest

ARG CI=true GITHUB_ACTIONS=true HOARD_LOG=debug
RUN apt-get update && apt-get install -y tree python3

COPY --from=build target/debug/hoard target/debug/hoard
COPY ci-tests ci-tests

RUN python3 ci-tests/tests cleanup
RUN python3 ci-tests/tests last_paths
RUN python3 ci-tests/tests operation
RUN python3 ci-tests/tests ignore
