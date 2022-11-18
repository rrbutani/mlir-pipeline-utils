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

    batConfigDir = np.runCommandNoCC "bat-config" {} ''
      mkdir -p $out/syntaxes
      cp ${mlir-syntax.lib.mlir-syntax} $out/syntaxes/mlir.sublime-syntax

      touch $out/config # no custom configuration options for now..
    '';
    batCacheDir = np.runCommandNoCC "bat-cache" {
      nativeBuildInputs = [ np.bat ];
    } ''
      bat cache --build --source ${batConfigDir} --target $out
    '';

    # export BAT_CONFIG_PATH="/path/to/bat.conf"
    # export BAT_CACHE_PATH=""
    # --wrap=never
    # zstd stuff

    # check phase: check that MLIR is in the listed languages
    # test zstd handling
    bat = let
      drv = np.writeShellApplication {
        name = "mlir-bat";
        runtimeInputs = with np; [ zstd coreutils np.bat ];
        text = ''
          export BAT_CONFIG_PATH="${batConfigDir}/config"
          export BAT_CACHE_PATH="${batCacheDir}"

          if [[ ''${NIX_DEBUG-0} -gt 5 ]]; then
            echo "mlir-bat $*" >&2
            set -x
          fi

          # If we have more than 1 file or other options, bail on trying to
          # parse them.
          declare -a args=("$@")
          declare -a fileIdxs=()

          for ((i = 1; i <= $#; ++i)); do
            a=$i
            if ! [[ "''${!a}" =~ ^\-.* ]]; then
              fileIdxs+=("$i")
            fi
          done

          if [[ "''${#fileIdxs[@]}" == 1 ]]; then
            idx="''${fileIdxs[0]}"

            # shellcheck disable=SC2184
            unset args["$((idx - 1))"]

            filePath="''${!idx}"

            fileName="$(basename "$filePath")"
            fileName="''${fileName,,}"
            zstd=false
            declare -a extraArgs=()

            if [[ "''${fileName}" =~ .*\.zst$ ]]; then
              zstd=true
              fileName="$(basename "$fileName" .zst)"
            fi

            if [[ "''${fileName}" =~ .*\.mlir$ ]]; then
              extraArgs+=(--wrap never)
            fi

            if [[ $zstd == true ]]; then
              zstdcat "$filePath" | bat "''${extraArgs[@]}" "''${args[@]}" --file-name "$filePath" --ignored-suffix .zst
            else
              exec bat "''${extraArgs[@]}" "''${args[@]}" "$filePath"
            fi
          else
            exec bat "''${@}"
          fi
        '';
      };

      addChecks = drv: drv.overrideAttrs (old: {
        postCheck = old.postCheck or "" + ''
          chmod +x $out/bin/mlir-bat # See: https://github.com/NixOS/nixpkgs/pull/201721

          # test that MLIR is listed in the supported languages:
          $out/bin/mlir-bat -L | grep "MLIR:mlir"

          # test that zstd compressed files are automatically piped through
          # `zstdcat`:
          echo "test test test test " > test
          ${np.zstd}/bin/zstd test
          $out/bin/mlir-bat test.zst | grep "test test test test"
        '';
      });
    in addChecks drv;

    delta = let
      # We want to point `delta` at our bat cache dir.
      # `bat` has an env var for this: https://github.com/sharkdp/bat/blob/7c847d84b0c3c97df6badfbb39d153ad93aec74e/src/bin/bat/directories.rs#L43-L60
      #
      # Unfortunately `delta` does not use it: https://github.com/dandavison/delta/blob/afa7a1a38dc13ea480653938e6c54c933396515c/src/utils/bat/dirs.rs#L19-L30
      # (see: https://docs.rs/dirs-next/latest/dirs_next/fn.cache_dir.html)
      #
      # So we need this symlink tree which we have delta treat as
      # `XDG_CACHE_HOME`:
      batConfigForDelta = np.runCommandNoCC "bat-config-for-delta" { } ''
        mkdir -p $out
        ln -s ${batCacheDir} $out/bat
      '';
    in np.writeShellApplication {
      name = "delta";
      runtimeInputs = [ np.delta ];
      text = ''
        export XDG_CACHE_HOME="${batConfigForDelta}"
        exec delta "''${@}"
      '';
    };

    split = let

    in null;

    view = let

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
