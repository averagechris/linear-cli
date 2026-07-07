{
  description = "Nix flake for linear";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fleet.url = "git+https://git.sr.ht/~averagechris/averagechris.srht.site";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    fleet,
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
        ciSkillsRenderScript = ''
          exec bash ./scripts/render-skills.sh --check "$@"
        '';
        buildPagesScript = ''
          repo_root="$(git rev-parse --show-toplevel)"
          cd "$repo_root"

          domain="averagechris.srht.site"
          subdirectory="/linear-cli"
          include_existing_downloads=0

          while [[ $# -gt 0 ]]; do
            case "$1" in
              --domain) domain="$2"; shift 2 ;;
              --subdirectory) subdirectory="$2"; shift 2 ;;
              --include-existing-downloads) include_existing_downloads=1; shift ;;
              -h|--help)
                printf 'usage: build-pages [--domain DOMAIN] [--subdirectory PATH] [--include-existing-downloads]\n'
                exit 0
                ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; exit 1 ;;
            esac
          done

          export LINEAR_PAGES_DOMAIN="$domain"
          export LINEAR_PAGES_SUBDIRECTORY="$subdirectory"
          export LINEAR_INCLUDE_EXISTING_DOWNLOADS="$include_existing_downloads"

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
          import urllib.error
          import urllib.request

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
          include_existing_downloads = os.environ["LINEAR_INCLUDE_EXISTING_DOWNLOADS"] == "1"

          download_dir.mkdir(parents=True, exist_ok=True)

          def include_existing_downloads_from_pages() -> None:
              manifest_url = f"{base_url}/manifest.json"
              try:
                  with urllib.request.urlopen(manifest_url, timeout=30) as response:
                      manifest = json.load(response)
              except urllib.error.HTTPError as error:
                  if error.code == 404:
                      return
                  raise
              except urllib.error.URLError as error:
                  raise SystemExit(f"failed to fetch existing downloads manifest {manifest_url}: {error}") from error

              for artifact in manifest.get("artifacts", []):
                  name = artifact.get("name")
                  url = artifact.get("url")
                  if not isinstance(name, str) or not isinstance(url, str):
                      continue
                  artifact_path = download_dir / name
                  checksum_path = download_dir / f"{name}.sha256"
                  if not artifact_path.exists():
                      print(f"fetching existing download {name}")
                      urllib.request.urlretrieve(url, artifact_path)
                  if not checksum_path.exists():
                      sha = artifact.get("sha256")
                      if isinstance(sha, str) and sha:
                          checksum_path.write_text(f"{sha}  {name}\n")
                      else:
                          urllib.request.urlretrieve(f"{url}.sha256", checksum_path)

          if include_existing_downloads:
              include_existing_downloads_from_pages()

          def artifact_sort_key(path: pathlib.Path) -> tuple[int, int, int, str]:
              match = re.match(r"linear-cli-v(\d+)\.(\d+)\.(\d+)-(.+)\.tar\.gz$", path.name)
              if not match:
                  return (-1, -1, -1, path.name)
              major, minor, patch, platform = match.groups()
              return (int(major), int(minor), int(patch), platform)

          artifacts = sorted(download_dir.glob("*.tar.gz"), key=artifact_sort_key, reverse=True)
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

          def artifact_info(path: pathlib.Path) -> dict[str, str]:
              match = re.match(r"linear-cli-(v\d+\.\d+\.\d+)-(.+)\.tar\.gz$", path.name)
              if not match:
                  return {"version": "other", "platform": path.name.removesuffix(".tar.gz")}
              release_version, platform = match.groups()
              return {"version": release_version, "platform": platform}

          def platform_label(platform: str) -> str:
              labels = {
                  "aarch64-darwin": "macOS Apple silicon",
                  "x86_64-linux": "Linux x86_64",
              }
              return labels.get(platform, platform.replace("-", " "))

          def build_count_label(count: int) -> str:
              return f"{count} build" if count == 1 else f"{count} builds"

          latest = artifacts[0]
          latest_checksum = latest.name + ".sha256"
          artifact_groups: dict[str, list[dict[str, str]]] = {}
          manifest = {"version": tag, "artifacts": []}
          for artifact in artifacts:
              checksum_path = download_dir / f"{artifact.name}.sha256"
              if not checksum_path.exists():
                  raise SystemExit(f"missing checksum for {artifact.name}: {checksum_path}")
              sha = checksum_path.read_text().split()[0]
              info = artifact_info(artifact)
              artifact_groups.setdefault(info["version"], []).append({
                  "name": artifact.name,
                  "platform": info["platform"],
                  "sha": sha,
              })
              manifest["artifacts"].append({
                  "name": artifact.name,
                  "url": f"{base_url}/downloads/{artifact.name}",
                  "sha256": sha,
              })

          latest_version = tag if tag in artifact_groups else artifact_info(latest)["version"]

          def render_build(build: dict[str, str]) -> str:
              name = build["name"]
              sha = build["sha"]
              return f"""
                <article class="build">
                  <h4>{html.escape(platform_label(build['platform']))}</h4>
                  <p class="filename"><code>{html.escape(name)}</code></p>
                  <p class="download-links">
                    <a class="primary-link" href="downloads/{html.escape(name)}">Download tarball</a>
                    <a href="downloads/{html.escape(name)}.sha256">Checksum</a>
                  </p>
                  <details>
                    <summary>SHA-256</summary>
                    <pre><code>{html.escape(sha)}  {html.escape(name)}</code></pre>
                  </details>
                </article>"""

          def render_release(release_version: str, builds: list[dict[str, str]], *, latest_release: bool) -> str:
              builds_html = "".join(render_build(build) for build in builds)
              label = "Latest release" if latest_release else "Release"
              latest_badge = "<span class=\"badge\">Latest</span>" if latest_release else ""
              title_html = f"{html.escape(release_version)} {latest_badge}" if latest_release else html.escape(release_version)
              class_names = "release latest" if latest_release else "release"
              return f"""
              <section class="{class_names}">
                <div class="release-heading">
                  <div>
                    <p class="eyebrow">{label}</p>
                    <h3>{title_html}</h3>
                  </div>
                  <span class="build-count">{html.escape(build_count_label(len(builds)))}</span>
                </div>
                <div class="build-grid">
                  {builds_html}
                </div>
              </section>"""

          latest_downloads = render_release(latest_version, artifact_groups[latest_version], latest_release=True)
          previous_downloads = "".join(
              render_release(release_version, builds, latest_release=False)
              for release_version, builds in artifact_groups.items()
              if release_version != latest_version
          )
          previous_downloads_section = f"""
            <h3 class="previous-heading">Previous releases</h3>
            {previous_downloads}""" if previous_downloads else ""

          (site_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
          (site_dir / "index.html").write_text(f"""<!doctype html>
          <html lang="en">
          <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <title>linear-cli downloads</title>
            <script>
              (function () {{
                var stored = null;
                try {{ stored = localStorage.getItem("theme"); }} catch (e) {{}}
                var system = matchMedia("(prefers-color-scheme: dark)").matches ? "moon" : "dawn";
                document.documentElement.dataset.theme = stored || system;
              }})();
            </script>
            <style>
              /* Rosé Pine Dawn */
              :root, :root[data-theme="dawn"] {{
                --base: #faf4ed;
                --surface: #fffaf3;
                --overlay: #f2e9e1;
                --hl-med: #dfdad9;
                --muted: #9893a5;
                --subtle: #797593;
                --text: #575279;
                --love: #b4637a;
                --rose: #d7827e;
                --pine: #286983;
                --foam: #56949f;
              }}
              /* Rosé Pine Moon */
              :root[data-theme="moon"] {{
                --base: #232136;
                --surface: #2a273f;
                --overlay: #393552;
                --hl-med: #44415a;
                --muted: #6e6a86;
                --subtle: #908caa;
                --text: #e0def4;
                --love: #eb6f92;
                --rose: #ea9a97;
                --pine: #3e8fb0;
                --foam: #9ccfd8;
              }}
              * {{ box-sizing: border-box; }}
              body {{
                background: var(--base);
                color: var(--text);
                font-family: Charter, Georgia, "Iowan Old Style", serif;
                line-height: 1.65;
                max-width: 920px;
                margin: 0 auto;
                padding: 3rem 1.25rem 4rem;
                transition: background 0.25s ease, color 0.25s ease;
              }}
              a {{ color: var(--pine); text-decoration-color: color-mix(in srgb, var(--pine) 40%, transparent); }}
              a:hover {{ color: var(--rose); }}
              .masthead {{ display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; }}
              h1 {{ font-size: 2rem; margin: 0; font-weight: 700; letter-spacing: -0.01em; }}
              .home-link {{ color: var(--subtle); font-style: italic; margin: 0.25rem 0 0; font-size: 0.95rem; }}
              .theme-toggle {{
                background: var(--surface); border: 1px solid var(--hl-med); color: var(--subtle);
                border-radius: 999px; padding: 0.3rem 0.8rem; cursor: pointer;
                font-family: inherit; font-size: 0.85rem; font-style: italic;
                transition: border-color 0.15s ease;
                flex-shrink: 0; margin-top: 0.5rem;
              }}
              .theme-toggle:hover {{ border-color: var(--rose); color: var(--text); }}
              h2 {{ font-size: 1.5rem; margin: 2.25rem 0 1rem; font-weight: 700; }}
              h3 {{ font-size: 1.15rem; }}
              code, pre {{
                font-family: ui-monospace, Menlo, monospace;
                background: var(--overlay); border-radius: 4px; padding: 0.15rem 0.3rem;
              }}
              pre {{ padding: 1rem; overflow-x: auto; }}
              pre code {{ background: none; padding: 0; }}
              .release {{
                background: var(--surface); border: 1px solid var(--hl-med); border-radius: 4px;
                padding: 1.25rem 1.4rem; margin: 1rem 0 1.5rem;
                box-shadow: 2px 2px 0 var(--hl-med);
              }}
              .release.latest {{ border-color: var(--foam); }}
              .release-heading {{ display: flex; justify-content: space-between; gap: 1rem; align-items: flex-start; margin-bottom: 1rem; }}
              .release-heading h3 {{ margin: 0.1rem 0 0; font-family: ui-monospace, Menlo, monospace; }}
              .eyebrow {{ color: var(--muted); font-size: 0.8rem; font-weight: 700; letter-spacing: 0.06em; margin: 0; text-transform: uppercase; }}
              .badge, .build-count {{
                border-radius: 999px; display: inline-block; font-size: 0.78rem; font-weight: 700;
                padding: 0.15rem 0.55rem; white-space: nowrap;
                font-family: ui-monospace, Menlo, monospace;
              }}
              .badge {{ background: var(--foam); color: var(--base); margin-left: 0.35rem; vertical-align: middle; }}
              .build-count {{ background: var(--overlay); color: var(--subtle); }}
              .build-grid {{ display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); }}
              .build {{ background: var(--base); border: 1px solid var(--hl-med); border-radius: 4px; padding: 1rem; }}
              .build h4 {{ margin: 0 0 0.5rem; font-size: 0.95rem; }}
              .filename {{ margin: 0 0 0.75rem; overflow-wrap: anywhere; font-size: 0.9rem; }}
              .download-links {{ display: flex; flex-wrap: wrap; gap: 0.75rem; margin: 0.75rem 0; font-family: ui-monospace, Menlo, monospace; font-size: 0.9rem; }}
              .primary-link {{ font-weight: 700; }}
              details summary {{ cursor: pointer; color: var(--subtle); }}
              details pre {{ margin-bottom: 0; }}
              .previous-heading {{ margin-top: 2rem; }}
              footer, .footer {{ color: var(--muted); font-size: 0.88rem; margin-top: 3.5rem; font-style: italic; text-align: center; }}
            </style>
          </head>
          <body>
            <div class="masthead">
              <div>
                <h1>linear-cli downloads</h1>
                <p class="home-link"><a href="https://averagechris.srht.site/">~averagechris</a> / linear-cli</p>
              </div>
              <button class="theme-toggle" id="theme-toggle" aria-label="toggle color theme">dawn &frasl; moon</button>
            </div>
            <p>A hardened Rust CLI for Linear.app.</p>
            <p><a href="https://git.sr.ht/~averagechris/linear-cli">Source repository</a></p>

            <h2>What's new in {html.escape(tag)}</h2>
            {markdownish_to_html(current_changelog())}

            <h2>Binary downloads</h2>
            <p>Choose the build for your platform. The latest release is highlighted first; older releases are grouped below by version.</p>
            {latest_downloads}
            {previous_downloads_section}

            <h2>Manual install</h2>
            <pre><code>curl -LO {html.escape(base_url)}/downloads/{html.escape(latest.name)}
          curl -LO {html.escape(base_url)}/downloads/{html.escape(latest_checksum)}
          sha256sum -c {html.escape(latest_checksum)}
          tar -xzf {html.escape(latest.name)}
          install -m 0755 {html.escape(latest.name.removesuffix('.tar.gz'))}/linear ~/.local/bin/linear</code></pre>

            <h2>Nix install</h2>
            <pre><code>nix run sourcehut:averagechris/linear-cli
          nix profile install sourcehut:averagechris/linear-cli</code></pre>

            <script>
              document.getElementById("theme-toggle").addEventListener("click", function () {{
                var next = document.documentElement.dataset.theme === "moon" ? "dawn" : "moon";
                document.documentElement.dataset.theme = next;
                try {{ localStorage.setItem("theme", next); }} catch (e) {{}}
              }});
              matchMedia("(prefers-color-scheme: dark)").addEventListener("change", function (event) {{
                var stored = null;
                try {{ stored = localStorage.getItem("theme"); }} catch (e) {{}}
                if (!stored) document.documentElement.dataset.theme = event.matches ? "moon" : "dawn";
              }});
            </script>
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
              if [[ ! -e "$out/bin/${cliProgram}" ]]; then
                mv "$out/bin/${package.name}" "$out/bin/${cliProgram}"
              fi
              ln -s "${cliProgram}" "$out/bin/${package.name}"
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
        fleetApps = fleet.lib.fleet.presets.rust {
          inherit pkgs self;
          pname = "linear-cli";
          binaries = ["linear"];
          subdir = "linear-cli";
          srhtRepo = "linear-cli";
          versionMode = "package";
          versionFile = "Cargo.toml";
          lockPackages = ["linear-cli"];
          ciExtraInputs = lib.optionals pkgs.stdenv.isLinux [
            (pkgs.writeShellApplication {
              name = "pkg-config";
              text = ''
                export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
                exec ${pkgs.pkg-config}/bin/pkg-config "$@"
              '';
            })
          ];
        };
        fleetAppPackage = name:
          pkgs.writeShellApplication {
            inherit name;
            text = ''
              exec ${fleetApps.apps.${name}.program} "$@"
            '';
          };
        ci-fmt = fleetAppPackage "ci-fmt";
        ci-clippy = fleetAppPackage "ci-clippy";
        ci-test = fleetAppPackage "ci-test";
        static-checks = fleetAppPackage "static-checks";
        prepare-release = fleetAppPackage "prepare-release";
        release-tag = fleetAppPackage "release-tag";
        release = fleetAppPackage "release";
        releaseArtifact = fleetApps.releaseArtifact system;
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
            static-checks
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
        # `nix fmt` invokes the formatter app without path arguments. Alejandra
        # treats no arguments as "format stdin", which fails on empty stdin, so
        # keep the formatter as Alejandra but default it to formatting the repo.
        nix-formatter = pkgs.writeShellApplication {
          name = "alejandra";
          runtimeInputs = with pkgs; [
            alejandra
          ];
          text = ''
            if [[ $# -eq 0 ]]; then
              exec alejandra .
            fi

            exec alejandra "$@"
          '';
        };
      in {
        formatter = nix-formatter;

        packages =
          {
            default = linear;
            ci-clippy = ci-clippy;
            ci-fmt = ci-fmt;
            ci-skills-render = ci-skills-render;
            ci-test = ci-test;
            linear-bundled = linear-bundled;
            linear = linear;
            linear-cli-bundled = linear-bundled;
            linear-cli = linear;
            fetch-upstream = fetch-upstream;
            link-opencode-skills = link-opencode-skills;
            build-pages = build-pages;
            prepare-release = prepare-release;
            publish-pages = publish-pages;
            release = release;
            release-tag = release-tag;
            scripts = repo-scripts;
            static-checks = static-checks;
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
        apps.linear-cli = flake-utils.lib.mkApp {
          drv = linear;
          exePath = "/bin/${cliProgram}";
        };
        apps.linear-cli-bundled = flake-utils.lib.mkApp {
          drv = linear-bundled;
          exePath = "/bin/${cliProgram}";
        };
        apps.ci-clippy = fleetApps.apps.ci-clippy;
        apps.ci-fmt = fleetApps.apps.ci-fmt;
        apps.ci-skills-render = flake-utils.lib.mkApp {
          drv = ci-skills-render;
        };
        apps.ci-test = fleetApps.apps.ci-test;
        apps.fetch-upstream = flake-utils.lib.mkApp {
          drv = fetch-upstream;
        };
        apps.link-opencode-skills = flake-utils.lib.mkApp {
          drv = link-opencode-skills;
        };
        apps.build-pages = flake-utils.lib.mkApp {
          drv = build-pages;
        };
        apps.prepare-release = fleetApps.apps.prepare-release;
        apps.publish-pages = flake-utils.lib.mkApp {
          drv = publish-pages;
        };
        apps.release = fleetApps.apps.release;
        apps.release-tag = fleetApps.apps.release-tag;
        apps.static-checks = fleetApps.apps.static-checks;

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
            cargo-edit
            cargo-machete
            cargo-nextest
            cargo-outdated
            cargo-semver-checks
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
