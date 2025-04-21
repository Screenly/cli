{
  description = "Screenly CLI flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
  };

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

      pkgsForSystem = system: (import nixpkgs {
        inherit system;
        overlays = [ self.overlays.default ];
      });
    in
    {
      overlays.default = final: _prev:
        let
          inherit ((builtins.fromTOML (builtins.readFile ./Cargo.toml)).package) version;
          inherit (final) openssl perl pkg-config stdenv lib darwin rustPlatform;
        in
        {
          screenly-cli = rustPlatform.buildRustPackage rec {
            inherit version;
            pname = "screenly-cli";

            src = lib.cleanSource ./.;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [
              pkg-config
              perl
            ];

            buildInputs = [
              openssl
            ] ++ lib.optionals stdenv.isDarwin [
              darwin.apple_sdk.frameworks.CoreFoundation
              darwin.apple_sdk.frameworks.CoreServices
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.SystemConfiguration
            ];

            meta = {
              description = "Command Line Interface (CLI) for Screenly.";
              homepage = "https://www.screenly.io/developers/cli/";
              license = lib.licenses.mit;
              mainProgram = "screenly";
              platforms = lib.platforms.unix;
              maintainers = with lib.maintainers; [ jnsgruk vpetersson ];
            };
          };
        };

      packages = forAllSystems (system: rec {
        inherit (pkgsForSystem system) screenly-cli;
        default = screenly-cli;
      });

      devShells = forAllSystems (system: {
        default = (pkgsForSystem system).mkShell {
          name = "screenly-cli";
          NIX_CONFIG = "experimental-features = nix-command flakes";
          inputsFrom = [ self.packages.${system}.screenly-cli ];
          shellHook = "exec $SHELL";
        };
      });
    };
}
