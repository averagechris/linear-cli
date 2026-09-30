# Release downloads

Future release tarballs and SHA-256 sidecars are attached to manual GitHub Releases:

```text
https://github.com/averagechris/linear-cli/releases
```

The tag is the canonical release-version source. `linear update --check` reads GitHub tags and never downloads or installs a binary.

Fleet builds two standard artifacts from the annotated tag:

- `linear-cli-vX.Y.Z-aarch64-darwin.tar.gz`
- `linear-cli-vX.Y.Z-x86_64-linux.tar.gz`

Each tarball has a matching `.sha256` sidecar. The GitHub Actions artifacts also contain `release-identity-<platform>` evidence. Do not upload the identity files as public release assets.

The Darwin `homebrew-artifact` is a separate Nix output with a different archive layout and name. It remains the input to the Homebrew formula workflow described in [homebrew.md](homebrew.md). It is not one of the public Fleet release assets unless a release operator makes a separate, reviewed decision to add it.

See [release.md](release.md) for the complete release and publication procedure.
