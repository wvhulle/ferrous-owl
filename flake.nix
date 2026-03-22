{
  description = "Show Rust data ownership flow as diagnostics in your editor";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      crane,
      rust-overlay,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };

      rustToolchain = pkgs.rust-bin.nightly."2025-06-20".default.override {
        extensions = [
          "rust-src"
          "rustc-dev"
          "llvm-tools"
        ];
      };

      craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);

      # Source filtering: only include files relevant to the Cargo build.
      # Changing non-Rust files (markdown, editor configs, CI) won't
      # invalidate the build derivation.
      src = pkgs.lib.fileset.toSource {
        root = ./.;
        fileset = pkgs.lib.fileset.unions [
          (craneLib.fileset.commonCargoSources ./.)
          ./build.rs
        ];
      };

      commonArgs = {
        inherit src;
        strictDeps = true;
        pname = "ferrous-owl";
        version = "0.0.3";

        nativeBuildInputs =
          with pkgs;
          [
            pkg-config
            makeWrapper
            patchelf
            llvmPackages_19.llvm
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.autoPatchelfHook ];

        buildInputs =
          with pkgs;
          [
            zlib
            llvmPackages_19.libllvm
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.stdenv.cc.cc.lib
          ];

        autoPatchelfIgnoreMissingDeps = [ "librustc_driver-*.so" ];

        RUSTC_BOOTSTRAP = "1";
        TOOLCHAIN_CHANNEL = "stable";
        LLVM_CONFIG = "${pkgs.llvmPackages_19.llvm.dev}/bin/llvm-config";

        preBuild = ''
          export NIX_LDFLAGS="$NIX_LDFLAGS -L${pkgs.llvmPackages_19.libllvm}/lib"
        '';
      };

      # Step 1: build only dependencies. Cached as long as Cargo.toml
      # and Cargo.lock don't change.
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      # Step 2: build the actual crate, reusing cached dependency artifacts.
      ferrous-owl = craneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;

          # The binary dynamically loads librustc_driver at runtime, so the
          # Rust toolchain must remain in the runtime closure. Disable
          # Crane's default store-path scrubbing.
          doNotRemoveReferencesToRustToolchain = true;

          preCheck = ''
            export RUSTOWL_SYSROOT="${rustToolchain}"
            export LD_LIBRARY_PATH="${rustToolchain}/lib''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          '';

          postInstall = ''
            wrapProgram $out/bin/ferrous-owl \
              --set RUSTOWL_SYSROOT "${rustToolchain}" \
              --prefix LD_LIBRARY_PATH : "${rustToolchain}/lib"
          '';

          meta = with pkgs.lib; {
            description = "Show Rust data ownership flow as diagnostics in your editor";
            homepage = "https://github.com/wvhulle/ferrous-owl";
            license = licenses.mpl20;
            mainProgram = "ferrous-owl";
            platforms = [
              "x86_64-linux"
              "aarch64-linux"
              "x86_64-darwin"
              "aarch64-darwin"
            ];
          };
        }
      );
    in
    {
      packages.${system}.default = ferrous-owl;

      devShells.${system}.default = craneLib.devShell {
        inputsFrom = [ ferrous-owl ];

        packages = with pkgs; [
          rust-analyzer
        ];

        RUSTC_BOOTSTRAP = "1";
        LLVM_CONFIG = "${pkgs.llvmPackages_19.llvm.dev}/bin/llvm-config";
      };
    };
}
