{
  description = "Bevy 2025 game project with client and server";

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
            # TODO not really needed?
            alsa-lib
          ];
          cargoBuildOptions = x: x ++ [ "-p" "server" ];
        };
        # Client package - needs graphics libraries
        clientPackage = naersk-lib.buildPackage {
          src = ./.;
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = buildInputsAll;
          cargoBuildOptions = x: x ++ [ "-p" "client" ];
        };
      in
      {
        # Default package is the client
        packages.default = clientPackage;
        packages.client = clientPackage;
        packages.server = serverPackage;
        packages.container = pkgs.dockerTools.buildLayeredImage {
          name = "bevy2025";
          tag = "latest";
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
            # Add labels for better container metadata
            Labels = {
              "org.opencontainers.image.source" = "https://github.com/2143-labs/bevy2025";
              "org.opencontainers.image.description" = "Bevy 2025 game server";
            };
          };
        };
        devShells.default = with pkgs; mkShell {
          buildInputs = [
            rust-analyzer
            cargo 
            rustPackages.rustfmt
            rustPackages.clippy
            cargo-flamegraph
            pre-commit 
            pkg-config
            bacon
            # Additional useful development tools
            cargo-audit
            cargo-deny
            cargo-outdated
            nixpkgs-fmt
            # lld is specifically required by the wasm compiler for web builds (tracing-wasm)
            lld
            binaryen
          ] ++ buildInputsAll;
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputsAll;
          # Set environment variables for better development experience
          shellHook = ''
            echo "Bevy 2025 development environment"
          '';
        };
      }
    );
}
