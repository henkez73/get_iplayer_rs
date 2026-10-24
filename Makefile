all:
	cargo build --release
	cp -f target/release/get_iplayer ./get_iplayer

test:
	cargo test

clean:
	cargo clean

install:
	cargo install --path .

.PHONY: all test clean install
