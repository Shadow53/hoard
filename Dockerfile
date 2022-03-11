FROM rust:alpine

ENV RUSTC_BOOTSTRAP="1"
ENV RUSTFLAGS="-Zinstrument-coverage"
ENV LLVM_PROFILE_FILE="profraw/hoard-test-%p-%9m.profraw"
ENV CI=true GITHUB_ACTIONS=true HOARD_LOG=trace
WORKDIR /hoard-tests

RUN apk add build-base xdg-utils file busybox
COPY Cargo.toml Cargo.lock config.toml.sample ./
COPY src ./src
RUN cargo test --no-run
CMD cargo test -- --test-threads=1
