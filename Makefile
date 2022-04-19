
.PHONY: clean
clean:
	rm -rf coverage/
	rm -rf target/

target/bin/grcov:
	cargo install grcov --root target/

coverage/report.lcov: target/bin/grcov target/debug/
	target/bin/grcov src --binary-path target/debug/ -s . --keep-only 'src/**' --prefix-dir $PWD -t lcov --branch --ignore-not-existing -o coverage/report.lcov

generate-lcov-coverage: coverage/report.lcov

generate-test-coverage:
	RUSTFLAGS="-C instrument-coverage" LLVM_PROFILE_FILE="coverage/lcov-%p-%m.profraw" cargo test

coverage/index.html: target/bin/grcov generate-test-coverage coverage/report.lcov
	target/bin/grcov src --binary-path target/debug/ -s . -t html --branch --ignore-not-existing -o ./coverage/

.PHONY: code-coverage
code-coverage: coverage/index.html
