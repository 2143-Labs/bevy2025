all: build optimize bindgen assets

build:
	cargo b -p client --no-default-features --features web --target wasm32-unknown-unknown --release

optimize:
	sleep 0.1
	# -Oz crashes here?
	wasm-opt -Os --output optimized.wasm target/wasm32-unknown-unknown/release/client.wasm
	mv optimized.wasm target/wasm32-unknown-unknown/release/client.wasm

serve:
	python3 -m http.server --directory web 8080

assets:
	rm ./web/assets
	ln -s ../client/assets/ ./web/assets

bindgen:
	wasm-bindgen --out-name rots_example --out-dir web --target web "target/wasm32-unknown-unknown/release/client.wasm"
