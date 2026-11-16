# Dev shell for sinterklaas. Enter with: nix develop
# Pure-Rust TLS (rustls) for both the database and SMTP, so no openssl.
{
  description = "sinterklaas — the family's Sinterklaas surprise";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (s: f nixpkgs.legacyPackages.${s});
    in {
      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rust-analyzer
            pkgs.rustfmt
            pkgs.clippy
            pkgs.mariadb # the ephemeral test database the gate brings up
          ];
        };
      });
    };
}
