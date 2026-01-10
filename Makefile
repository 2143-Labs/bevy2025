# Source wasm from cargo build
CARGO_WASM := target/wasm32-unknown-unknown/release/client.wasm
# Optimized wasm
OPT_WASM := target/optimized.wasm
# Final bindgen output
BINDGEN_WASM := web/rots_example_bg.wasm

.PHONY: all build serve assets print_size clean

all: build $(BINDGEN_WASM) assets print_size

# Cargo build - always runs cargo (it handles incremental builds internally)
build:
	cargo b -p client --no-default-features --features web --target wasm32-unknown-unknown --release

# Optimize only if cargo wasm is newer than optimized wasm
$(OPT_WASM): $(CARGO_WASM)
	wasm-opt -Os --output $@ $<

# Bindgen only if optimized wasm is newer
$(BINDGEN_WASM): $(OPT_WASM)
	wasm-bindgen --out-name rots_example --out-dir web --target web $<

serve:
	python3 -m http.server --directory web 8080

assets:
	@if [ ! -L ./web/assets ]; then \
		ln -s ../client/assets/ ./web/assets; \
	fi

print_size: $(BINDGEN_WASM)
	@echo "WASM size:"
	@ls -lh $(CARGO_WASM)
	@echo "WASM size (optimized):"
	@ls -lh $(OPT_WASM)
	@echo "WASM size (bindgen):"
	@ls -lh $(BINDGEN_WASM)

clean:
	rm -f $(OPT_WASM) $(BINDGEN_WASM) web/rots_example.js
	@echo "Cleaned generated wasm files (cargo clean not run)"
