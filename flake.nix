{
  description = "Screenly CLI flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
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
      overlays.default = final: _prev: {
        screenly-cli = final.rustPlatform.buildRustPackage rec {
          pname = "screenly-cli";
          version = "0.2.3";

          src = final.fetchFromGitHub {
            owner = "screenly";
            repo = "cli";
            rev = "refs/tags/v${version}";
            hash = "sha256-rQK1EYb1xYtcxq0Oj4eY9PCFMoaYinr42W8NkG36ps0=";
          };

          # This can be removed if/when the next tagged release contains the Cargo.lock.
          # The patch adds the correct Cargo.lock for v0.2.3 to the sources before the configure
          # phase.
          cargoPatches = [
            (final.fetchpatch {
              url = "https://gist.githubusercontent.com/jnsgruk/851afc2da1d18ca91e34677c7e72aebb/raw/517e82390f3aa9dbd95793563d7b442186a08940/Cargo.lock";
              hash = "sha256-Cqc1PHRhgS3zK19bSqpU2v+R3jSlOY6oaLJXpUy6+50=";
            })
          ];

          cargoHash = "sha256-TzJ56Wuk77qrxDLL17fYEj4i/YhAS6DRmjoqrzb+5AA=";

          nativeBuildInputs = with final; [ pkg-config perl ];

          buildInputs = with final; [
            openssl
          ] ++ final.lib.optionals stdenv.isDarwin [ CoreFoundation Security ];

          meta = {
            description = "Command Line Interface (CLI) for Screenly.";
            homepage = "https://github.com/Screenly/cli";
            license = final.lib.licenses.mit;
            mainProgram = "screenly";
            platforms = final.lib.platforms.unix;
            maintainers = with final.lib.maintainers; [ jnsgruk vpetersson ];
          };
        };
      };

      packages = forAllSystems (system: {
        inherit (pkgsForSystem system) screenly-cli;
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
