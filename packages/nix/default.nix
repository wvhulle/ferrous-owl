{
  lib,
  stdenv,
  rustPlatform,
  rustc,
  autoPatchelfHook,
  makeWrapper,
  patchelf,
  pkg-config,
  zlib,
  llvmPackages_19,
  pkgs ? import <nixpkgs> { },
  fetchFromGitHub ? pkgs.fetchFromGitHub,
}:
let
  meta = builtins.fromTOML (pkgs.lib.readFile ../../Cargo.toml);

  fenix = pkgs.callPackage (fetchFromGitHub {
    owner = "nix-community";
    repo = "fenix";
    rev = "b0fa429fc946e6e716dff3bfb97ce6383eae9359";
    hash = "sha256-YmnUYXjacFHa8fWCo8gBAHpqlcG8+P5+5YYFhy6hOkg=";
  }) { };

  toolchain = fenix.fromToolchainFile {
    file = ../../rust-toolchain.toml;
  };

  rustPlatform = pkgs.makeRustPlatform {
    cargo = toolchain;
    rustc = toolchain;
  };
in
rustPlatform.buildRustPackage {
  pname = meta.package.name;
  version = meta.package.version;

  src = pkgs.lib.cleanSource ../..;

  cargoLock = {
    lockFile = ../../Cargo.lock;
  };

  nativeBuildInputs = [
    pkg-config
    makeWrapper
    patchelf
    llvmPackages_19.llvm
  ]
  ++ lib.optionals stdenv.isLinux [ autoPatchelfHook ];

  buildInputs = [
    zlib
    llvmPackages_19.libllvm
    rustc.unwrapped
  ]
  ++ lib.optionals stdenv.isLinux [
    stdenv.cc.cc.lib
  ];

  # Tell autoPatchelfHook to skip rustowlc - we handle librustc_driver via LD_LIBRARY_PATH
  autoPatchelfIgnoreMissingDeps = [ "librustc_driver-*.so" ];

  env = {
    RUSTC_BOOTSTRAP = "1";
    # TOOLCHAIN_CHANNEL is used by build.rs when RUSTUP_TOOLCHAIN is not set
    TOOLCHAIN_CHANNEL = "stable";
    LLVM_CONFIG = "${llvmPackages_19.llvm.dev}/bin/llvm-config";
  };

  preBuild = ''
    export NIX_LDFLAGS="$NIX_LDFLAGS -L${llvmPackages_19.libllvm}/lib"
  '';

  postInstall = ''
    # Use nixpkgs rustc sysroot - it matches the rustc used to compile rustowlc
    sysroot="${rustc.unwrapped}"

    # RUSTOWL_SYSROOT tells rustowl to use the nixpkgs rustc sysroot directly,
    # skipping any toolchain download attempts
    wrapProgram $out/bin/rustowl \
      --set RUSTOWL_SYSROOT "$sysroot" \
      --prefix LD_LIBRARY_PATH : "${rustc.unwrapped}/lib"

    wrapProgram $out/bin/rustowlc \
      --prefix LD_LIBRARY_PATH : "${rustc.unwrapped}/lib"
  '';

  meta = with lib; {
    description = "Visualize ownership and lifetimes in Rust for debugging and optimization";
    homepage = meta.package.repository;
    license = licenses.mpl20;
    maintainers = [ ];
    platforms = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
  };
}
