all: build bindgen assets

build:
	cargo b -p client --no-default-features --features web --target wasm32-unknown-unknown --release

optimize:
	wasm-opt -Oz --output optimized.wasm target/wasm32-unknown-unknown/release/client.wasm
	mv optimized.wasm target/wasm32-unknown-unknown/release/client.wasm

serve:
	python3 -m http.server --directory web 8080

assets:
	rm ./web/assets
	ln -s ../client/assets/ ./web/assets

bindgen: build optimize
	wasm-bindgen --out-name rots_example --out-dir web --target web "target/wasm32-unknown-unknown/release/client.wasm"
