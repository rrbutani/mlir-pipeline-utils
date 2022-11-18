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
  in {
  });
}

