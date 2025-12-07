{
  description = "Software break enforcer, with activity detection.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs { inherit system overlays; };

          ####################################################################
          #### break-enforcer package                                     ####
          ####################################################################
          break-enforcer = with pkgs; let
            src = ./.;
            
            cargoTOML = lib.importTOML "${src}/Cargo.toml";
            rustToolchain = rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
            rust = makeRustPlatform {
              cargo = rustToolchain;
              rustc = rustToolchain;
            };
          in
          rust.buildRustPackage
            {
              pname = cargoTOML.package.name;
              version = cargoTOML.package.version;

              inherit src;

              cargoLock = { lockFile = "${src}/Cargo.lock"; };

              meta = {
                inherit (cargoTOML.package) description homepage;
                maintainers = cargoTOML.package.authors;
              };
            };

          ####################################################################
          #### dev shell                                                  ####
          ####################################################################
          devShell = with pkgs;
            mkShell {
              name = "break-enforcer";
              inputsFrom = [ break-enforcer ];
              RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
              CARGO_TERM_COLOR = "always";
            };
        in
        {
          apps = {
            break-enforcer = {
              type = "app";
              program = "${break-enforcer}/bin/break-enforcer";
              description = "Software break enforcer, with activity detection.";
            };
            default = self.apps.${system}.break-enforcer;
          };
          devShells.default = devShell;
          packages = {
            inherit break-enforcer;
            default = self.packages.${system}.break-enforcer;
          };
          checks = {
            inherit break-enforcer;
          };
        });
}

# {
#   lib,
#   rustPlatform,
#   fetchFromGitHub,
#   stdenv,
#   darwin,
#   pkgs,
# }:
#
# # let 
# # rustPlatform = pkgs.makeRustPlatform {
# # 	cargo = pkgs.rust-bin.stable.lastest.default;
# # 	rustVersion = pkgs.rust-bin.stable.lastest.default;
# # };
# # in {
# rustPlatform.buildRustPackage rec {
#   pname = "break-enforcer";
#   version = "0.3.2";
#
#   src = fetchFromGitHub {
#     owner = "evavh";
#     repo = "break-enforcer";
#     rev = version;
#     hash = "sha256-I1tr37DQyXFB4ucutQv84tbK8VtuF1kVXSb7ayyfkGY=";
#   };
#
#   cargoHash = "sha256-GAC9sKiGyaTY2LnGOxVGTxXteAVOeQZZ79N4ae2GOrY=";
#
#   packages = with pkgs; [
#   	alsa-utils
#   ];
#
#   meta = {
#     description = "Software break enforcer, with activity detection";
#     homepage = "https://github.com/evavh/break-enforcer";
#     changelog = "https://github.com/evavh/break-enforcer/blob/${src.rev}/CHANGELOG.md";
#     maintainers = with lib.maintainers; [ ];
#     mainProgram = "break-enforcer";
#   };
# }
