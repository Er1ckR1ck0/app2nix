{
  description = "app2nix - Convert .deb packages to Nix expressions";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        buildInputs = with pkgs; [
          openssl
        ];

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config

          patchelf
          binutils
          gnutar
          wget
          nix-index
          dpkg
        ];

      in
      {
        packages = {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "app2nix";
            version = "0.1.0";
            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            inherit buildInputs;
            nativeBuildInputs = with pkgs; [ pkg-config ];

            postInstall = ''
              mkdir -p $out/share/app2nix
              cp ${./libraries.json} $out/share/app2nix/libraries.json
            '';

            meta = with pkgs.lib; {
              description = "Convert .deb packages to Nix expressions";
              homepage = "https://github.com/Er1ckR1ck0/app2nix";
              license = licenses.mit;
              maintainers = [ ];
              platforms = platforms.linux;
            };
          };

          app2nix = self.packages.${system}.default;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = self.packages.${system}.default;
          };
          app2nix = self.apps.${system}.default;
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [
            cargo-watch
            cargo-edit
            clippy
          ]);

          shellHook = ''
            echo "🦀 app2nix development environment"
            echo ""
            echo "Available commands:"
            echo "  cargo build    - Build the project"
            echo "  cargo run      - Run the project"
            echo "  cargo test     - Run tests"
            echo "  cargo watch    - Watch for changes and rebuild"
            echo ""
            echo "Runtime tools available: patchelf, ar, tar, wget, nix-locate, dpkg"
          '';

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          RUST_BACKTRACE = "1";
        };

        checks = {
          format = pkgs.runCommand "check-format" {
            nativeBuildInputs = [ rustToolchain ];
          } ''
            cd ${self}
            cargo fmt --check
            touch $out
          '';

          clippy = pkgs.runCommand "check-clippy" {
            nativeBuildInputs = [ rustToolchain pkgs.pkg-config ] ++ buildInputs;
          } ''
            cd ${self}
            cargo clippy -- -D warnings
            touch $out
          '';
        };
      }
    );
}
