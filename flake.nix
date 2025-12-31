{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
        buildInputsAll = with pkgs; [
          wayland
          libxkbcommon
          vulkan-loader
          alsa-lib
          udev
        ];
      in {
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = buildInputsAll;
        };
        devShell = with pkgs; mkShell {
          buildInputs = [
            rust-analyzer
            cargo 
            rustPackages.rustfmt
            rustPackages.clippy
            #rustPackages.cargo-flamegraph
            cargo-flamegraph
            pre-commit 
            pkg-config
            bacon
          ] ++ buildInputsAll;
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputsAll;
        };
      }
    );
}
