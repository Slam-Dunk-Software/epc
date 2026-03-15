.PHONY: test coverage coverage-ci

test:
	cargo test --workspace

coverage:
	cargo llvm-cov --all-features --workspace --html --open

coverage-ci:
	cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
