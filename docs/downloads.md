# Hosted downloads

Static release downloads are published with SourceHut Pages at:

```text
https://averagechris.srht.site/linear-cli/
```

Primary installs remain package-manager based (`cargo install`, Nix, or Homebrew), but release tarballs are available for manual installs and checksum verification.

## Build local macOS artifact

On macOS:

```bash
nix build .#release-artifact --out-link result-release-artifact
mkdir -p dist/downloads
cp -p result-release-artifact/* dist/downloads/
```

This creates a deterministic tarball and checksum such as:

```text
dist/downloads/linear-cli-v1.2.8-aarch64-darwin.tar.gz
dist/downloads/linear-cli-v1.2.8-aarch64-darwin.tar.gz.sha256
```

## Build and publish Linux artifact on SourceHut

The SourceHut build manifest builds the Linux artifact, fetches existing hosted downloads from the current Pages manifest, regenerates the downloads page, and publishes it with `hut pages publish`. It uses build-scoped OAuth (`PAGES:RW`) rather than a checked-in token.

Submit the release build after updating `.builds/release-linux-x86_64.yml` for the new version, or let the git.sr.ht integration run it after pushing `main`:

```bash
hut builds submit .builds/release-linux-x86_64.yml \
  --note "linear-cli v1.2.8 linux release" \
  --tags "linear-cli/v1.2.8/release" \
  --visibility unlisted
```

The successful job still exposes the Linux tarball and checksum as build artifacts, but the durable download URLs are the copies published to SourceHut Pages.

## Build and publish pages

```bash
nix run .#build-pages
nix run .#publish-pages
```

To merge locally-built artifacts with artifacts already hosted on Pages before publishing:

```bash
nix run .#build-pages -- --include-existing-downloads
```

Defaults:

- domain: `averagechris.srht.site`
- subdirectory: `/linear-cli`

Override if needed:

```bash
nix run .#build-pages -- --domain example.com --subdirectory /linear-cli
nix run .#publish-pages -- --domain example.com --subdirectory /linear-cli
```

The generated pages archive is `dist/pages/linear-cli-pages.tar.gz` and contains `index.html`, `manifest.json`, and `downloads/`.

The helper commands above are flake-provided `writeShellApplication` outputs; there are no standalone release scripts to run directly from `scripts/`.
