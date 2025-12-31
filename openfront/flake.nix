{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";

    rust = {
      url = "path:./rust";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    frontend = {
      url = "path:./frontend";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    simulator = {
      url = "path:./simulator";
      inputs.nixpkgs.follows = "nixpkgs";
      #inputs.bun2nix.follows = "bun2nix";
      #inputs.openfrontio_a221fee.follows = "openfrontio_a221fee";
      inputs.flake-utils.follows = "utils";
    };

    #bun2nix.url = "github:baileyluTCD/bun2nix";

    openfrontio_cur = {
        url = "github:OpenFrontIO/OpenFrontIO/v0.28.2";
        flake = false;
    };

  };

  outputs = { nixpkgs, utils, rust, simulator, frontend, openfrontio_cur, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      rec {
        packages.default = packages.container;

        # This exists to act like the docker image, except only as a nix executable.
        # It only has two inputs, packages.openfrontpro and packages.openfront-frontend.
        packages.backend = pkgs.stdenv.mkDerivation {
          name = "openfrontpro-bundle";
          src = ./.;
            buildInputs = [
                rust.outputs.packages.${system}.default
                frontend.outputs.packages.${system}.default
                pkgs.cacert
                pkgs.bashInteractive
                pkgs.coreutils
                pkgs.curl
            ];
            nativeBuildInputs = [ pkgs.makeWrapper ];
            installPhase = ''
              mkdir -p $out/bin
              cp ${rust.outputs.packages.${system}.default}/bin/openfrontpro $out/bin/openfrontpro-bundle
              wrapProgram $out/bin/openfrontpro-bundle \
                --set FRONTEND_FOLDER ${frontend.outputs.packages.${system}.default} \
                --set RUST_LOG info
            '';
        };

        packages.container = pkgs.dockerTools.buildLayeredImage {
          name = "openfrontpro";
          contents = [
                packages.backend
                frontend.outputs.packages.${system}.default
          ];

          config = {
            ExposedPorts = { "3000/tcp" = { }; };
            EntryPoint = [ "${packages.backend}/bin/openfrontpro-bundle" ];
            Env = [
              "RUST_LOG=info"
              "FRONTEND_FOLDER=${frontend.outputs.packages.${system}.default}"
            ];
            #Cmd = [ "openfrontpro" ];
          };
        };

        packages.simulator = simulator.outputs.packages.${system}.default;
        # https://github.com/moby/docker-image-spec/blob/v1.2.0/v1.2.md
        packages.container-sim = pkgs.dockerTools.buildLayeredImage {
          name = "openfrontpro-simulator";
          contents = [
            packages.simulator
            simulator.outputs.packages.${system}.simulator-base
            pkgs.nodejs_24
            #simulator.outputs.packages.${system}.default
            pkgs.fish
            pkgs.bash
          ];

          config = {
            #ExposedPorts = { "3000/tcp" = { }; };
            #EntryPoint = [ "fish" ];
            #EntryPoint = [ "${pkgs.fish}/bin/fish" ];
            EntryPoint = [ "${packages.simulator}/bin/openfronter-sim" ];
            Env = [
              #"RUST_LOG=info"
              "MAP_FOLDER=${simulator.outputs.packages.${system}.simulator-base}/OpenFrontIO/resources/maps"
            ];
            #Cmd = [ "-c" "${packages.simulator}/bin/openfronter-sim" ];
          };
        };
      }
    );
}
