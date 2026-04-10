{
  description = "Nix flake for linear";

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
        cliConfig = fromTOML (builtins.readFile ./config/cli.toml);
        package = cargoToml.package;
        cliProgram = cliConfig.cli.program_name;
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
          usage() {
            printf 'Usage: %s [--global | --project]\n\n' "$0"
            printf '  --project  Symlink skills into .opencode/skills/ in the repo root (default)\n'
            printf '  --global   Symlink skills into ~/.config/opencode/skills/\n'
          }

          mode="project"
          while [[ $# -gt 0 ]]; do
            case "$1" in
              --global)  mode="global"; shift ;;
              --project) mode="project"; shift ;;
              -h|--help) usage; exit 0 ;;
              *) printf 'Unknown argument: %s\n' "$1" >&2; usage >&2; exit 1 ;;
            esac
          done

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

          if [[ "''${mode}" == "global" ]]; then
            target_dir="''${XDG_CONFIG_HOME:-''${HOME}/.config}/opencode/skills"
          else
            target_dir="''${repo_root}/.opencode/skills"
          fi

          mkdir -p "''${target_dir}"

          shopt -s nullglob
          linked=0
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
            linked=$((linked + 1))
          done

          printf 'Linked %d skills into %s\n' "''${linked}" "''${target_dir}"
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
        ciSkillsRenderScript = ''
          exec bash ./scripts/render-skills.sh --check "$@"
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
        linear = pkgs.rustPlatform.buildRustPackage (commonRustArgs
          // {
            pname = cliProgram;
            doCheck = false;
            postInstall = ''
              mv "$out/bin/${package.name}" "$out/bin/${cliProgram}"
            '';

            meta = lib.attrsets.filterAttrs (_: value: value != null) {
              description = package.description or null;
              homepage = package.homepage or package.repository or null;
              license =
                if (package.license or null) == "MIT"
                then lib.licenses.mit
                else null;
              mainProgram = cliProgram;
            };
          });
        homebrewArtifactPlatform =
          if pkgs.stdenv.hostPlatform.isDarwin && pkgs.stdenv.hostPlatform.isAarch64
          then "darwin-arm64"
          else if pkgs.stdenv.hostPlatform.isDarwin && pkgs.stdenv.hostPlatform.isx86_64
          then "darwin-amd64"
          else null;
        homebrewArtifactName =
          if homebrewArtifactPlatform == null
          then null
          else "linear-cli-v${package.version}-${homebrewArtifactPlatform}.tar.gz";
        homebrewArtifact =
          if homebrewArtifactName == null
          then null
          else
            pkgs.runCommand "linear-cli-homebrew-artifact-${package.version}" {
              nativeBuildInputs = with pkgs; [
                coreutils
                gnutar
                gzip
              ];
            } ''
              mkdir -p "$out" "$TMPDIR/stage"
              cp -p ${linear}/bin/${cliProgram} "$TMPDIR/stage/${cliProgram}"
              chmod 0555 "$TMPDIR/stage/${cliProgram}"

              tar \
                --sort=name \
                --format=ustar \
                --mtime='@1' \
                --owner=0 \
                --group=0 \
                --numeric-owner \
                -C "$TMPDIR/stage" \
                -cf - \
                ${cliProgram} | gzip -n > "$out/${homebrewArtifactName}"

              sha="$(sha256sum "$out/${homebrewArtifactName}" | cut -d ' ' -f1)"
              printf '%s  %s\n' "$sha" "${homebrewArtifactName}" > "$out/${homebrewArtifactName}.sha256"
            '';
        linear-runtime-tools = with pkgs;
          [
            git
            gh
            jujutsu
            less
          ]
          ++ lib.optionals stdenv.isLinux [xdg-utils];
        linear-bundled = pkgs.symlinkJoin {
          name = "${cliProgram}-bundled-${package.version}";
          paths = [linear];
          nativeBuildInputs = [pkgs.makeWrapper];
          postBuild = ''
            wrapProgram "$out/bin/${cliProgram}" \
              --prefix PATH : ${lib.makeBinPath linear-runtime-tools}
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
        ci-skills-render = mkRepoScript {
          name = "ci-skills-render";
          text = ciSkillsRenderScript;
          runtimeInputs = with pkgs; [
            bash
            coreutils
            diffutils
            findutils
            gnused
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
            ci-skills-render
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
        skills-check =
          pkgs.runCommand "${package.name}-skills-check" {
            nativeBuildInputs = [ci-skills-render];
            src = lib.cleanSource ./.;
          } ''
            export HOME="$TMPDIR"
            cp -R "$src" source
            chmod -R +w source
            cd source
            ci-skills-render
            mkdir -p "$out"
          '';
      in {
        packages =
          {
            default = linear;
            ci-clippy = ci-clippy;
            ci-fmt = ci-fmt;
            ci-skills-render = ci-skills-render;
            ci-test = ci-test;
            linear-bundled = linear-bundled;
            linear = linear;
            fetch-upstream = fetch-upstream;
            link-opencode-skills = link-opencode-skills;
            release-tag = release-tag;
            scripts = repo-scripts;
          }
          // lib.optionalAttrs (homebrewArtifact != null) {
            "homebrew-artifact" = homebrewArtifact;
          };

        apps.default = flake-utils.lib.mkApp {
          drv = linear;
          exePath = "/bin/${cliProgram}";
        };
        apps.linear-bundled = flake-utils.lib.mkApp {
          drv = linear-bundled;
          exePath = "/bin/${cliProgram}";
        };
        apps.linear = flake-utils.lib.mkApp {
          drv = linear;
          exePath = "/bin/${cliProgram}";
        };
        apps.ci-clippy = flake-utils.lib.mkApp {
          drv = ci-clippy;
        };
        apps.ci-fmt = flake-utils.lib.mkApp {
          drv = ci-fmt;
        };
        apps.ci-skills-render = flake-utils.lib.mkApp {
          drv = ci-skills-render;
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

        checks =
          {
            build = linear;
            fmt = fmt-check;
            skills = skills-check;
          }
          // lib.optionalAttrs (homebrewArtifact != null) {
            "homebrew-artifact" = homebrewArtifact;
          };

        devShells.default = pkgs.mkShell {
          inputsFrom = [linear];
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
      lib.opencodeSkills = let
        skillsDir = ./skills;
        entries = builtins.readDir skillsDir;
        isSkill = name: type:
          type == "directory" && builtins.pathExists (skillsDir + "/${name}/SKILL.md");
        skillNames = builtins.filter (name: isSkill name entries.${name}) (builtins.attrNames entries);
      in
        builtins.listToAttrs (map (name: {
            inherit name;
            value = builtins.readFile (skillsDir + "/${name}/SKILL.md");
          })
          skillNames);

      overlays.default = final: prev: {
        inherit (self.packages.${prev.system}) linear;
      };
    };
}
