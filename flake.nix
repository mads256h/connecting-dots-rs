{
  description = "Development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
  };

  outputs = {nixpkgs, ...}:
    let
    system = "x86_64-linux";
    pkgs = import nixpkgs { inherit system; };
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
        ];

        LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        RUST_BACKTRACE="full";
        RUST_LOG="debug";
      };
    };
}
