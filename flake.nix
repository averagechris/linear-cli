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
        prepareReleaseScript = ''
          exec python3 - "$@" <<'PY'
          from __future__ import annotations

          import argparse
          import datetime as dt
          import pathlib
          import re
          import subprocess
          import sys
          import tomllib

          SEMVER_RE = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)$")

          def run(args: list[str], *, check: bool = True) -> str:
              completed = subprocess.run(args, check=check, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
              return completed.stdout

          def read_version(cargo_toml: pathlib.Path) -> str:
              with cargo_toml.open("rb") as handle:
                  version = tomllib.load(handle).get("package", {}).get("version")
              if not isinstance(version, str) or not SEMVER_RE.match(version):
                  raise SystemExit("Cargo.toml package.version must be X.Y.Z")
              return version

          def write_version(cargo_toml: pathlib.Path, version: str) -> None:
              if not SEMVER_RE.match(version):
                  raise SystemExit(f"invalid semver version: {version}")
              bare = version.removeprefix("v")
              text = cargo_toml.read_text()
              updated, count = re.subn(r'(?m)^version = "[^"]+"$', f'version = "{bare}"', text, count=1)
              if count != 1:
                  raise SystemExit("could not update package.version in Cargo.toml")
              cargo_toml.write_text(updated)

          def update_linux_build_manifest(repo_root: pathlib.Path, version: str) -> None:
              manifest = repo_root / ".builds" / "release-linux-x86_64.yml"
              if not manifest.exists():
                  return
              tag = f"v{version.removeprefix('v')}"
              text = manifest.read_text()
              text = re.sub(
                  r"linear-cli-v\d+\.\d+\.\d+-x86_64-linux\.tar\.gz",
                  f"linear-cli-{tag}-x86_64-linux.tar.gz",
                  text,
              )
              manifest.write_text(text)

          def update_lockfile_version(repo_root: pathlib.Path, version: str) -> None:
              lockfile = repo_root / "Cargo.lock"
              if not lockfile.exists():
                  return
              text = lockfile.read_text()
              updated, count = re.subn(
                  r'(\[\[package\]\]\nname = "linear-cli"\nversion = ")([^"]+)(")',
                  rf'\g<1>{version.removeprefix("v")}\3',
                  text,
                  count=1,
              )
              if count != 1:
                  raise SystemExit("could not update linear-cli version in Cargo.lock")
              lockfile.write_text(updated)

          def version_key(version: str) -> tuple[int, int, int]:
              match = SEMVER_RE.match(version)
              if not match:
                  raise ValueError(version)
              return tuple(int(part) for part in match.groups())

          def latest_tag_before(version: str) -> str | None:
              tags = []
              for line in run(["jj", "tag", "list", "--no-pager", "--color=never"], check=False).splitlines():
                  name = line.split(":", 1)[0].strip()
                  if SEMVER_RE.match(name) and version_key(name) < version_key(version):
                      tags.append(name)
              if not tags:
                  return None
              return sorted(tags, key=version_key)[-1]

          def commit_summaries(baseline: str | None, revision: str) -> list[str]:
              revset = f"{baseline}::{revision}" if baseline else revision
              output = run(
                  [
                      "jj", "log", "-r", revset, "--no-graph", "--color=never",
                      "-T", 'description.first_line() ++ "\\n"',
                  ],
                  check=False,
              )
              ignored = re.compile(r"^(chore|build)(\([^)]*\))?: (bump version|release)\b", re.IGNORECASE)
              return [line.strip() for line in output.splitlines() if line.strip() and not ignored.search(line)]

          def bullet_from_commit(summary: str) -> tuple[str, str]:
              match = re.match(r"^(?P<type>[a-z]+)(?:\([^)]*\))?(?P<breaking>!)?:\s*(?P<body>.+)$", summary)
              if not match:
                  return "Changed", summary[0].upper() + summary[1:]
              kind = match.group("type")
              body = match.group("body")
              sentence = body[0].upper() + body[1:]
              if match.group("breaking"):
                  return "Breaking", sentence
              if kind == "feat":
                  return "Added", sentence
              if kind == "fix":
                  return "Fixed", sentence
              if kind == "docs":
                  return "Documentation", sentence
              if kind == "perf":
                  return "Performance", sentence
              return "Changed", sentence

          def generated_changelog(commits: list[str]) -> str:
              if not commits:
                  return "### Changed\n\n- Maintenance release.\n"
              sections: dict[str, list[str]] = {}
              for summary in commits:
                  section, bullet = bullet_from_commit(summary)
                  sections.setdefault(section, []).append(bullet.rstrip("."))
              order = ["Breaking", "Added", "Changed", "Fixed", "Performance", "Documentation"]
              parts: list[str] = []
              for section in order:
                  bullets = sections.get(section)
                  if not bullets:
                      continue
                  parts.append(f"### {section}\n")
                  parts.extend(f"- {bullet}." for bullet in bullets)
                  parts.append("")
              return "\n".join(parts).rstrip() + "\n"

          def unreleased_body(text: str) -> tuple[tuple[int, int] | None, str]:
              match = re.search(r"(?m)^## Unreleased\s*$", text)
              if not match:
                  return None, ""
              next_heading = re.search(r"(?m)^## ", text[match.end():])
              body_start = match.end()
              body_end = match.end() + next_heading.start() if next_heading else len(text)
              return (body_start, body_end), text[body_start:body_end].strip()

          def update_changelog(changelog: pathlib.Path, version: str, date: str, commits: list[str]) -> None:
              tag = f"v{version.removeprefix('v')}"
              entry_heading = f"## {tag} - {date}"
              if not changelog.exists():
                  changelog.write_text(f"# Changelog\n\n## Unreleased\n\n{entry_heading}\n\n{generated_changelog(commits)}")
                  return
              text = changelog.read_text()
              if re.search(rf"(?m)^## {re.escape(tag)}(?:\s+-\s+.*)?$", text):
                  return
              body_range, body = unreleased_body(text)
              entry_body = body if body else generated_changelog(commits).strip()
              entry = f"{entry_heading}\n\n{entry_body}\n"
              if body_range:
                  start, end = body_range
                  text = text[:start] + f"\n\n{entry}\n" + text[end:].lstrip("\n")
              else:
                  if not text.startswith("# Changelog"):
                      text = "# Changelog\n\n" + text
                  text = text.rstrip() + f"\n\n## Unreleased\n\n{entry}"
              changelog.write_text(text.rstrip() + "\n")

          def main() -> int:
              parser = argparse.ArgumentParser(description="Prepare Cargo.toml and CHANGELOG.md for a deterministic release.")
              parser.add_argument("--version", help="release version to write; defaults to Cargo.toml package.version")
              parser.add_argument("--revision", default="@", help="jj revision to summarize for the changelog")
              parser.add_argument("--date", default=dt.date.today().isoformat(), help="release date for CHANGELOG.md")
              parser.add_argument("--repo-root", default=".", help="repository root")
              args = parser.parse_args()
              repo_root = pathlib.Path(args.repo_root).resolve()
              cargo_toml = repo_root / "Cargo.toml"
              version = args.version.removeprefix("v") if args.version else read_version(cargo_toml)
              if args.version:
                  write_version(cargo_toml, version)
              update_lockfile_version(repo_root, version)
              update_linux_build_manifest(repo_root, version)
              baseline = latest_tag_before(version)
              commits = commit_summaries(baseline, args.revision)
              update_changelog(repo_root / "CHANGELOG.md", version, args.date, commits)
              print(f"prepared {version}")
              if baseline:
                  print(f"baseline: {baseline}")
              print(f"changelog commits: {len(commits)}")
              return 0

          sys.exit(main())
          PY
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
            if [[ "$(jj log -r "$revision" --no-graph --color=never -T 'empty()' 2>/dev/null)" == "true" ]]; then
              printf 'refusing to tag empty jj revision: %s\n' "$revision" >&2
              printf 'pass the non-empty release revision explicitly (for example --revision @-)\n' >&2
              exit 1
            fi
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
        buildPagesScript = ''
          repo_root="$(git rev-parse --show-toplevel)"
          cd "$repo_root"

          domain="averagechris.srht.site"
          subdirectory="/linear-cli"

          while [[ $# -gt 0 ]]; do
            case "$1" in
              --domain) domain="$2"; shift 2 ;;
              --subdirectory) subdirectory="$2"; shift 2 ;;
              -h|--help)
                printf 'usage: build-pages [--domain DOMAIN] [--subdirectory PATH]\n'
                exit 0
                ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; exit 1 ;;
            esac
          done

          export LINEAR_PAGES_DOMAIN="$domain"
          export LINEAR_PAGES_SUBDIRECTORY="$subdirectory"

          exec python3 - <<'PY'
          from __future__ import annotations

          import html
          import json
          import os
          import pathlib
          import re
          import shutil
          import subprocess
          import tomllib

          repo = pathlib.Path.cwd()
          download_dir = repo / "dist" / "downloads"
          site_dir = repo / "dist" / "pages" / "site"
          pages_tarball = repo / "dist" / "pages" / "linear-cli-pages.tar.gz"

          with (repo / "Cargo.toml").open("rb") as handle:
              version = tomllib.load(handle)["package"]["version"]
          tag = f"v{version}"
          domain = os.environ["LINEAR_PAGES_DOMAIN"].rstrip("/")
          subdirectory = "/" + os.environ["LINEAR_PAGES_SUBDIRECTORY"].strip("/")
          base_url = f"https://{domain}{subdirectory}"

          artifacts = sorted(download_dir.glob("*.tar.gz"), reverse=True)
          if not artifacts:
              raise SystemExit(f"no download artifacts found in {download_dir}; run nix build .#release-artifact first")

          if site_dir.exists():
              shutil.rmtree(site_dir)
          (site_dir / "downloads").mkdir(parents=True)
          pages_tarball.parent.mkdir(parents=True, exist_ok=True)

          for path in sorted(download_dir.iterdir()):
              if path.is_file():
                  shutil.copy2(path, site_dir / "downloads" / path.name)

          def current_changelog() -> str:
              path = repo / "CHANGELOG.md"
              if not path.exists():
                  return "- See the tagged commit history for this release."
              text = path.read_text()
              match = re.search(rf"(?m)^## {re.escape(tag)}(?:\s+-\s+.*)?\s*$", text)
              if not match:
                  return "- See the tagged commit history for this release."
              next_match = re.search(r"(?m)^## ", text[match.end():])
              end = match.end() + next_match.start() if next_match else len(text)
              body = text[match.end():end].strip()
              return body or "- Maintenance release."

          def markdownish_to_html(markdown: str) -> str:
              lines = markdown.splitlines()
              out: list[str] = []
              in_list = False
              for line in lines:
                  if line.startswith("### "):
                      if in_list:
                          out.append("</ul>")
                          in_list = False
                      out.append(f"<h3>{html.escape(line[4:])}</h3>")
                  elif line.startswith("- "):
                      if not in_list:
                          out.append("<ul>")
                          in_list = True
                      out.append(f"<li>{html.escape(line[2:])}</li>")
                  elif line.strip():
                      if in_list:
                          out.append("</ul>")
                          in_list = False
                      out.append(f"<p>{html.escape(line.strip())}</p>")
              if in_list:
                  out.append("</ul>")
              return "\n".join(out)

          latest = artifacts[0]
          latest_checksum = latest.name + ".sha256"
          artifact_cards = []
          manifest = {"version": tag, "artifacts": []}
          for artifact in artifacts:
              checksum_path = download_dir / f"{artifact.name}.sha256"
              if not checksum_path.exists():
                  raise SystemExit(f"missing checksum for {artifact.name}: {checksum_path}")
              sha = checksum_path.read_text().split()[0]
              artifact_cards.append(f"""
            <div class="artifact">
              <h3>{html.escape(artifact.name)}</h3>
              <ul>
                <li><a href="downloads/{html.escape(artifact.name)}">Download tarball</a></li>
                <li><a href="downloads/{html.escape(artifact.name)}.sha256">SHA-256 checksum</a></li>
              </ul>
              <pre><code>{html.escape(sha)}  {html.escape(artifact.name)}</code></pre>
            </div>""")
              manifest["artifacts"].append({
                  "name": artifact.name,
                  "url": f"{base_url}/downloads/{artifact.name}",
                  "sha256": sha,
              })

          (site_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
          (site_dir / "index.html").write_text(f"""<!doctype html>
          <html lang="en">
          <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <title>linear-cli downloads</title>
            <style>
              body {{ font-family: system-ui, sans-serif; max-width: 820px; margin: 3rem auto; padding: 0 1rem; line-height: 1.5; }}
              code, pre {{ background: #f4f4f4; padding: 0.15rem 0.3rem; border-radius: 4px; }}
              pre {{ padding: 1rem; overflow-x: auto; }}
              .artifact {{ margin: 1rem 0; padding: 1rem; border: 1px solid #ddd; border-radius: 8px; }}
            </style>
          </head>
          <body>
            <h1>linear-cli downloads</h1>
            <p>A hardened Rust CLI for Linear.app.</p>
            <p><a href="https://git.sr.ht/~averagechris/linear-cli">Source repository</a> · <a href="https://crates.io/crates/linear-cli">Crates.io</a></p>

            <h2>What's new in {html.escape(tag)}</h2>
            {markdownish_to_html(current_changelog())}

            <h2>Binary downloads</h2>
            <p>Current packaged version: <code>{html.escape(tag)}</code></p>
            {"".join(artifact_cards)}

            <h2>Manual install</h2>
            <pre><code>curl -LO {html.escape(base_url)}/downloads/{html.escape(latest.name)}
          curl -LO {html.escape(base_url)}/downloads/{html.escape(latest_checksum)}
          sha256sum -c {html.escape(latest_checksum)}
          tar -xzf {html.escape(latest.name)}
          install -m 0755 {html.escape(latest.name.removesuffix('.tar.gz'))}/linear ~/.local/bin/linear</code></pre>

            <h2>Nix install</h2>
            <pre><code>nix run sourcehut:averagechris/linear-cli
          nix profile install sourcehut:averagechris/linear-cli</code></pre>
          </body>
          </html>
          """)

          subprocess.run([
              "tar", "--sort=name", "--format=ustar", "--mtime=@1", "--owner=0", "--group=0", "--numeric-owner",
              "-C", str(site_dir), "-czf", str(pages_tarball), "."
          ], check=True)
          print(f"created {pages_tarball}")
          PY
        '';
        publishPagesScript = ''
          repo_root="$(git rev-parse --show-toplevel)"
          cd "$repo_root"

          domain="averagechris.srht.site"
          subdirectory="/linear-cli"

          while [[ $# -gt 0 ]]; do
            case "$1" in
              --domain) domain="$2"; shift 2 ;;
              --subdirectory) subdirectory="$2"; shift 2 ;;
              -h|--help)
                printf 'usage: publish-pages [--domain DOMAIN] [--subdirectory PATH]\n'
                exit 0
                ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; exit 1 ;;
            esac
          done

          pages_tarball="$repo_root/dist/pages/linear-cli-pages.tar.gz"
          if [[ ! -f "$pages_tarball" ]]; then
            printf 'pages tarball not found: %s\nrun nix run .#build-pages first\n' "$pages_tarball" >&2
            exit 1
          fi

          exec hut pages publish "$pages_tarball" --domain "$domain" --subdirectory "$subdirectory"
        '';
        releaseScript = ''
          repo_root="$(git rev-parse --show-toplevel)"
          cd "$repo_root"

          version=""
          revision="@"
          validate=1
          tag_release=1
          build_artifact=1
          build_pages=1
          publish_pages=0
          submit_linux_build=0
          domain="averagechris.srht.site"
          subdirectory="/linear-cli"
          linux_manifest=".builds/release-linux-x86_64.yml"

          usage() {
            cat <<'EOF'
          usage: release [options]

          Prepare changelog/version metadata, validate, tag, build local release artifacts,
          build SourceHut Pages content, and optionally publish/submit Linux builds.

          Options:
            --version X.Y.Z            update Cargo.toml before preparing the release
            --revision REV             jj revision to tag/summarize (default: @)
            --skip-validate            skip nix flake check, ci-test, and ci-clippy
            --skip-tag                 do not create/push the release tag
            --skip-artifact            do not build/copy the local release artifact
            --skip-pages               do not build the static downloads page
            --publish-pages            publish dist/pages/linear-cli-pages.tar.gz with hut
            --submit-linux-build       submit .builds/release-linux-x86_64.yml with hut
            --domain DOMAIN            SourceHut Pages domain (default: averagechris.srht.site)
            --subdirectory PATH        SourceHut Pages subdirectory (default: /linear-cli)
            -h, --help                 show this help
          EOF
          }

          while [[ $# -gt 0 ]]; do
            case "$1" in
              --version) version="$2"; shift 2 ;;
              --revision) revision="$2"; shift 2 ;;
              --skip-validate) validate=0; shift ;;
              --skip-tag) tag_release=0; shift ;;
              --skip-artifact) build_artifact=0; shift ;;
              --skip-pages) build_pages=0; shift ;;
              --publish-pages) publish_pages=1; shift ;;
              --submit-linux-build) submit_linux_build=1; shift ;;
              --domain) domain="$2"; shift 2 ;;
              --subdirectory) subdirectory="$2"; shift 2 ;;
              -h|--help) usage; exit 0 ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; usage >&2; exit 1 ;;
            esac
          done

          prepare_args=("--revision" "$revision" "--repo-root" "$repo_root")
          if [[ -n "$version" ]]; then
            prepare_args+=("--version" "$version")
          fi
          prepare-release "''${prepare_args[@]}"

          cargo check --locked --quiet

          version="$(python3 - <<'PY'
          import pathlib, tomllib
          with pathlib.Path('Cargo.toml').open('rb') as handle:
              print(tomllib.load(handle)['package']['version'])
          PY
          )"
          tag="v$version"

          if [[ $validate -eq 1 ]]; then
            nix flake check --no-write-lock-file
            nix run .#ci-test
            nix run .#ci-clippy
          fi

          if [[ $tag_release -eq 1 ]]; then
            nix run .#release-tag -- --revision "$revision"
          fi

          if [[ $build_artifact -eq 1 ]]; then
            nix build .#release-artifact --out-link result-release-artifact
            mkdir -p dist/downloads
            cp -p result-release-artifact/* dist/downloads/
            printf 'copied release artifact(s) to dist/downloads\n'
          fi

          if [[ $build_pages -eq 1 ]]; then
            nix run .#build-pages -- --domain "$domain" --subdirectory "$subdirectory"
          fi

          if [[ $publish_pages -eq 1 ]]; then
            nix run .#publish-pages -- --domain "$domain" --subdirectory "$subdirectory"
          else
            printf 'pages not published; run: nix run .#publish-pages -- --domain %q --subdirectory %q\n' "$domain" "$subdirectory"
          fi

          if [[ $submit_linux_build -eq 1 ]]; then
            hut builds submit "$linux_manifest" --note "linear-cli $tag linux release" --tags "linear-cli/$tag/release" --visibility unlisted
          else
            printf 'linux build not submitted; run: hut builds submit %s --note %q --tags %q --visibility unlisted\n' \
              "$linux_manifest" "linear-cli $tag linux release" "linear-cli/$tag/release"
          fi
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
        releaseArtifactPlatform =
          if pkgs.stdenv.hostPlatform.isDarwin && pkgs.stdenv.hostPlatform.isAarch64
          then "aarch64-darwin"
          else if pkgs.stdenv.hostPlatform.isDarwin && pkgs.stdenv.hostPlatform.isx86_64
          then "x86_64-darwin"
          else if pkgs.stdenv.hostPlatform.isLinux && pkgs.stdenv.hostPlatform.isAarch64
          then "aarch64-linux"
          else if pkgs.stdenv.hostPlatform.isLinux && pkgs.stdenv.hostPlatform.isx86_64
          then "x86_64-linux"
          else null;
        releaseArtifactName =
          if releaseArtifactPlatform == null
          then null
          else "linear-cli-v${package.version}-${releaseArtifactPlatform}.tar.gz";
        releaseArtifact =
          if releaseArtifactName == null
          then null
          else
            pkgs.runCommand "linear-cli-release-artifact-${package.version}-${releaseArtifactPlatform}" {
              nativeBuildInputs = with pkgs; [
                coreutils
                gnutar
                gzip
              ];
            } ''
              mkdir -p "$out" "$TMPDIR/stage/${lib.removeSuffix ".tar.gz" releaseArtifactName}"
              stage="$TMPDIR/stage/${lib.removeSuffix ".tar.gz" releaseArtifactName}"

              cp -p ${linear}/bin/${cliProgram} "$stage/${cliProgram}"
              chmod 0555 "$stage/${cliProgram}"
              cp -p ${./README.md} "$stage/README.md"
              cp -p ${./LICENSE} "$stage/LICENSE"
              cp -p ${./CHANGELOG.md} "$stage/CHANGELOG.md"

              tar \
                --sort=name \
                --format=ustar \
                --mtime='@1' \
                --owner=0 \
                --group=0 \
                --numeric-owner \
                -C "$TMPDIR/stage" \
                -cf - \
                "${lib.removeSuffix ".tar.gz" releaseArtifactName}" | gzip -n > "$out/${releaseArtifactName}"

              sha="$(sha256sum "$out/${releaseArtifactName}" | cut -d ' ' -f1)"
              printf '%s  %s\n' "$sha" "${releaseArtifactName}" > "$out/${releaseArtifactName}.sha256"
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
        prepare-release = mkRepoScript {
          name = "prepare-release";
          text = prepareReleaseScript;
          runtimeInputs = with pkgs; [
            jujutsu
            python3
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
        build-pages = mkRepoScript {
          name = "build-pages";
          text = buildPagesScript;
          runtimeInputs = with pkgs; [
            cargo
            coreutils
            git
            gnutar
            python3
          ];
        };
        publish-pages = mkRepoScript {
          name = "publish-pages";
          text = publishPagesScript;
          runtimeInputs = with pkgs; [
            git
            hut
          ];
        };
        release = mkRepoScript {
          name = "release";
          text = releaseScript;
          runtimeInputs = with pkgs; [
            coreutils
            git
            hut
            jujutsu
            nix
            prepare-release
            python3
          ];
        };
        repo-scripts = pkgs.symlinkJoin {
          name = "${package.name}-scripts";
          paths = [
            build-pages
            ci-clippy
            ci-fmt
            ci-skills-render
            ci-test
            fetch-upstream
            link-opencode-skills
            prepare-release
            publish-pages
            release
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
            build-pages = build-pages;
            prepare-release = prepare-release;
            publish-pages = publish-pages;
            release = release;
            release-tag = release-tag;
            scripts = repo-scripts;
          }
          // lib.optionalAttrs (homebrewArtifact != null) {
            "homebrew-artifact" = homebrewArtifact;
          }
          // lib.optionalAttrs (releaseArtifact != null) {
            "release-artifact" = releaseArtifact;
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
        apps.build-pages = flake-utils.lib.mkApp {
          drv = build-pages;
        };
        apps.prepare-release = flake-utils.lib.mkApp {
          drv = prepare-release;
        };
        apps.publish-pages = flake-utils.lib.mkApp {
          drv = publish-pages;
        };
        apps.release = flake-utils.lib.mkApp {
          drv = release;
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
          }
          // lib.optionalAttrs (releaseArtifact != null) {
            "release-artifact" = releaseArtifact;
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
