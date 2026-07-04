# SEC-6: CI supply-chain — privileged workflows run unpinned third-party code

> **STATUS: TRIAGED (code review + Fable security triage, 2026-07-02).**
> Fable verified the facts against the workflow files AND the live GitHub API
> (repo permissions, the `github-pages` environment, the generator repo's tags &
> lockfile) and produced a decided pinning policy. Queue item SEC-6. Low-to-
> Medium hygiene, no incident; cheap enough (~30 min) to do in one sitting.

## Summary

The three GitHub Actions workflows reference third-party code by **mutable tags**
(or a default-branch HEAD for the page generator), inside jobs holding
write-scoped tokens. A moved tag or an upstream compromise would execute attacker
code with the job's permissions (cf. the tj-actions/changed-files compromise,
March 2025: mutable tags retargeted at a malicious commit). Not a runtime bug in
the shipped binary; latent CI supply-chain exposure.

## Severity: Low-to-Medium hygiene (per file)

- **`pages.yml`: Medium-Low (highest of the three).** The build job executes
  another repo's default-branch HEAD, that repo's npm dependency tree, and four
  mutable-tag actions, **while holding `id-token: write` + `pages: write` + a
  persisted repo credential**. Worst case: a malicious landing page served from
  the project's official domain — a real distribution vector, since that page
  tells users how to install the CLI — plus OIDC tokens asserting this repo's
  identity.
- **`release.yml`: Medium-Low (lower likelihood, highest impact).** A moved
  `taiki-e/*@v1` tag executes with `contents: write` at release time and can
  tamper with tags, release assets, and the sha256 checksums (produced in the
  same trust domain). Tampered release binaries are the worst outcome for a
  GitHub-Releases-distributed CLI. `taiki-e` is among the most reputable Rust
  action maintainers, which lowers likelihood, not impact.
- **`ci.yml`: Low / informational.** Read-only token, no secrets flow;
  `Swatinem/rust-cache` is only here and `release.yml` uses no cache, so a
  poisoned CI cache cannot reach release artifacts.

**"Self-owned generator" reduces the risk less than it looks.** It removes the
malicious-upstream-maintainer case (and account compromise of the generator ≈
compromise of ntropy anyway), but does **not** reduce (1) **auditability**: a
deploy triggered by an innocuous README push runs whatever the generator HEAD is
that day, with no record in ntropy's history of what code produced the published
page; (2) **transitive supply chain**: the generator's dependency tree is
inherited invisibly. Net: it downgrades from "third-party code with write-ish
privileges" to "unauditable moving dependency with write-ish privileges".

## Verified facts (workflow files + GitHub API)

### `pages.yml`
- Workflow-level `permissions: contents: read, pages: write, id-token: write`
  (lines 14-17) applies to **both** `build` and `deploy`. A `concurrency` group
  (20-22) is fine.
- Second checkout of `jakobwesthoff/project-page-starter` with **no `ref:`**
  (34-37); `bun install` without `--frozen-lockfile` (44); leftover template
  header (lines 1-2). Triggers: push to `main` on `README.md`/`docs/pages/**`,
  and `workflow_dispatch`.
- **`id-token: write` is needed ONLY by `actions/deploy-pages` (deploy job).**
  `configure-pages` / `upload-pages-artifact` need neither `pages: write` nor
  `id-token: write`; the build job needs only `contents: read`. **This is the
  single worst fact:** the job running unpinned foreign code can mint OIDC tokens
  and drive a Pages deploy.
- The generator repo **has a committed `bun.lock`** → `--frozen-lockfile` works
  today with no change in that repo. The generator repo has **zero tags and zero
  releases** → the checkout can only be pinned to a **SHA**, not a tag.
- `actions/checkout@v6` **persists credentials by default** (v6 moved them to a
  `$RUNNER_TEMP` file, still readable by every later step), so `bun install` and
  the generate step run with an ambient repo token available.
- `oven-sh/setup-bun@v2` with no `bun-version` installs latest Bun at run time
  (reproducibility, minor). Bun does not run dependency lifecycle scripts by
  default (blunts install-time RCE; imported code still executes).
- The `github-pages` environment already has a custom branch deployment policy,
  so `workflow_dispatch` from an arbitrary branch cannot complete the deploy.

### `release.yml`
- Workflow-level `contents: write`; tag triggers (`v[0-9]+.[0-9]+.[0-9]+`,
  `-rc[0-9]+`). `create-release`: `actions/checkout@v6`,
  `taiki-e/create-gh-release-action@v1`. `upload-assets` (matrix 2 macOS + 2
  linux-musl): `actions/checkout@v6`, `taiki-e/upload-rust-binary-action@v1`.
- Both jobs genuinely need `contents: write`, so a per-job split here is
  declarative hygiene, not a privilege reduction.
- **No Rust toolchain pin anywhere** (no `rust-toolchain.toml`, no
  `rust-version` in `Cargo.toml`): release binaries build with whatever stable
  is on the runner that day.

### `ci.yml`
- No explicit `permissions`; the repo default token is `read` (verified via
  API) — but that is a repo setting, not a workflow guarantee.
- `actions/checkout@v6`, `dtolnay/rust-toolchain@stable`,
  `taiki-e/install-action@just`, `Swatinem/rust-cache@v2`, `just check`.
- **Pinning nuance:** `taiki-e/install-action@just` and
  `dtolnay/rust-toolchain@stable` use the ref *as the parameter*. SHA-pinning
  them requires switching to the explicit input form (`with: tool: just` /
  `with: toolchain: stable`). A SHA-pinned `dtolnay/rust-toolchain` still installs
  the current stable channel — the pin covers the action code, not the compiler,
  which is fine (official rustup channel).

## Decided policy

**SHA-pin every action in all three workflows uniformly (including `actions/*`),
with a `# vX.Y.Z` trailing comment; let Dependabot manage bumps. Pin the
generator checkout to a full commit SHA. Add `--frozen-lockfile`. Split
`pages.yml` permissions per job. Add `persist-credentials: false` everywhere.**

- **Pin `actions/*` too:** GitHub's guidance only *requires* SHA pins outside
  your trust boundary, and OpenSSF Scorecard weights third-party more heavily,
  but for a solo project **uniformity wins**: "everything is a SHA + comment" is
  mechanical and Dependabot-maintained, versus a standing two-tier judgment call
  on every edit.
- **Generator checkout → full commit SHA** (no tags exist; HEAD rejected on
  auditability grounds). Residual risk ~zero; cost is that generator
  improvements no longer flow in automatically (trivial — you control both repos;
  the page rebuilds only on README/docs pushes). **Dependabot will NOT bump a
  `with: ref:` value**, so this is a manual bump when the generator changes.
- **`--frozen-lockfile`: yes, works today** (`bun.lock` committed upstream).
  Secondary to the SHA pin (the lockfile ships in the same checkout as the code);
  it turns silent dependency re-resolution on lockfile/`package.json` drift into
  a loud CI failure.
- **Per-job least privilege in `pages.yml`: the most important single change.**
  After it, the Bun/generator job holds only `contents: read`; only GitHub's own
  (SHA-pinned) `deploy-pages` sees `pages: write`/`id-token: write`.
- **Dependabot over Renovate** (solo repo: zero infra, native, preserves the
  `# vX.Y.Z` comment, grouped PRs). Renovate's only edge here (automating the
  generator `ref:` bump) doesn't justify the machinery.

**Cross-repo split (flag, don't silently pull in):** the SHA pin and
`--frozen-lockfile` belong **here** (ntropy owns the workflow). Tagging releases
so future pins read `ref: <sha> # v0.x.y`, and keeping `bun.lock` committed
(already true), belong in `project-page-starter`.

## Prioritization

Batchable but cheap (~30 min); do it in one sitting — this project's product *is*
downloadable binaries, so release-pipeline and landing-page integrity are the two
assets that matter, and both affected workflows touch them. **If only one change
is made: the `pages.yml` per-job permission split** — a ~6-line edit that
neutralizes the blast radius of the unpinned generator HEAD, the unpinned bun
deps, and every unpinned build-job action at once, without touching any of them.
Second: SHA-pin the two `taiki-e` actions in `release.yml`.

## Concrete edits

**`pages.yml` — permissions split (replace lines 14-17):**
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

**`pages.yml` — generator checkout, pinned + no persisted creds:**
```yaml
      - uses: actions/checkout@<full-40-char-sha>       # v6.0.3
        with:
          repository: jakobwesthoff/project-page-starter
          ref: <full-40-char-generator-commit-sha>      # bump manually when generator changes
          path: generator
          persist-credentials: false
```
(Add `persist-credentials: false` to the project checkout too; nothing after
checkout needs git credentials in any workflow — the `taiki-e` actions get their
token via the explicit `token:` input.)

**`pages.yml` — frozen lockfile:**
```yaml
      - name: Install dependencies
        run: cd generator/generator && bun install --frozen-lockfile
```

**SHA-pin format (every `uses:` line, all three files):**
```yaml
      - uses: actions/checkout@08eba0b27e820071cde6df949e0beb9ba4906955   # v6.0.3
      - uses: taiki-e/create-gh-release-action@<full-sha>                 # v1.9.1
```
Resolve SHAs (dereferences annotated tags) with:
`git ls-remote https://github.com/actions/checkout 'refs/tags/v6.0.3^{}'`

**`ci.yml` — ref-as-parameter actions become explicit-input + add read perms:**
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

**`.github/dependabot.yml` (new):**
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

## Anything else

- **Delete the leftover template header** (line 2 of `pages.yml`, "Copy this
  file to your project...").
- **`workflow_dispatch`** does not materially widen exposure: only write-access
  users can dispatch, and the `github-pages` environment branch policy blocks the
  deploy job from non-allowed branches. After the permission split, a rogue-branch
  dispatch runs a build job with only `contents: read`.
- **`release.yml` extras (descending value):**
  - **Artifact attestation** via `actions/attest-build-provenance` (needs
    `id-token: write` + `attestations: write` on the upload job) gives provenance
    that survives a compromised `contents: write` token — unlike the sha256
    checksums, generated in the same trust domain they verify. Worth doing
    eventually; not part of the minimum fix.
    NOTE: this reintroduces `id-token: write` on a build job, so keep it
    SHA-pinned and scoped to the upload job only.
  - **Toolchain pin** (`rust-toolchain.toml` or a pinned `dtolnay/rust-toolchain`
    step) so release binaries aren't built with "whatever stable the runner has".
    Reproducibility more than security.
  - `persist-credentials: false` on both checkouts.
- **`github-pages` environment** already has a branch policy; adequate for a solo
  repo (required reviewers would be theater when the same person approves).
- **In `project-page-starter` (out of scope here — flag to user):** start tagging
  releases so future ntropy pins are `ref: <sha> # v0.x.y`; keep `bun.lock`
  committed (already true).

## Acceptance

- Every `uses:` in the three workflows is a full commit SHA + `# vX.Y.Z` comment;
  the generator checkout has a SHA `ref:`.
- `pages.yml` permissions are per-job: build has only `contents: read`, deploy has
  `pages: write` + `id-token: write`.
- `bun install --frozen-lockfile`; `persist-credentials: false` on all checkouts.
- `.github/dependabot.yml` present for `github-actions` (or Dependabot explicitly
  declined).
- Leftover `pages.yml` header removed.
- Deferred/cross-repo items recorded: release attestation + toolchain pin
  (later), and `project-page-starter` release tagging (other repo).
