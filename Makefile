.PHONY: ensure-rustup
ensure-rustup:
ifeq ($(shell which rustup),)
	@read -p "Press enter to install Rustup or Ctrl-C to abort" unused
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
else
	@echo "Found rustup"
endif

.PHONY: ensure-stable
ensure-stable: ensure-rustup
	rustup toolchain install stable
	rustup component add rustfmt clippy

.PHONY: ensure-nightly
ensure-nightly: ensure-rustup
	rustup toolchain install nightly
	rustup component add llvm-tools-preview

.PHONY: ensure-grcov
ensure-grcov: ensure-nightly
ifeq ($(shell which cargo-grcov),)
	cargo install grcov
endif

.PHONY: coverage
coverage: export RUSTFLAGS=-Zinstrument-coverage
coverage: ensure-grcov
	cargo +nightly build
	cargo +nightly test
	grcov . --binary-path ./target/debug/ -s . -t html --branch --ignore-not-existing -o ./target/debug/coverage/
	@echo "You can find the generated coverage report in ./target/debug/coverage/"
