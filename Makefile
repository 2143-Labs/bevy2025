all: build optimize bindgen assets print_size

build:
	cargo b -p client --no-default-features --features web --target wasm32-unknown-unknown --release

optimize: target/wasm32-unknown-unknown/release/client.wasm
	sleep 0.1
	# -Oz crashes here?
	wasm-opt -Os --output target/optimized.wasm target/wasm32-unknown-unknown/release/client.wasm

serve:
	python3 -m http.server --directory web 8080

assets:
	rm ./web/assets
	ln -s ../client/assets/ ./web/assets

bindgen: target/optimized.wasm
	wasm-bindgen --out-name rots_example --out-dir web --target web target/optimized.wasm

print_size:
	@echo "WASM size:"
	@ls -lh target/wasm32-unknown-unknown/release/client.wasm
	@echo "WASM size (optimized):"
	@ls -lh target/optimized.wasm
	@echo "WASM size (bindgen):"
	@ls -lh web/rots_example_bg.wasm
