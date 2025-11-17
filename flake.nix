{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      self,
      ...
    }:
    {
      overlays.default = (
        final: prev:
        let
          pkgs = import nixpkgs {
            system = final.system;
          };
          lib = nixpkgs.lib;
          rustPlatform = pkgs.rustPlatform;
        in
        {
          pulld = import ./nix/pulld.nix {
            inherit pkgs lib rustPlatform;
          };
        }
      );

      nixosModules.default = {
        nixpkgs.overlays = [ self.overlays.default ];
      };
    }
    // (flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ self.overlays.default ];
        };
      in
      {
        packages = {
          pulld = pkgs.pulld;
        };
      }
    ));
}
