{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bun2nix.url = "github:baileyluTCD/bun2nix";
    openfrontio_cur = {
        url = "github:OpenFrontIO/OpenFrontIO/v0.28.2";
        flake = false;
    };
  };

  outputs = { nixpkgs, bun2nix, openfrontio_cur, flake-utils, ... } @ inputs:
    inputs.flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import (nixpkgs) { inherit system; });
            in rec {
        bun2nix = inputs.bun2nix;
        devShells.default = pkgs.mkShell {
          buildInputs = [
              pkgs.bun
            pkgs.nodejs_24
            pkgs.nodePackages.typescript-language-server
            pkgs.nodePackages.eslint
            pkgs.nodePackages.typescript
            pkgs.python3
            pkgs.pkg-config
            pkgs.nixd
            pkgs.pixman
            pkgs.cairo
            pkgs.pango
            #packages.my-husky
            # Add the bun2nix binary to our devshell
            bun2nix.packages.${system}.default
          ];

          shellHook = ''
            echo "Frontend development shell"
          '';
        };

        # TODO Switch to bun when it is more stable
        packages.simulator-bun = pkgs.callPackage ./. {
          inherit (bun2nix.lib.${system}) mkBunDerivation;
          inherit pkgs;
        };

        # we need to combine the flake input openfrontio with the simulator package
        packages.simulator-base = pkgs.stdenv.mkDerivation {
          pname = "openfront-simulator-base";
          version = "0.1.0";
          src = ./.;
          buildInputs = [
            pkgs.nodejs_24
            pkgs.jq
          ];

          installPhase = ''
            mkdir -p $out/OpenFrontIO/
            cp -r ${openfrontio_cur}/src/ $out/OpenFrontIO/src/
            cp -r ${openfrontio_cur}/resources/ $out/OpenFrontIO/resources/
            cp -r ${openfrontio_cur}/package.json $out/OpenFrontIO/package.json
            cp -r * $out/
            jq 'del(.scripts.prepare)' $out/OpenFrontIO/package.json > $out/OpenFrontIO/package.json.tmp
            mv $out/OpenFrontIO/package.json.tmp $out/OpenFrontIO/package.json
          '';
          #ln -s ${packages.my-husky}/bin/husky $out/bin/husky
        };

        packages.default = packages.simulator-node;
        packages.simulator-node = pkgs.buildNpmPackage {
          pname = "openfronter-sim";
          version = "0.1.0";
          buildInputs = [
            pkgs.nodejs_24
            packages.simulator-base
          ];

          # We also need to include submodules here:
          src = packages.simulator-base;
          npmDeps = pkgs.importNpmLock {
            npmRoot = packages.simulator-base;
          };
            npmConfigHook = pkgs.importNpmLock.npmConfigHook;
            dontNpmPrune = true;
          nodejs = pkgs.nodejs_24;  # ensures proper shebang patching
          installPhase = ''
            runHook preInstall

            mkdir -p $out/bin/
            mkdir -p $out/
            cp -r src/ $out/src
            cp ./package.json $out/
            cp -r ./node_modules/ $out/node_modules/
            #ln -s ./node_modules $out/node_modules

            mkdir -p $out/OpenFrontIO/resources/maps/
            cp -r OpenFrontIO/resources/maps/ $out/OpenFrontIO/resources/maps/

            cat > $out/bin/openfronter-sim <<EOF
            #!/bin/sh
            ${pkgs.nodejs_24}/bin/npm start
            EOF
            chmod +x $out/bin/openfronter-sim

            runHook postInstall
            '';
        };
        }
    );
}
