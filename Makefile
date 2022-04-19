
.PHONY: clean
clean:
	rm -rf coverage/
	rm -rf target/

target/bin/grcov:
	cargo install grcov --root target/

.PHONY: code-coverage
code-coverage: target/bin/grcov
	RUSTFLAGS="-C instrument-coverage" LLVM_PROFILE_FILE="coverage/lcov-%p-%m.profraw" cargo test
	target/bin/grcov src --binary-path target/debug/ -s . -t html --branch --ignore-not-existing -o ./coverage/
	target/bin/grcov src --binary-path target/debug/ -s . -t lcov --branch --ignore-not-existing -o ./coverage/lcov.info
