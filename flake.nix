{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-24.05";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, ... }@inputs:
    inputs.flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [ (import inputs.rust-overlay) ];
        };

        rust = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" ];
          targets = [ "wasm32-unknown-unknown" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rust
            pkgs.lld
            pkgs.renderdoc
          ];
          # bunch of dynamically linked libs for wgpu
          LD_LIBRARY_PATH = with pkgs.xorg; with pkgs.lib.strings;
            concatStrings (intersperse ":" [
              "${pkgs.libxkbcommon}/lib"
              "${libXcursor}/lib"
              "${libX11}/lib"
              "${libXxf86vm}/lib"
              "${libXi}/lib"
              "${libXrandr}/lib"
              "${pkgs.vulkan-loader}/lib"
              "${pkgs.stdenv.cc.cc.lib}/lib64"
              "${pkgs.stdenv.cc.cc.lib}/lib64"
            ]);
        };
      });
}
