
flash: build
	cargo run --release

build:
	cargo build --release --target=thumbv6m-none-eabi
	cargo size --release --target=thumbv6m-none-eabi

test:
	cargo test --lib --target=x86_64-unknown-linux-gnu

size:
	cargo size --release --target=thumbv6m-none-eabi

bloat:
	cargo bloat --release --crates
