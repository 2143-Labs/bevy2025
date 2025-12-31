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
        # Server package - headless, doesn't need graphics libraries
        serverPackage = naersk-lib.buildPackage {
          src = ./.;
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [
            udev
          ];
          cargoBuildOptions = x: x ++ [ "-p" "server" ];
        };
      in
      rec {
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = buildInputsAll;
        };
        packages.server = serverPackage;
        packages.container = pkgs.dockerTools.buildLayeredImage {
          name = "bevy2025";
          contents = [
            serverPackage
            pkgs.cacert
            pkgs.bashInteractive
            pkgs.coreutils
          ];
          config = {
            ExposedPorts = { "25565/udp" = { }; };
            EntryPoint = [ "${serverPackage}/bin/server" ];
            Env = [
              "RUST_LOG=info"
            ];
          };
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
