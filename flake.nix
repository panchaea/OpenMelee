{
  description = "slippi-re";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    crane.inputs.nixpkgs.follows = "nixpkgs";
    ssbm-nix.url = "github:djanatyn/ssbm-nix";
    ssbm-nix.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, ssbm-nix, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        ssbmPkgs = ssbm-nix.packages.${system};

        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        nativeBuildInputs = [ pkgs.clang pkgs.cmake ];
        buildInputs = [ pkgs.enet ];
      in {
        devShells.default = pkgs.mkShell {
          inherit LIBCLANG_PATH;
          buildInputs = nativeBuildInputs ++ buildInputs ++ [
            pkgs.cargo
            pkgs.nixfmt
            pkgs.rust-analyzer
            pkgs.rustc
            pkgs.rustfmt
            (ssbmPkgs.slippi-netplay.overrideAttrs (oldAttrs: rec {
              # TODO: remove version and src after
              # https://github.com/djanatyn/ssbm-nix/pull/27 is merged
              version = "2.5.1";
              src = pkgs.fetchFromGitHub {
                owner = "project-slippi";
                repo = "Ishiiruka";
                rev = "v${version}";
                sha256 = "1ha3hv2lnmjhqn3vhbca6vm3l2p2v0mp94n1lgrvjfrn827g2kbx";
              };
              patches = [ ./ishiiruka.patch ];
            }))
          ];
        };

        packages.default = crane.lib.${system}.buildPackage {
          inherit LIBCLANG_PATH nativeBuildInputs buildInputs;
          src = ./.;
          dontUseCmakeConfigure = true;
        };
      });
}
