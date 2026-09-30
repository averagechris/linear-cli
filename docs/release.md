# Manual GitHub release

Fleet prepares and validates release refs locally. GitHub Actions only rebuilds the two public artifacts from a trusted annotated tag. A person verifies the bytes and publishes the GitHub Release. There is no PR workflow, automatic publisher, release secret, or Pages dispatch in this repository.

## 1. Check the release

Start from an empty jj working-copy commit whose parent, local `main`, and `main@origin` are identical:

```bash
nix run .#release -- --version X.Y.Z --check
```

The check is read-only. It accepts only the version and checks refs and version state. It does not run gates or build artifacts.

## 2. Run the local release

```bash
nix run .#release -- --version X.Y.Z
```

This prepares the version and changelog, runs fmt, clippy, tests, and `ci-skills-render`, then atomically pushes `main` and annotated `vX.Y.Z`. It does not create a GitHub Release or upload assets. Do not add `--submit-linux-build`; the GitHub backend rejects it.

## 3. Check the artifact run

The annotated tag push calls Fleet at `e31a02573d79dfeb2496fec6c21cf74a0ece4d79` for `aarch64-darwin` and `x86_64-linux`. The caller and reusable workflow have read-only repository permission.

If the tag-push run must be recovered, dispatch `Build release artifacts (manual publication required)` from `main` and supply the existing annotated tag. The validation job rejects dispatches from any other branch and tags that are not ancestors of the selected `main` commit.

Download both named Actions artifacts. For each platform, verify the sidecar and identity evidence:

```bash
sha256sum -c linear-cli-vX.Y.Z-x86_64-linux.tar.gz.sha256
shasum -a 256 -c linear-cli-vX.Y.Z-aarch64-darwin.tar.gz.sha256
git ls-remote --tags origin refs/tags/vX.Y.Z refs/tags/vX.Y.Z^{}
cat release-identity-aarch64-darwin
cat release-identity-x86_64-linux
```

Each identity file must contain the remote annotated tag object ID on line 1 and its peeled commit ID on line 2. The two files must agree. Publish the tarballs and `.sha256` files only.

## 4. Draft, verify, and publish

Create a draft before uploading bytes:

```bash
gh release create vX.Y.Z --repo averagechris/linear-cli --draft --verify-tag \
  --title 'linear-cli vX.Y.Z' --generate-notes
gh release upload vX.Y.Z --repo averagechris/linear-cli \
  linear-cli-vX.Y.Z-aarch64-darwin.tar.gz \
  linear-cli-vX.Y.Z-aarch64-darwin.tar.gz.sha256 \
  linear-cli-vX.Y.Z-x86_64-linux.tar.gz \
  linear-cli-vX.Y.Z-x86_64-linux.tar.gz.sha256
```

Download the draft assets into a clean directory and compare every byte and digest with the verified Actions downloads. Use the paginated release list to identify exactly one draft; the tag lookup is not reliable for drafts:

```bash
set -euo pipefail

expected_assets=(
  linear-cli-vX.Y.Z-aarch64-darwin.tar.gz
  linear-cli-vX.Y.Z-aarch64-darwin.tar.gz.sha256
  linear-cli-vX.Y.Z-x86_64-linux.tar.gz
  linear-cli-vX.Y.Z-x86_64-linux.tar.gz.sha256
)

release_ids="$(gh api --paginate 'repos/averagechris/linear-cli/releases?per_page=100' \
  --jq '.[] | select(.draft == true and .tag_name == "vX.Y.Z") | .id')"
test "$(printf '%s\n' "$release_ids" | awk 'NF { n++ } END { print n + 0 }')" -eq 1
release_id="$release_ids"

asset_rows="$(gh api --paginate \
  "repos/averagechris/linear-cli/releases/$release_id/assets?per_page=100" \
  --jq '.[] | [.id, .name, .digest] | @tsv')"
test "$(printf '%s\n' "$asset_rows" | awk 'NF { n++ } END { print n + 0 }')" -eq 4
test "$(printf '%s\n' "$asset_rows" | cut -f2 | sort)" = \
  "$(printf '%s\n' "${expected_assets[@]}" | sort)"

rm -rf published
mkdir published
while IFS=$'\t' read -r asset_id file _; do
  gh api "repos/averagechris/linear-cli/releases/assets/$asset_id" \
    -H 'Accept: application/octet-stream' > "published/$file"
done <<< "$asset_rows"

while IFS=$'\t' read -r _ file api_digest; do
  test -f "$file"
  test -f "published/$file"
  cmp "$file" "published/$file"
  local_digest="sha256:$(shasum -a 256 "$file" | awk '{print $1}')"
  downloaded_digest="sha256:$(shasum -a 256 "published/$file" | awk '{print $1}')"
  test -n "$api_digest"
  test "$api_digest" != null
  test "$local_digest" = "$downloaded_digest"
  test "$local_digest" = "$api_digest"
done <<< "$asset_rows"

remote_identity="$(git ls-remote --tags origin \
  'refs/tags/vX.Y.Z' 'refs/tags/vX.Y.Z^{}' | \
  awk '$2 == "refs/tags/vX.Y.Z" || $2 == "refs/tags/vX.Y.Z^{}" { print $1 }')"
test "$(printf '%s\n' "$remote_identity" | awk 'NF { n++ } END { print n + 0 }')" -eq 2
for identity in release-identity-aarch64-darwin release-identity-x86_64-linux; do
  test "$(sed -n '1,2p' "$identity")" = "$remote_identity"
done
```

The checks fail if there is not exactly one matching draft, the asset names are not exactly the four expected names, GitHub does not report a digest, any local/downloaded/API digest differs, or the remote annotated tag no longer matches both recorded identity files. Only then remove draft status:

```bash
gh release edit vX.Y.Z --repo averagechris/linear-cli --draft=false
```

After the public release, manually dispatch the existing Pages workflow with `gh workflow run pages.yml -R averagechris/averagechris.github.io --ref main -f project=linear-cli -f tag=vX.Y.Z -f sha=<peeled tag commit>`, then verify the live state and both the download and sidecar SHA. Do not edit the site registry or dispatch it automatically.

The `homebrew-artifact` output is separate from these public release assets. Follow [homebrew.md](homebrew.md) to update its formula and digest; do not silently substitute it for a Fleet tarball or upload it under a Fleet artifact name.
