# Releasing

cancellai has no build step and no PyPI package — the release artifact is a
git tag, and the "package manager" is the Homebrew formula in this same
repo. This is the exact sequence used for every release; follow it in order.

1. **Land the change on `main` first**, with `CHANGELOG.md`'s `## [Unreleased]`
   section already updated (see [AGENTS.md](../AGENTS.md#changelog)). Don't
   bundle the version bump with unrelated code changes.

2. **Bump the version** in two places (they must always match):
   - `VERSION` constant in `cancellai.py`
   - `version` field in `pyproject.toml`

3. **Move the changelog entry**: rename `## [Unreleased]` to
   `## [X.Y.Z] - YYYY-MM-DD` and add a fresh empty `## [Unreleased]` above it.

4. **Commit and push** the version bump + changelog to `main`.

5. **Tag and push the tag**:

   ```sh
   git tag -a vX.Y.Z -m "cancellAI vX.Y.Z"
   git push origin vX.Y.Z
   ```

6. **Compute the release tarball's sha256** (GitHub generates the tarball
   automatically from the tag, no upload step needed):

   ```sh
   curl -sL -o /tmp/cancellai.tar.gz \
     https://github.com/matteo-dritara/homebrew-cancellai/archive/refs/tags/vX.Y.Z.tar.gz
   shasum -a 256 /tmp/cancellai.tar.gz
   ```

7. **Update `Formula/cancellai.rb`**: bump the `url` tag and replace `sha256`
   with the value from step 6. Commit and push this to `main` — it's a
   separate commit from step 4, because the sha256 of a given tag's tarball
   cannot be known until the tag exists.

8. **Verify the formula end-to-end before calling it done.** Do not skip
   this — a broken formula fails silently for every future `brew install`
   until someone reports it:

   ```sh
   brew tap matteo-dritara/cancellai   # or: brew tap-new-update if already tapped
   brew audit --strict matteo-dritara/cancellai/cancellai
   brew style matteo-dritara/cancellai/cancellai
   brew install matteo-dritara/cancellai/cancellai
   cancellai --version                 # confirm it matches X.Y.Z
   brew test matteo-dritara/cancellai/cancellai
   brew uninstall cancellai && brew untap matteo-dritara/cancellai   # clean up the local test tap
   ```

## Version scheme

Semantic versioning. Given the tool's actual failure mode is "deletes the
wrong thing," treat any change to the safety-critical core described in
[ARCHITECTURE.md](ARCHITECTURE.md#the-safety-critical-core) — the protected
name lists, `safe_remove`, `validate_config_root`, the keep-latest/age-cutoff
selection logic — as at least a MINOR bump with an explicit changelog entry,
even if the CLI surface didn't change.

## What doesn't need a tag

Documentation, CI configuration, and dev-tooling-only changes (ruff/mypy
config, this file) don't need a release on their own. Fold them into
whichever `main` state the next real tag is cut from, or cut a lightweight
patch release if enough of them accumulate that `brew install` should pick
them up sooner (Homebrew installs from a tagged release, not from `main`).
