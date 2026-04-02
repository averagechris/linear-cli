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
        releaseTagScript = ''
          if [[ $# -eq 1 && ( "$1" == "-h" || "$1" == "--help" ) ]]; then
            printf 'usage: %s [--revision REV]\n' "$0"
            printf 'Create and push a release tag from Cargo.toml version.\n\n'
            printf 'In jj repos, uses jj tag set + git push.\n'
            printf 'In plain git repos, uses git tag + git push.\n'
            exit 0
          fi

          revision="@"
          while [[ $# -gt 0 ]]; do
            case "$1" in
              --revision) revision="$2"; shift 2 ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; exit 1 ;;
            esac
          done

          repo_root="$(git rev-parse --show-toplevel)"

          version="$(${pkgs.python3}/bin/python3 -c '
          import pathlib, sys, tomllib
          path = pathlib.Path(sys.argv[1])
          with path.open("rb") as f:
              data = tomllib.load(f)
          version = data.get("package", {}).get("version")
          if not isinstance(version, str) or not version:
              raise SystemExit("Cargo.toml is missing package.version")
          print(version)
          ' "$repo_root/Cargo.toml")"

          if [[ "$version" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
            tag="v''${version#v}"
          else
            printf 'Cargo.toml version must be semver in the form X.Y.Z\n' >&2
            exit 1
          fi

          if git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; then
            printf 'local tag already exists: %s\n' "$tag" >&2
            exit 1
          fi

          if [[ -n "$(git ls-remote --tags origin "refs/tags/$tag" 2>/dev/null)" ]]; then
            printf 'remote tag already exists on origin: %s\n' "$tag" >&2
            exit 1
          fi

          if [[ -d .jj ]]; then
            jj tag set "$tag" --revision "$revision" --no-pager --color=never
            printf 'created tag %s via jj\n' "$tag"
          else
            if ! git diff --quiet || ! git diff --cached --quiet; then
              printf 'working tree must be clean before tagging\n' >&2
              exit 1
            fi
            git tag -a "$tag" HEAD -m "Release $tag"
            printf 'created annotated tag %s\n' "$tag"
          fi

          if ! git push origin "refs/tags/$tag"; then
            printf 'failed to push %s. run manually:\n  git push origin "refs/tags/%s"\n' "$tag" "$tag" >&2
            exit 1
          fi

          printf 'pushed %s to origin\n' "$tag"
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
        linear-cli-runtime-tools = with pkgs;
          [
            git
            gh
            jujutsu
            less
          ]
          ++ lib.optionals stdenv.isLinux [xdg-utils];
        linear-cli-bundled = pkgs.symlinkJoin {
          name = "${package.name}-bundled-${package.version}";
          paths = [linear-cli];
          nativeBuildInputs = [pkgs.makeWrapper];
          postBuild = ''
            wrapProgram "$out/bin/${package.name}" \
              --prefix PATH : ${lib.makeBinPath linear-cli-runtime-tools}
          '';
        };
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
        release-tag = mkRepoScript {
          name = "release-tag";
          text = releaseTagScript;
          runtimeInputs = with pkgs; [
            git
            jujutsu
            python3
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
            release-tag
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
        packages.linear-cli-bundled = linear-cli-bundled;
        packages.linear-cli = linear-cli;
        packages.fetch-upstream = fetch-upstream;
        packages.link-opencode-skills = link-opencode-skills;
        packages.release-tag = release-tag;
        packages.scripts = repo-scripts;

        apps.default = flake-utils.lib.mkApp {
          drv = linear-cli;
        };
        apps.linear-cli-bundled = flake-utils.lib.mkApp {
          drv = linear-cli-bundled;
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
        apps.release-tag = flake-utils.lib.mkApp {
          drv = release-tag;
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
