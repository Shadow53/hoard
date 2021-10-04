FROM rust:alpine AS build
ENV RUSTFLAGS="-Zinstrument-coverage"
ENV LLVM_PROFILE_FILE="profraw/hoard-test-%p-%m.profraw"
ENV CI=true GITHUB_ACTIONS=true HOARD_LOG=trace
WORKDIR /hoard-tests

RUN apk add python3 tree
#RUN apk add build-base python3 tree
#RUN rustup toolchain add nightly --component llvm-tools-preview
#COPY Cargo.toml Cargo.toml
#COPY Cargo.lock Cargo.lock
#COPY src src
#RUN cargo +nightly build

#FROM ubuntu:latest
#ENV RUSTFLAGS="-Zinstrument-coverage"
#ENV LLVM_PROFILE_FILE="/hoard-tests/profraw/hoard-test-%p-%m.profraw"

#VOLUME /hoard-tests/profraw

#RUN apt-get update && apt-get install -y tree python3

#COPY --from=build target/debug/hoard target/debug/hoard
COPY target/x86_64-unknown-linux-musl/debug/hoard target/debug/hoard
COPY ci-tests ci-tests
RUN echo $WORKDIR

CMD ["python3", "ci-tests/tests", "all"]
