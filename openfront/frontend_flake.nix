{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = inputs:
    inputs.flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import (inputs.nixpkgs) { inherit system; });
      in rec {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.nodejs_24
            pkgs.nodePackages.eslint
            pkgs.nodePackages.typescript-language-server
            pkgs.nodePackages.typescript

          ];

          shellHook = ''
            echo "Frontend development shell"
            echo "Run 'npm install' to install dependencies"
          '';
        };

        packages.default = packages.frontend-node;
        packages.frontend-node = pkgs.buildNpmPackage {
          pname = "openfront-frontend";
          version = "0.1.0";
          buildInputs = [
            pkgs.nodejs_24
          ];

          src = ./.;
          npmDeps = pkgs.importNpmLock {
            npmRoot = ./.;
          };
          npmConfigHook = pkgs.importNpmLock.npmConfigHook;

          installPhase = ''
            mkdir -p $out
            cp -r dist/* $out/
          '';
        };
      }
    );
}
