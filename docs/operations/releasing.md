# Releasing

How a version of Sanctum is released. The design and the reasons are in
[ADR-0009](../architecture/decisions/0009-release-integrity.md).

ISOs are hosted on SourceForge or the project site, and uploaded by hand.
The GitHub release holds the checksum, the source manifest, the package
manifest and the download link
([ADR-0005](../architecture/decisions/0005-iso-hosting-and-size-budget.md)).

## One-time repository setup

Before the first release, in the GitHub repository settings:

1. **Environments:** create an environment named `release`, and add yourself
   as a required reviewer. The publishing job then waits for your approval.
2. **Rules:** add a tag ruleset for `v*` so only maintainers can create,
   update or delete release tags.
3. **Branch protection** on `main`: require pull requests, and the CI and
   Build ISO checks.
4. **Code security:** turn on secret scanning and push protection, so a
   key or token is refused even if it slips past `.gitignore`.

## 1. Prepare

1. [Update the base](updating-the-base.md) to a current snapshot.
2. Make sure `CHANGELOG.md`'s `[Unreleased]` section describes everything in
   the release.
3. Set the version and date the changelog:

   ```bash
   scripts/release/prepare.sh X.Y.Z
   ```

   This updates `VERSION`, `Cargo.toml` and `Cargo.lock`, and turns
   `[Unreleased]` into `[X.Y.Z] - YYYY-MM-DD` with a new empty
   `[Unreleased]` above it. It commits nothing.
4. Open a pull request to `main` with these changes. CI and the boot tests
   run on it.

## 2. Test on hardware

Build the ISO from the pull request (or download the `iso` artifact of its
Build ISO run), and work through the
[release checklist](release-checklist.md). Paste the filled-in checklist into
the pull request, then merge.

## 3. Tag

On the merged commit:

```bash
git checkout main && git pull
git tag -s vX.Y.Z -m "Abyssal Sanctum vX.Y.Z"
git push origin vX.Y.Z
```

The Release workflow then:

1. checks the tag (`scripts/release/check-release.sh vX.Y.Z --on-main`),
2. builds and boot-tests the ISO from the tag, without caches,
3. after your approval of the `release` environment, attests the ISO and the
   manifests and drafts the GitHub release.

## 4. Upload

1. Download the `iso` artifact from the Release workflow run (kept 30 days)
   and unpack it.
2. Verify it:

   ```bash
   sha256sum -c abyssal-sanctum-vX.Y.Z-x86_64.iso.sha256
   gh attestation verify abyssal-sanctum-vX.Y.Z-x86_64.iso --repo AbyssalOath/abyssal-sanctum
   ```

3. Collect the package sources (GPL; [ADR-0009](../architecture/decisions/0009-release-integrity.md)):

   ```bash
   scripts/release/fetch-sources.sh abyssal-sanctum-vX.Y.Z-x86_64.sources.txt sources-vX.Y.Z/
   ```

   For each package it downloads Arch's source tarball if Arch still has
   it. Most versions are not on Arch's source server, so it usually clones
   the packaging repository at the version tag and builds the tarball with
   `makepkg --allsource`. This takes a few hours and several GB. The script
   can be stopped and restarted; finished tarballs are kept. Anything it
   could not collect is listed in `MISSING.txt`. Run it again for those
   (`... sources-vX.Y.Z/ PKGBASE ...`), or fix the cause by hand.
4. Upload to the download host: the ISO, its `.sha256`, and the
   `sources-vX.Y.Z/` directory.
5. Download the ISO again from the host and run `sha256sum -c` once more.

## 5. Publish

1. Edit the draft release on GitHub: replace "DOWNLOAD LINK" with the ISO's
   address on the download host, and add a link to the sources directory.
2. Add a "Tested on" line from the hardware checklist.
3. Publish the release.

## If something goes wrong

- **The Release workflow fails before publishing:** fix the problem on
  `main`, delete the tag (`git push --delete origin vX.Y.Z` and
  `git tag -d vX.Y.Z`), and tag the new commit.
- **A problem is found after publishing:** do not move the tag. Release a
  new patch version (`X.Y.Z+1`).
