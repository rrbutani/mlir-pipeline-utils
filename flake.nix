{
  description = "utilities for viewing MLIR pass pipeline logs";
  nixConfig = {
    extra-substituters = [
      "https://cache.garnix.io"
    ];
    extra-trusted-public-keys = [
      "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g="
    ];
  };

  inputs = {
    flake-utils.url = github:numtide/flake-utils;
    nixpkgs.url = github:NixOS/nixpkgs/nixos-unstable;
    mlir-syntax.url = github:rrbutani/sublime-mlir-syntax;
  };

  outputs = {
    self, flake-utils, nixpkgs, mlir-syntax
  }: flake-utils.lib.eachDefaultSystem (system: let
    np = nixpkgs.legacyPackages.${system};

    batConfigDir = null;
    batCacheDir = null;

    # export BAT_CONFIG_PATH="/path/to/bat.conf"
    # export BAT_CACHE_PATH=""
    # --wrap=never
    # zstd stuff

    # check phase: check that MLIR is in the listed languages
    # test zstd handling
    bat = let
    in null;

    delta = let
    in null;
  in rec {
    packages = { inherit bat delta; };

    apps =
      (builtins.mapAttrs
        (_: pkg: { type = "app"; program = np.lib.getExe pkg; })
        packages
      );

    checks = {
      lint = let
        sources = builtins.path {
          path = ./.;
          name = "shell-sources";
          filter = p: _: np.lib.hasSuffix (builtins.baseNameOf p) ".sh";
        };
      in np.runCommand "lint" {
        nativeBuildInputs = [ np.shellcheck ];
      } "shellcheck ${sources}/*.sh";
    };

    devShells.default = np.mkShell {
      inputsFrom = [ checks.lint ];
      packages = builtins.attrValues packages;
    };
  });
}

# TODO bat:
#   - wrapper with the theme
#   - nowrap
#   - transparently handle .zst files correctly

# TODO: delta
#   - use our bat

# TODO: use zstd adaptive compression in split
