# Cutting a release

The pipeline is tag-driven: pushing a `v*` tag runs the Release
workflow (`.github/workflows/release.yml`), which gates on CI,
regenerates both changelogs, syncs the version files, creates the
GitHub Release, and publishes the multi-arch image. The authoritative
statements of that flow are the header comment on `release.yml` and
the `changelog` recipe in the `justfile` (D042, D062); this page is
the runbook form of the same facts.

## Steps

1. Cut the release commits on `main` — `cog.toml` whitelists bumps to
   `main` only, and `cog bump --auto` underreports ("no bump needed").
   Make them by hand: bump the four version files (workspace
   `Cargo.toml`, `Cargo.lock` via `cargo update -w`,
   `custom_components/leaflog/manifest.json`, `leaflog/config.yaml`)
   in a `chore(version): X.Y.Z` commit.
2. Tag that version commit: `git tag -a vX.Y.Z -m "Leaflog X.Y.Z"`
   (annotated and signed; `tag.gpgsign` is on).
3. Push `main` and the tag. That push is the release — everything
   after it is the pipeline working.

## What the pipeline owns

- **Both changelogs.** `CHANGELOG.md` and `leaflog/CHANGELOG.md` are
  generated output — never edit them by hand. The workflow regenerates
  them wholesale via `just changelog`, commits them itself as
  `chore(release): bump version to X.Y.Z`, and pushes `main`. Do not
  run `just changelog` and commit the result before pushing the tag:
  the workflow's own commit then fails to fast-forward and the
  release run fails (this exact failure shipped v0.5.1).
- **The GitHub Release.** Created by the workflow with the cocogitto
  changelog as its body.

## Recovery

If the tag-triggered run fails after `main` has moved on (any reason),
the workflow accepts `workflow_dispatch`: `gh workflow run
release.yml --ref main`. Dispatch mode reads the version from
`leaflog/config.yaml`, finds no drift when the working tree already
matches, and proceeds straight to the GitHub Release. This hatch
recovered v0.5.1.

## Content that is not changelog material

Both changelogs are generated, so prose that belongs to neither file —
release-history context such as the v0.5.1 note that the false "No
data" banner shipped in v0.4.0 but stayed invisible until v0.4.1 —
goes in the GitHub Release notes instead
(`gh release edit vX.Y.Z --notes-file ...`).

## Serving the notes to Home Assistant

Home Assistant serves `leaflog/CHANGELOG.md` from the app directory
and slices it on headings that must be exactly `## <version>` matching
`leaflog/config.yaml`. The `changelog` recipe performs that rewrite;
preserve it when touching either file.
