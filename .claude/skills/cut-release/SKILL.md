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

In the tap repo, `Formula/screenly-cli.rb` needs exactly **one** line changed —
the tag:

```ruby
  url "https://github.com/Screenly/cli.git",
      tag: "v26.9.0"
```

There is deliberately **no `version` field**. Homebrew derives the version from
the tag and strips the leading `v`, so `tag: "v26.9.0"` yields version `26.9.0`
by itself. Keeping it derived means it cannot drift out of step with the tag.

Do **not** add a `version "v26.9.0"` line back. Homebrew tokenises a leading `v`
as a StringToken, which sorts below any NumericToken — so a `v`-prefixed version
compares as *older* than every bare one, and `brew audit` rejects it outright
([`FormulaAudit/Version`](https://github.com/Homebrew/brew/blob/master/Library/Homebrew/rubocops/version.rb)).
That ordering also makes the switch one-way: going back to a `v` prefix would
read as a downgrade and would not upgrade users.

Because `url` points at the git repo and pins a **tag** (not a release tarball),
there is no `sha256` to recompute. (Homebrew also wants a `revision:` alongside a
git `tag:` — but that cop is scoped to `homebrew-core` and does not apply to our
tap.) Open it as a PR on a `release-YY.M.MICRO` branch, same as upstream.

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

- **Docker Hub** (`screenly/cli`) — pushed by the release workflow as
  `screenly/cli:<tag>` and `:latest`. The image tag keeps the `v` (it is
  `$GITHUB_REF_NAME`). No manual step.
- **`.mcpb` bundles** — built and attached by the release workflow. No manual step.
- **`flake.nix` in this repo** — reads `version` straight out of `Cargo.toml` via
  `builtins.fromTOML`, so it follows step 1 automatically and can never drift.
  Do **not** add a version to it.
- **nixpkgs** (`nix-shell -p screenly-cli`) — a *different* package from our flake,
  living in `pkgs/by-name/sc/screenly-cli/package.nix` in `NixOS/nixpkgs`. It
  normally updates itself via `passthru.updateScript = nix-update-script {}`, which
  files r-ryantm PRs titled `screenly-cli: X -> Y`. **This is worth spot-checking at
  release time** — the bot has gone quiet before and left nixpkgs several releases
  behind:

  ```bash
  gh api repos/NixOS/nixpkgs/contents/pkgs/by-name/sc/screenly-cli/package.nix \
    --jq .content | base64 -d | grep '^  version'
  gh api "search/issues?q=repo:NixOS/nixpkgs+screenly-cli+in:title+type:pr" \
    --jq '.items[] | "\(.number) [\(.state)] \(.title)"' | head -5
  ```

  We do not own that repo, so closing a gap there means opening a PR against
  nixpkgs (bump `version`, refresh `hash` and `cargoHash`). Note it stores the
  version **bare** and builds the tag as `tag = "v${finalAttrs.version}"` — same
  convention as the Homebrew formula.
