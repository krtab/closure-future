
.PHONY: all doc check test

all: test doc

doc:
	cargo +nightly doc --no-deps --all-features

check:
	cargo +stable check --features rayon

test:
	cargo +stable test --features rayon