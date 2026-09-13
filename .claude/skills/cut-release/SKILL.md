---
name: cut-release
description: Step-by-step runbook for cutting a tagged Screenly CLI release — pick the CalVer version, bump it across Cargo.toml/Cargo.lock/action.yml/Dockerfile, merge, tag to trigger the release workflow, then sync the Screenly/homebrew-screenly-cli tap. Use when the task is to ship a new versioned release, or to check whether the Homebrew tap has drifted behind the latest tag.
---

# Cut a Screenly CLI release

Versions are **CalVer `YY.M.MICRO`** — two-digit year, month, and a micro that
starts at `0` for the first release of a given month.

> **The release is not done when the tag is pushed.** Pushing the tag builds the
> binaries, but `brew install screenly-cli` keeps serving the *previous* version
> until [`Screenly/homebrew-screenly-cli`](https://github.com/Screenly/homebrew-screenly-cli)
> is bumped too. That tap lives in a separate repo with no automation pointing at
> it, so nothing fails loudly when it is skipped — it just silently drifts.
> **Step 4 is part of the release, not a follow-up.**

## 0. Pick the version

- Two-digit year + month + micro. A new month resets MICRO to `0` (the release
  after `26.8.2` is `26.9.0`, not `26.8.3`).
- The month is **not** zero-padded: `26.8.0`, never `26.08.0`. `Cargo.toml`'s
  `version` is parsed as strict [SemVer](https://semver.org/), which forbids a
  leading zero in a numeric component.
- Check what already exists for this month before picking MICRO:

```bash
git tag -l "v$(date +%y).$(date +%-m).*"
```

## 1. Bump the version (four files)

Branch as `release-YY.M.MICRO` (e.g. `release-26.9.0`). Note that the spelling
differs per file — `Cargo.toml` is bare, the other two are `v`-prefixed:

1. `Cargo.toml` → `version = "26.9.0"` (**no** `v`)
2. `action.yml` → `cli_version` input, `default: "v26.9.0"` (**with** `v`)
3. `Dockerfile` → `ARG RELEASE=v26.9.0` (**with** `v`)
4. `Cargo.lock` → run `cargo build` to update the `screenly` package entry;
   do not hand-edit it.

Do **not** touch `mcpb/manifest.json`. Its `"version": "0.0.0"` is a deliberate
placeholder — the release workflow injects the real version from the tag with
`jq`, so the tag stays the single source of truth.

Sanity check before committing — these should be the only version edits:

```bash
git diff -- Cargo.toml action.yml Dockerfile
grep -n -A1 'name = "screenly"' Cargo.lock
```

## 2. PR → merge to master

Open a PR from the release branch to `master`, let CI pass, and merge once
approved.

## 3. Tag master → the release workflow runs

Make sure you are on `master` with the merge pulled, then:

```bash
git tag vYY.M.MICRO
git push origin vYY.M.MICRO
```

`.github/workflows/release.yml` triggers on `v*` tags and does the rest: builds
every target (Linux gnu/musl/arm64/armhf, macOS x86_64 + aarch64, Windows 32/64),
packs the `.mcpb` bundles for macOS and Windows, publishes the GitHub release with
all artifacts, attests build provenance, and pushes `screenly/cli:<tag>` plus
`screenly/cli:latest` to Docker Hub.

Then add release notes to the GitHub release description.

CI is not run-owned — kick it off and check back; don't block on it.

## 4. Sync the Homebrew tap — REQUIRED

[`Screenly/homebrew-screenly-cli`](https://github.com/Screenly/homebrew-screenly-cli)
serves `brew tap screenly/screenly-cli && brew install screenly-cli`. It is a
separate repo, so it does not move on its own.

Confirm the drift, then close it:

```bash
# what the tap currently pins vs. what we just shipped
gh release view --repo Screenly/cli --json tagName -q .tagName
```

In the tap repo, `Formula/screenly-cli.rb` needs **both** fields moved to the new
tag:

```ruby
  url "https://github.com/Screenly/cli.git",
      tag: "v26.9.0"
  version "v26.9.0"
```

The `version` field keeps the `v` prefix here — that is the established
convention in this formula's history; leave it alone rather than "fixing" it in a
release PR.

Because `url` points at the git repo and pins a **tag** (not a release tarball),
there is no `sha256` to recompute. Open it as a PR on a `release-YY.M.MICRO`
branch, same as upstream.

Verify the tag is actually pushed before bumping the formula, or `brew install`
will fail to fetch it:

```bash
git ls-remote --tags https://github.com/Screenly/cli.git vYY.M.MICRO
```

## 5. Verify

- `gh release view vYY.M.MICRO --repo Screenly/cli` — artifacts attached.
- `grep 'tag:' Formula/screenly-cli.rb` in the tap — matches the new tag.
- The two must agree. If they don't, the release is incomplete.

## Drift check (any time, outside a release)

This is worth running on its own — the tap fell a full release behind before:

```bash
gh release view --repo Screenly/cli --json tagName -q .tagName
gh api repos/Screenly/homebrew-screenly-cli/contents/Formula/screenly-cli.rb \
  --jq .content | base64 -d | grep 'tag:'
```

## Other distribution channels

These do **not** need a manual step, but know where they come from:

- **Docker Hub** (`screenly/cli`) — pushed by the release workflow.
- **Nix** (`nix-shell -p screenly-cli`) — packaged in nixpkgs upstream, updated by
  nixpkgs maintainers, not by us.
- **`.mcpb` bundles** — built and attached by the release workflow.
