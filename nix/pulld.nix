{
  pkgs,
  lib,
  rustPlatform,
  ...
}:
rustPlatform.buildRustPackage rec {
  pname = "pulld";
  version = "0.1.0";
  src = ./..;

  buildInputs = [ pkgs.openssl ];

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  meta = with lib; {
    homepage = "https://github.com/phlmn/pulld";
    mainProgram = "pulld";
  };
}
