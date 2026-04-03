# Homebrew release artifacts

This repo can produce prebuilt release archives for any Homebrew tap that wants to package `linear-cli`.

## Artifact contract

On supported Darwin hosts, the tap should consume a tarball with one of these names:

- `linear-cli-vX.Y.Z-darwin-arm64.tar.gz`
- `linear-cli-vX.Y.Z-darwin-amd64.tar.gz`

Build it with:

```bash
nix build .#homebrew-artifact
```

The build output appears as a `result/` directory containing the tarball and checksum. The tarball currently contains a single executable at the archive root:

- `linear`

The recommended Homebrew formula install stanza is:

```ruby
def install
  bin.install "linear"
  bin.install_symlink "linear" => "linear-cli"
end
```

That keeps `linear` as the primary command while preserving `linear-cli` as a compatibility alias.

## How the artifact is produced

The `homebrew-artifact` package is a first-class Nix derivation. It:

1. builds the Nix `linear` package
2. packages the resulting binary into a tarball named for the current supported Darwin platform
3. writes a matching `.sha256` file next to that tarball

Supported platforms today:

- Apple Silicon macOS → `darwin-arm64`
- Intel macOS → `darwin-amd64`

`homebrew-artifact` is only exposed on those Darwin targets; Linux builds intentionally do not define it.

Because the artifact is produced by a derivation, the tarball and checksum come from the Nix store and do not depend on a separate local packaging script.

## Publishing checklist

1. Update `Cargo.toml` version.
2. Create and push the release tag:

   ```bash
   nix run .#release-tag
   ```

3. Build the Homebrew artifact:

   ```bash
   nix build .#homebrew-artifact
   ```

4. Upload the tarball and matching `.sha256` file from `result/` to the release host your tap references.
5. Update your tap's `Formula/linear-cli.rb` with the new `url`, `version`, and `sha256`.
6. Validate the formula in the tap repo with `brew audit` and a local install test.
