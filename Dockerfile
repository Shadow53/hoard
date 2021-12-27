FROM rust:alpine

ENV RUSTFLAGS="-Zinstrument-coverage"
ENV LLVM_PROFILE_FILE="profraw/hoard-python-test-%p-%m.profraw"
ENV CI=true GITHUB_ACTIONS=true HOARD_LOG=trace
WORKDIR /hoard-tests

RUN apk add python3 tree py3-yaml py3-toml
COPY target/x86_64-unknown-linux-musl/debug/hoard target/debug/hoard
COPY ci-tests ci-tests
RUN echo $WORKDIR

CMD ["python3", "ci-tests/tests", "all"]
