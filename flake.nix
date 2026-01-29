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

				  nativeBuildInputs = with pkgs; [
				  	makeWrapper
				  ];

				  postInstall = ''
				    wrapProgram $out/bin/break-enforcer \
					  --prefix PATH : "${nixpkgs.lib.makeBinPath [
					  pkgs.alsa-utils ]}"
				  '';
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
          devShells.default = devShell;
		  defaultPackage = break-enforcer;
        }) // {
			overlays.default = _: prev: {
				break-enforcer = self.defaultPackage.${prev.system};
			};
			  nixosModules.break-enforcer = ./nix_module.nix;
		};
}
