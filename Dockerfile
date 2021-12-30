FROM rust:alpine

ENV RUSTFLAGS="-Zinstrument-coverage"
ENV LLVM_PROFILE_FILE="profraw/hoard-python-test-%p-%m.profraw"
ENV CI=true GITHUB_ACTIONS=true HOARD_LOG=trace #XDG_UTILS_DEBUG_LEVEL=4
WORKDIR /hoard-tests

RUN apk add python3 tree py3-yaml py3-toml xdg-utils file
COPY target/x86_64-unknown-linux-musl/debug/hoard target/debug/hoard
COPY ci-tests ci-tests
COPY ci-tests/opener-bin /usr/bin/opener-bin
COPY ci-tests/xdg-open /usr/bin/xdg-open
RUN echo $WORKDIR
RUN file --brief --dereference --mime-type "/hoard-tests/ci-tests/config.toml"

CMD ["python3", "ci-tests/tests", "all"]
