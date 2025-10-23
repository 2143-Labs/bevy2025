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
      in
      {
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [
            wayland
            libxkbcommon
            vulkan-loader
            alsa-lib
            udev
          ];
        };
        devShell = with pkgs; mkShell {
          buildInputs = [ 
            rust-analyzer
            cargo 
            rustc 
            rustfmt 
            pre-commit 
            rustPackages.clippy
            pkg-config
            wayland
            libxkbcommon
            vulkan-loader
            alsa-lib
            udev
            bacon
          ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          LD_LIBRARY_PATH = lib.makeLibraryPath [
            wayland
            libxkbcommon
            vulkan-loader
            alsa-lib
            udev
          ];
        };
      }
    );
}
