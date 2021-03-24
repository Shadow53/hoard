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

.PHONY: ensure-tarpaulin
ensure-tarpaulin: # ensure-nightly
ifeq ($(shell which cargo-tarpaulin),)
	cargo install cargo-tarpaulin
endif

.PHONY: clean
clean:
	cargo clean
	rm -f default.profraw

.PHONY: coverage
#coverage: export RUSTFLAGS=-Zinstrument-coverage
coverage: export CARGO_INCREMENTAL=0
coverage: export RUSTFLAGS=-Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests -Cpanic=abort
coverage: export RUSTDOCFLAGS=-Cpanic=abort
coverage: ensure-grcov clean
	cargo +nightly build
	cargo +nightly test
	grcov . -s . -t html --branch --ignore-not-existing -o ./target/debug/coverage/
	#grcov default.profraw --binary-path ./target/debug/ -s . -t html --branch --ignore-not-existing -o ./target/debug/coverage/
	#grcov . --binary-path ./target/debug/ -s . -t lcov --branch --ignore-not-existing -o ./target/debug/lcov.info
	#genhtml -o ./target/debug/coverage/ --show-details --highlight --ignore-errors source --legend ./target/debug/lcov.info
	@echo "You can find the generated coverage report in ./target/debug/coverage/"
	xdg-open ./target/debug/coverage/index.html

.PHONY: src-coverage
src-coverage: export RUSTFLAGS=-Zinstrument-coverage
src-coverage: ensure-grcov clean
	cargo +nightly build
	cargo +nightly test
	grcov default.profraw --binary-path ./target/debug/ -s . -t html --branch --ignore-not-existing -o ./target/debug/coverage/
	#grcov . --binary-path ./target/debug/ -s . -t lcov --branch --ignore-not-existing -o ./target/debug/lcov.info
	#genhtml -o ./target/debug/coverage/ --show-details --highlight --ignore-errors source --legend ./target/debug/lcov.info
	@echo "You can find the generated coverage report in ./target/debug/coverage/"
	xdg-open ./target/debug/coverage/index.html

.PHONY: tarpaulin
#tarpaulin: export RUSTFLAGS=-Zinstrument-coverage
tarpaulin: ensure-tarpaulin clean
	cargo tarpaulin --count --tests -o html --output-dir ./target/debug/coverage
	xdg-open ./target/debug/coverage/tarpaulin-report.html
