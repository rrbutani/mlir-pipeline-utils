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
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    mlir-syntax.url = github:rrbutani/sublime-mlir-syntax;
  };

  outputs = {
    self, flake-utils, nixpkgs, crane, mlir-syntax
  }: flake-utils.lib.eachDefaultSystem (system: let
    np = nixpkgs.legacyPackages.${system};
    craneLib = crane.lib.${system};

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

    rustPkgs = let
      commonArgs = {
        src = craneLib.cleanCargoSource ./.;
        nativeBuildInputs = np.lib.optional (np.stdenv.isDarwin) np.libiconv;
      };
      deps = craneLib.buildDepsOnly (commonArgs // {
        # Unfortunate...
        #
        # `buildDepsOnly` does a full build and leaves in the binary artifacts.
        #
        # Normally (when doing one hop from deps to the release drv) this is not
        # an issue but in our case, because we have two hops (lib, then
        # binaries), these binaries are symlinked into the binary drv's target
        # folder and cannot be overwritten.
        #
        # What we really want is to be able to elide `--all-targets` or better
        # yet to drop all artifacts from the top-level crate/workspace (or just
        # not build them in the first place).
        postBuild = ''
          rm target/$CARGO_PROFILE/{split,view}*
        '';
      });
      lib = craneLib.cargoBuild (commonArgs // {
        src = np.lib.cleanSourceWith {
          src = ./.;
          filter = path: _: let
            n = builtins.baseNameOf path;
          in n == "common.rs" || np.lib.hasPrefix "Cargo" n;
        };
        cargoArtifacts = deps;
        cargoExtraArgs = "--lib ";
        doInstallCargoArtifacts = true;
      });
      bin = name: craneLib.buildPackage (commonArgs // {
        cargoArtifacts = lib;
        pnameSuffix = "-" + name;
        cargoExtraArgs = "--bin=${name}";
      });

    in {
      split = bin "split";
      view = bin "view";

      clippy = craneLib.cargoClippy (commonArgs // {
        cargoArtifacts = lib;
        cargoClippyExtraArts = "--all-targets -- --deny warnings";
      });
      fmt = craneLib.cargoFmt commonArgs;
    };

    # TODO: wrappers with env-vars + rename to `mlir-`
    split = rustPkgs.split;
    view = rustPkgs.view;

  in rec {
    packages = {
      inherit bat delta split view;
    };

    apps =
      (builtins.mapAttrs
        (_: pkg: { type = "app"; program = np.lib.getExe pkg; })
        packages
      ) // { default = self.apps.${system}.split; };

    checks = {
      inherit (rustPkgs) clippy;
    } // np.lib.optionalAttrs (system == "x86_64-linux") {
      inherit (rustPkgs) fmt;
    };

    devShells = rec {
      default = playground;

      # All the binaries from this flake in a shell for you to play around with.
      playground = np.mkShell {
        packages = builtins.attrValues packages;
      };

      dev = np.mkShell {
        inputsFrom = (builtins.attrValues packages) ++ (builtins.attrValues checks);
        packages = (with packages; [ bat delta ]) ++ [ np.rust-analyzer np.rustc ];
      };
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
