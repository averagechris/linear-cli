{
  description = "Nix flake for linear-cli";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
        lib = pkgs.lib;
        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        package = cargoToml.package;
        commonRustArgs = {
          version = package.version;
          src = lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          buildInputs = lib.optionals pkgs.stdenv.isLinux (with pkgs; [
            openssl
          ]);
        };
        fetchUpstreamScript = ''
          # Refresh GitHub upstream refs in both git and jj. For jj, Git's
          # refs/remotes/upstream/master is exposed as the remote bookmark master@upstream.
          exec jj git fetch --remote upstream "$@"
        '';
        linkOpencodeSkillsScript = ''
          find_repo_root() {
            local dir
            dir="''${PWD}"

            while [[ "''${dir}" != "/" ]]; do
              if [[ -d "''${dir}/skills" && -f "''${dir}/flake.nix" ]]; then
                printf '%s\n' "''${dir}"
                return 0
              fi

              dir="$(dirname "''${dir}")"
            done

            printf 'Could not locate repository root from %s\n' "''${PWD}" >&2
            return 1
          }

          repo_root="$(find_repo_root)"
          source_dir="''${repo_root}/skills"
          target_dir="''${repo_root}/.opencode/skills"

          mkdir -p "''${target_dir}"

          shopt -s nullglob
          for skill_dir in "''${source_dir}"/*; do
            [[ -d "''${skill_dir}" && -f "''${skill_dir}/SKILL.md" ]] || continue

            skill_name="$(basename "''${skill_dir}")"
            target_path="''${target_dir}/''${skill_name}"

            if [[ -L "''${target_path}" ]]; then
              rm "''${target_path}"
            elif [[ -e "''${target_path}" ]]; then
              printf 'Skipping existing non-symlink path: %s\n' "''${target_path}" >&2
              continue
            fi

            ln -s "''${skill_dir}" "''${target_path}"
          done
        '';
        ciFmtScript = ''
          cargo fmt --all --check
          alejandra --check flake.nix
        '';
        ciClippyScript = ''
          cargo clippy --locked --all-targets -- -D warnings
        '';
        ciTestScript = ''
          cargo test --locked
        '';
        mkRepoScript = {
          name,
          runtimeInputs ? [],
          text,
        }:
          pkgs.writeShellApplication {
            inherit name runtimeInputs;
            inherit text;
          };
        linear-cli = pkgs.rustPlatform.buildRustPackage (commonRustArgs
          // {
            pname = package.name;
            doCheck = false;

            meta = lib.attrsets.filterAttrs (_: value: value != null) {
              description = package.description or null;
              homepage = package.homepage or package.repository or null;
              license =
                if (package.license or null) == "MIT"
                then lib.licenses.mit
                else null;
              mainProgram = package.name;
            };
          });
        fetch-upstream = mkRepoScript {
          name = "fetch-upstream";
          text = fetchUpstreamScript;
          runtimeInputs = with pkgs; [
            jujutsu
          ];
        };
        link-opencode-skills = mkRepoScript {
          name = "link-opencode-skills";
          text = linkOpencodeSkillsScript;
          runtimeInputs = with pkgs; [
            coreutils
          ];
        };
        ci-fmt = mkRepoScript {
          name = "ci-fmt";
          text = ciFmtScript;
          runtimeInputs = with pkgs; [
            alejandra
            cargo
            rustfmt
          ];
        };
        ci-clippy = mkRepoScript {
          name = "ci-clippy";
          text = ciClippyScript;
          runtimeInputs = with pkgs; [
            cargo
            clippy
            rustc
          ];
        };
        ci-test = mkRepoScript {
          name = "ci-test";
          text = ciTestScript;
          runtimeInputs = with pkgs; [
            cargo
            rustc
          ];
        };
        repo-scripts = pkgs.symlinkJoin {
          name = "${package.name}-scripts";
          paths = [
            ci-clippy
            ci-fmt
            ci-test
            fetch-upstream
            link-opencode-skills
          ];
        };
        test-check = pkgs.rustPlatform.buildRustPackage (commonRustArgs
          // {
            pname = "${package.name}-tests";
            doCheck = true;
            installPhase = ''
              mkdir -p "$out"
            '';
          });
        fmt-check =
          pkgs.runCommand "${package.name}-fmt-check" {
            nativeBuildInputs = [ci-fmt];
            src = lib.cleanSource ./.;
          } ''
            export HOME="$TMPDIR"
            cp -R "$src" source
            chmod -R +w source
            cd source
            ci-fmt
            mkdir -p "$out"
          '';
      in {
        packages.default = linear-cli;
        packages.ci-clippy = ci-clippy;
        packages.ci-fmt = ci-fmt;
        packages.ci-test = ci-test;
        packages.linear-cli = linear-cli;
        packages.fetch-upstream = fetch-upstream;
        packages.link-opencode-skills = link-opencode-skills;
        packages.scripts = repo-scripts;

        apps.default = flake-utils.lib.mkApp {
          drv = linear-cli;
        };
        apps.linear-cli = flake-utils.lib.mkApp {
          drv = linear-cli;
        };
        apps.ci-clippy = flake-utils.lib.mkApp {
          drv = ci-clippy;
        };
        apps.ci-fmt = flake-utils.lib.mkApp {
          drv = ci-fmt;
        };
        apps.ci-test = flake-utils.lib.mkApp {
          drv = ci-test;
        };
        apps.fetch-upstream = flake-utils.lib.mkApp {
          drv = fetch-upstream;
        };
        apps.link-opencode-skills = flake-utils.lib.mkApp {
          drv = link-opencode-skills;
        };

        checks = {
          build = linear-cli;
          fmt = fmt-check;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [linear-cli];
          packages = with pkgs; [
            alejandra
            cargo
            cargo-audit
            cargo-deny
            clippy
            jujutsu
            nixd
            pkg-config
            rust-analyzer
            rustc
            rustfmt
            repo-scripts
          ];
        };
      }
    )
    // {
      overlays.default = final: prev: {
        inherit (self.packages.${prev.system}) linear-cli;
      };
    };
}
