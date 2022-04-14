
target/bin/grcov:
	cargo install grcov --root target/

.PHONY: code-coverage
code-coverage: target/bin/grcov
	set RUSTFLAGS="-C instrument-coverage"
	set LLVM_PROFILE_FILE="coverage/lcov-%p-%m.profraw"
	cargo test
	target/bin/grcov  . --binary-path target/debug/ -s src -t lcov --branch --ignore-not-existing -o ./coverage/lcov.info
	target/bin/grcov  . --binary-path target/debug/ -s src -t html --branch --ignore-not-existing -o ./coverage/
