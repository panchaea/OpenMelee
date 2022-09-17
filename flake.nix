{
  description = "slippi-re";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    crane.inputs.nixpkgs.follows = "nixpkgs";
    ssbm-nix.url = "github:djanatyn/ssbm-nix";
    ssbm-nix.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
    pre-commit-hooks.inputs.nixpkgs.follows = "nixpkgs";
    pre-commit-hooks.inputs.flake-utils.follows = "flake-utils";
  };

  outputs =
    { self, nixpkgs, crane, ssbm-nix, flake-utils, pre-commit-hooks, ... }:
    nixpkgs.lib.recursiveUpdate (flake-utils.lib.eachSystem [
      "x86_64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ] (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        ssbmPkgs = ssbm-nix.packages.${system};
        craneLib = crane.lib.${system};

        _crateBuildAttrs = {
          src = ./.;
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          nativeBuildInputs = [ pkgs.clang pkgs.cmake ];
          buildInputs = [ pkgs.enet pkgs.sqlite ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.libiconv
            ];
          dontUseCmakeConfigure = true;
        };

        cargoArtifacts = craneLib.buildDepsOnly _crateBuildAttrs;

        crateBuildAttrs = _crateBuildAttrs // { inherit cargoArtifacts; };

        crate = craneLib.buildPackage crateBuildAttrs;

        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          inherit (_crateBuildAttrs) src;
          hooks.nixfmt.enable = true;
        };
      in {
        devShells.default = pkgs.mkShell {
          inherit (crateBuildAttrs) LIBCLANG_PATH nativeBuildInputs;
          inherit (pre-commit-check) shellHook;
          buildInputs = crateBuildAttrs.buildInputs ++ [
            pkgs.cargo
            pkgs.clippy
            pkgs.diesel-cli
            pkgs.nixfmt
            pkgs.rust-analyzer
            pkgs.rustc
            pkgs.rustfmt
          ] ++ pkgs.lib.optionals (system == "x86_64-linux") [
            (ssbmPkgs.slippi-netplay.overrideAttrs
              (oldAttrs: rec { patches = [ ./ishiiruka.patch ]; }))
          ];
        };

        packages.default = crate;

        apps.default = flake-utils.lib.mkApp { drv = crate; };

        checks = {
          inherit crate pre-commit-check;
          clippy = craneLib.cargoClippy crateBuildAttrs;
          formatting = craneLib.cargoFmt crateBuildAttrs;
        };
      })) {
        overlays.default = final: _prev: {
          slippi-re = self.packages.${final.stdenv.system}.default;
        };

        nixosModules.default = {
          nixpkgs.overlays = [ self.overlays.default ];
          imports = [ ./module.nix ];
        };
      };
}
