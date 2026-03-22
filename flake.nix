{
  description = "Show Rust data ownership flow as diagnostics in your editor";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
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

      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };
    in
    {
      packages.${system}.default = rustPlatform.buildRustPackage {
        pname = "ferrous-owl";
        version = "0.0.3";

        src = pkgs.lib.cleanSource ./.;

        cargoLock.lockFile = ./Cargo.lock;

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

        env = {
          RUSTC_BOOTSTRAP = "1";
          TOOLCHAIN_CHANNEL = "stable";
          LLVM_CONFIG = "${pkgs.llvmPackages_19.llvm.dev}/bin/llvm-config";
        };

        preBuild = ''
          export NIX_LDFLAGS="$NIX_LDFLAGS -L${pkgs.llvmPackages_19.libllvm}/lib"
        '';

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
      };

      devShells.${system}.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rustToolchain
          rust-analyzer
          pkg-config
          zlib
          llvmPackages_19.llvm
          llvmPackages_19.libllvm
        ];

        RUSTC_BOOTSTRAP = "1";
        LLVM_CONFIG = "${pkgs.llvmPackages_19.llvm.dev}/bin/llvm-config";
      };
    };
}
