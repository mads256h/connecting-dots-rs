{
  description = "Development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {nixpkgs, rust-overlay, ...}:
    let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
        config.allowUnfree = true;
      };

      rustToolchain = pkgs.rust-bin.stable."1.88.0".default.override {
        extensions = [ "rust-src" "clippy" "rustfmt" ];
      };
    lib = pkgs.lib;
    in 
    {
      devShells.${system}.default = pkgs.mkShell rec {
        packages = with pkgs; [
          cargo
          rustc
          rustfmt
          rust-analyzer

          wayland
        ];

        buildInputs = with pkgs; [
          libxkbcommon
          libGL
          wayland
          wayland.dev
          glslang # or shaderc
          vulkan-tools
          vulkan-headers
          vulkan-loader
          vulkan-validation-layers
          pulseaudio
          pulseaudio.dev

          openssl
          pkg-config

          jetbrains.rust-rover
        ];

        nativeBuildInputs = with pkgs; [
          rustToolchain
        ];

        shellHook = ''
          mkdir -p ~/.rust-rover/toolchain

          ln -sfn ${rustToolchain}/lib ~/.rust-rover/toolchain
          ln -sfn ${rustToolchain}/bin ~/.rust-rover/toolchain

          export RUST_SRC_PATH="$HOME/.rust-rover/toolchain/lib/rustlib/src/rust/library"
        '';

        LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
        RUST_BACKTRACE="full";
        RUST_LOG="debug";
      };
    };
}
