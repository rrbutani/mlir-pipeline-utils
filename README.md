# `mlir-pipeline-utils`

## what

Tooling for managing the [output of `--mlir-print-ir-{before,after}-all`](https://mlir.llvm.org/docs/PassManagement/#ir-printing).

Specifically, this includes:
  - a tool to split pass log output into separate MLIR files (compressing them on the way)
  - a tool to step through a captured MLIR pass pipeline, with diffing

## how to use?

This repo is distributed as a [nix flake](https://nixos.wiki/wiki/Flakes); currently this is the only provided usage method (but you should be able to install the required deps and build the programs in this repo manually if you'd rather not [install `nix`](https://nixos.org/download.html)).

To run without installing: `nix run github:rrbutani/mlir-pipeline-utils`.
To install: `nix profile install github:rrbutani/mlir-pipeline-utils`.

### `mlir-pipeline-split` (`#split` in the flake)

> `mlir-pipeline-split [<output directory>]`

Reads a log file in on stdin and splits it into files within the specified output directory. Defaults to outputting to `dump` if no directory is specified.

The tool prints out passes as they are run on stderr.

i.e.:
```bash
mlir-opt example.mlir --mlir-print-ir-before-all \
    |& nix run github:rrbutani/mlir-pipeline-utils#split -- example-pipeline-dump
```

TODO: asciicinema

> **Warning**
> Pass output goes out on stderr which may contain other output (i.e. `llvm::errs() <<`) so the produced `.mlir.zst` files may not be semantically valid.


### `mlir-pipeline-view` (`.#view` in the flake)

> `mlir-pipeline-view [<output directory>]`

TODO: asciicinema

> **Note**
> The viewer works best with `--mlir-print-ir-before-all --mlir-print-ir-after-all --mlir-print-ir-module-scope --mlir-disable-threading --mlir-elide-elementsattrs-if-larger=50`.
>
> The heuristics the viewer uses to correlate IR from before/after a pass in the presence of nested pass pipelines assume that both `print-ir-before-all` and `print-ir-after-all` are enabled and that the entire module is printed (TODO: maybe not?).

TODO:
  - allow comparing IR at arbitrary points in the pipeline!

### `mlir-bat` (`.#bat` in the flake)

A [`bat`](https://github.com/sharkdp/bat) wrapper that uses [this MLIR grammar](https://github.com/rrbutani/sublime-mlir-syntax) and is `.zst` aware.

The viewer calls this internally but you can invoke this explicitly if you'd rather not install the MLIR grammar + pipe in from `zstdcat` yourself.

## anything else?

A few things:
  - Untested but nothing here is particularly MLIR specific; this *should* work with the corresponding LLVM pass pipeline options.
  - Works with `--mlir-print-ir-after=(...)` and `--mlir-print-ir-before=(...)` as well

devShell that has bat, etc. that's good for experimenting with stuff?
