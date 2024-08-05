{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
        "aarch64-linux"
      ];

      perSystem = {
        config,
        system,
        pkgs,
        ...
      }: {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;

          overlays = [
            inputs.fenix.overlays.default
          ];

          config = {};
        };

        formatter = pkgs.alejandra;

        packages.rust-toolchain = pkgs.fenix.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-nqZ6L3zf3f9xh1Qh53BhBM+cWfO9S8VPlfE0h+1lg6o=";
        };

        packages.ci-deps = pkgs.symlinkJoin {
          name = "ci-deps";

          paths = with pkgs;
            [
              taplo
              nodejs_20
              wasmtime
              cargo-nextest
              cargo-udeps
              config.packages.rust-toolchain
              (python312.withPackages (python-pkgs: [
                python-pkgs.colorama
                python-pkgs.toml
              ]))
            ]
            ++ lib.lists.optional (stdenv.isDarwin) pkgs.darwin.libiconv;
        };

        # CI Setup for GitHub Actions
        packages.ci-env = pkgs.writeShellScriptBin "ci-setup" ''
          echo CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-args=-Wl,-rpath,${config.packages.ci-deps}/lib/"
          echo CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-args=-Wl,-rpath,${config.packages.ci-deps}/lib/"
          echo CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS="-C link-args=-Wl,-rpath,${config.packages.ci-deps}/lib/"
        '';

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            config.packages.ci-deps
          ];

          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = ''-C link-args=-Wl,-rpath,${config.packages.ci-deps}/lib/'';
          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS = ''-C link-args=-Wl,-rpath,${config.packages.ci-deps}/lib/'';
          CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS = ''-C link-args=-Wl,-rpath,${config.packages.ci-deps}/lib/'';
        };
      };
    };
}
