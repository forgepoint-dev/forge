{
  inputs = {
    devenv.url = "github:cachix/devenv";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs = {
      nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{
      flake-parts,
      nixpkgs,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        inputs.devenv.flakeModule
        inputs.treefmt-nix.flakeModule
      ];
      systems = nixpkgs.lib.systems.flakeExposed;

      perSystem =
        {
          config,
          self',
          inputs',
          pkgs,
          system,
          ...
        }:
        {
          devenv.shells.default = {
            languages.javascript = {
              enable = true;
              bun.enable = true;
              npm.enable = true;
            };

            languages.nix = {
              enable = true;
              lsp.package = pkgs.nixd;
            };

            languages.rust = {
              enable = true;
              channel = "stable";
              components = [
                "cargo"
                "clippy"
                "rust-analyzer"
                "rustc"
                "rustfmt"
              ];

              mold.enable = true;
            };
          };

          treefmt = {
            projectRootFile = "flake.nix";

            programs.nixfmt.enable = true;

            programs.biome = {
              enable = true;
              formatCommand = "format";
            };

            programs.rustfmt.enable = true;
          };
        };
    };
}
