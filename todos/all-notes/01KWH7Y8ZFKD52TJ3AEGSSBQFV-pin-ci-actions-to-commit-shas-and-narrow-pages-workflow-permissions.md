---
title: "Pin CI actions to commit SHAs and narrow pages workflow permissions"
kind: chore
component: project
origin: review
tags: [security, ci]
---
# Pin CI actions to commit SHAs and narrow pages workflow permissions

Found in the 2026-07-02 security review of the CI workflows. The three GitHub Actions workflows reference third-party code by mutable tags, and the page generator by the default branch HEAD. These references run inside jobs that hold write-scoped tokens, so a moved tag or an upstream compromise would run attacker code with those permissions. The tj-actions/changed-files compromise of March 2025 worked this way, with tags retargeted at a malicious commit. This is latent supply-chain exposure in CI, not a runtime bug in the shipped binary, and the security triage found no incident. Per file, `pages.yml` is the most exposed, `release.yml` has the highest impact with a lower likelihood, and `ci.yml` is low: its token is read-only and no secrets flow through it.

## Scope

The security triage decided the pinning policy. Pin every action in all three workflows uniformly, including the `actions/*` ones, to a full commit SHA with a `# vX.Y.Z` trailing comment, and let Dependabot manage the bumps. Pin the generator checkout to a full commit SHA. Add `--frozen-lockfile`. Split the permissions of `pages.yml` per job. Add `persist-credentials: false` everywhere.

- Pin `actions/*` too. GitHub only requires SHA pins outside your trust boundary, and OpenSSF Scorecard weights third-party actions more heavily. For a solo project, uniformity wins: "every action is a SHA plus a comment" is mechanical and maintained by Dependabot, while a two-tier rule needs a judgment call on every edit.
- Generator checkout: full commit SHA. No tags exist, and HEAD is rejected on auditability grounds. The residual risk is near zero. The cost is that generator improvements no longer flow in automatically. That is a small cost, since the same owner controls both repositories and the page rebuilds only on README or docs pushes. Dependabot does not bump a `with: ref:` value, so that bump is manual whenever the generator changes.
- `--frozen-lockfile` works today, since `bun.lock` is committed upstream. It is secondary to the SHA pin, because the lockfile ships in the same checkout as the code. It turns silent dependency re-resolution into a loud CI failure when the lockfile and `package.json` drift apart.
- Per-job least privilege in `pages.yml` is the most important single change. After it, the Bun and generator job holds only `contents: read`, and only GitHub's own SHA-pinned `deploy-pages` sees `pages: write` and `id-token: write`.
- Dependabot over Renovate. For a solo repository, Dependabot needs no infrastructure, is native to GitHub, preserves the `# vX.Y.Z` comments and groups its PRs. Renovate's one edge here, automating the generator `ref:` bump, does not justify the extra machinery.
- Cross-repository split, flagged rather than pulled in silently. The SHA pins and `--frozen-lockfile` belong in this repository, since ntropy owns the workflows. Tagging releases in `project-page-starter`, so future pins read `ref: <sha> # v0.x.y`, belongs in that repository. Keeping `bun.lock` committed there is already true.

## Verified facts

Verified against the workflow files and the GitHub API (repository permissions, the `github-pages` environment, and the generator repository's tags and lockfile).

### `pages.yml`

- The workflow-level `permissions: contents: read, pages: write, id-token: write` (lines 14-17) applies to both `build` and `deploy`. The `concurrency` group (lines 20-22) is fine.
- A second checkout of `jakobwesthoff/project-page-starter` has no `ref:` (lines 34-37). `bun install` runs without `--frozen-lockfile` (line 44). A leftover template header sits in lines 1-2. Triggers: a push to `main` touching `README.md` or `docs/pages/**`, and `workflow_dispatch`.
- `id-token: write` is needed only by `actions/deploy-pages` in the deploy job. `configure-pages` and `upload-pages-artifact` need neither `pages: write` nor `id-token: write`, and the build job needs only `contents: read`. This is the worst single fact: the job that runs unpinned foreign code can mint OIDC tokens and drive a Pages deploy.
- The generator repository has a committed `bun.lock`, so `--frozen-lockfile` works today without a change there. It has zero tags and zero releases, so the checkout can only be pinned to a SHA.
- `actions/checkout@v6` persists credentials by default. v6 moved them into a file under `$RUNNER_TEMP`, which every later step can still read. So `bun install` and the generate step run with an ambient repository token available.
- `oven-sh/setup-bun@v2` without `bun-version` installs the latest Bun at run time. This is a minor reproducibility issue. Bun does not run dependency lifecycle scripts by default, which blunts install-time code execution, but imported code still runs.
- The `github-pages` environment already has a custom branch deployment policy, so a `workflow_dispatch` from an arbitrary branch cannot complete the deploy.

### `release.yml`

- Workflow-level `contents: write`. Tag triggers: `v[0-9]+.[0-9]+.[0-9]+` and `-rc[0-9]+`. The `create-release` job uses `actions/checkout@v6` and `taiki-e/create-gh-release-action@v1`. The `upload-assets` job, a matrix of two macOS and two linux-musl targets, uses `actions/checkout@v6` and `taiki-e/upload-rust-binary-action@v1`.
- Both jobs genuinely need `contents: write`, so a per-job split is declarative hygiene, not a privilege reduction.
- No Rust toolchain is pinned anywhere: there is no `rust-toolchain.toml` and no `rust-version` in `Cargo.toml`. Release binaries build with whatever stable toolchain the runner has that day.

### `ci.yml`

- No explicit `permissions`. The repository default token is `read`, verified through the API. That is a repository setting, not a guarantee the workflow itself makes.
- Uses `actions/checkout@v6`, `dtolnay/rust-toolchain@stable`, `taiki-e/install-action@just`, `Swatinem/rust-cache@v2` and `just check`.
- Pinning nuance: `taiki-e/install-action@just` and `dtolnay/rust-toolchain@stable` take the ref as the parameter. SHA-pinning them means switching to the explicit input form (`with: tool: just`, `with: toolchain: stable`). A SHA-pinned `dtolnay/rust-toolchain` still installs the current stable channel. The pin covers the action code, not the compiler, which is fine because the official rustup channel supplies the compiler.

## Why the self-owned generator still needs pinning

A self-owned generator reduces the risk less than it looks. It removes the malicious-upstream-maintainer case, and an account compromise of the generator is roughly as bad as one of ntropy itself. It does not reduce two things. First, auditability: a deploy triggered by an innocuous README push runs whatever the generator HEAD is that day, and ntropy's history records nothing about the code that produced the published page. Second, the transitive supply chain: the generator's dependency tree is inherited invisibly. The net effect is a move from "third-party code with write-like privileges" to "an unauditable moving dependency with write-like privileges".

## Prioritization

The work is batchable and cheap, about 30 minutes, so do it in one sitting. This project ships downloadable binaries, so the integrity of the release pipeline and of the landing page are the two assets that matter, and both affected workflows touch them. If only one change is made, make the `pages.yml` permission split. It is a six-line edit that limits the blast radius of the unpinned generator HEAD, the unpinned Bun dependencies and every unpinned action in the build job, without touching any of them. The second change is SHA-pinning the two `taiki-e` actions in `release.yml`.

## Concrete edits

`pages.yml`, permissions split (replacing lines 14-17):

```yaml
permissions: {}

jobs:
  build:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    ...
  deploy:
    needs: build
    runs-on: ubuntu-latest
    permissions:
      pages: write
      id-token: write
    ...
```

`pages.yml`, generator checkout pinned with no persisted credentials:

```yaml
      - uses: actions/checkout@<full-40-char-sha>       # v6.0.3
        with:
          repository: jakobwesthoff/project-page-starter
          ref: <full-40-char-generator-commit-sha>      # bump manually when generator changes
          path: generator
          persist-credentials: false
```

Add `persist-credentials: false` to the project checkout too. Nothing after checkout needs git credentials in any workflow, since the `taiki-e` actions get their token through the explicit `token:` input.

`pages.yml`, frozen lockfile:

```yaml
      - name: Install dependencies
        run: cd generator/generator && bun install --frozen-lockfile
```

SHA pin format, for every `uses:` line in all three files:

```yaml
      - uses: actions/checkout@08eba0b27e820071cde6df949e0beb9ba4906955   # v6.0.3
      - uses: taiki-e/create-gh-release-action@<full-sha>                 # v1.9.1
```

Resolve SHAs, which dereferences annotated tags, with:
`git ls-remote https://github.com/actions/checkout 'refs/tags/v6.0.3^{}'`

`ci.yml`, ref-as-parameter actions become explicit input, plus read permissions:

```yaml
permissions:
  contents: read
# ...
      - uses: dtolnay/rust-toolchain@<full-sha>   # branch HEAD, installs stable
        with:
          toolchain: stable
          components: clippy, rustfmt
      - uses: taiki-e/install-action@<full-sha>   # v2.x.y
        with:
          tool: just
```

`.github/dependabot.yml` (new):

```yaml
version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
    groups:
      actions:
        patterns: ["*"]
```

## Further changes

- Delete the leftover template header (line 2 of `pages.yml`, "Copy this file to your project...").
- `workflow_dispatch` does not widen exposure materially. Only users with write access can dispatch, and the `github-pages` branch policy blocks the deploy job from disallowed branches. After the permission split, a dispatch from a rogue branch runs a build job with only `contents: read`.
- `release.yml` extras, in descending value:
  - Artifact attestation via `actions/attest-build-provenance`, which needs `id-token: write` and `attestations: write` on the upload job. It gives provenance that survives a compromised `contents: write` token, unlike the sha256 checksums, which are generated in the same trust domain they verify. Worth doing eventually, but not part of the minimum fix. It reintroduces `id-token: write` on a build job, so keep it SHA-pinned and scoped to the upload job only.
  - A toolchain pin (`rust-toolchain.toml`, or a pinned `dtolnay/rust-toolchain` step), so release binaries are not built with whatever stable the runner has. This is about reproducibility more than security.
  - `persist-credentials: false` on both checkouts.
- The `github-pages` environment's branch policy is adequate for a solo repository. Required reviewers would be theater when one person approves every deploy.
- In `project-page-starter`, which is out of scope here and should be raised with the user: start tagging releases, so ntropy's future pins can read `ref: <sha> # v0.x.y`. Keep `bun.lock` committed, which is already true.

## Done when

- Every `uses:` in the three workflows is a full commit SHA with a `# vX.Y.Z` comment, and the generator checkout has a SHA `ref:`.
- `pages.yml` permissions are per job: build has only `contents: read`, and deploy has `pages: write` and `id-token: write`.
- `bun install --frozen-lockfile` runs, and `persist-credentials: false` is set on all checkouts.
- `.github/dependabot.yml` exists for `github-actions`, or Dependabot is explicitly declined.
- The leftover `pages.yml` header is removed.
- Deferred items are recorded: release attestation and the toolchain pin (later), and release tagging in `project-page-starter` (in that repository).
