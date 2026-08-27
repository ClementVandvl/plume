# Packaging and release

> Written in English, like the rest of the technical content.

## What can be built where

A Tauri installer is produced by tools that only run on their own platform:
NSIS and WiX on Windows, `bundle_dmg.sh` and `codesign` on macOS, `dpkg` and
`appimagetool` on Linux. The Windows target also needs the MSVC toolchain.

**A Mac cannot produce a Windows package.** Cross-compiling the Rust binary to
`x86_64-pc-windows-gnu` is possible in principle, but Tauri's WebView2
integration and its installers are not supported that way. The supported answer
is one runner per platform, which is what
[`.github/workflows/release.yml`](../.github/workflows/release.yml) does.

| Platform | Produced by the workflow | Needs a git remote |
|---|---|---|
| macOS (Apple Silicon, Intel) | `.dmg`, `.app` | yes |
| Windows | NSIS `.exe` installer | yes |
| Linux | `.AppImage`, `.deb` | yes |

Locally, `npm run tauri build` produces packages for **the current platform
only**.

## Releasing

1. Commit and push the repository to GitHub.
2. Tag a version and push the tag:

   ```bash
   git tag v0.1.0 && git push origin v0.1.0
   ```

3. The workflow builds all four targets and opens a **draft** release with the
   packages attached. Review it, then publish.

### Trying one platform

**Actions → release → Run workflow** offers a checkbox per platform, so a
Windows build can be tried without waiting on the other three.

Leave **Créer une release brouillon** unticked for a trial: the packages are
attached to the run as artifacts (kept 14 days) and no release is opened. A
Windows check has no business creating a release.

Tick it, with a version such as `v0.1.0`, to produce a draft release instead.
Pushing a tag always builds every platform and always releases.

The workflow runs `npm run build` (style tokens, front-end tests, typecheck) and
`cargo test` before packaging, so a release cannot be cut from a red tree.

## macOS: the `internet-enable` failure

`npm run tauri build` may report:

```
failed to bundle project: error running bundle_dmg.sh
```

while the `.dmg` is in fact present and valid. The script's last step calls
`hdiutil internet-enable`, which Apple removed in macOS 10.15; it returns
non-zero and Tauri treats that as failure. Verify the image rather than trusting
the exit code:

```bash
hdiutil verify src-tauri/target/release/bundle/dmg/Plume_*.dmg
```

If the image is missing entirely, delete the leftover `rw.*.dmg` intermediate
and run the build again.

## Signing and notarisation

Configuration is in place and **switched off**: the workflow already passes the
signing environment variables, and they resolve to empty until the secrets
exist. Nothing else changes when they do.

Until then the build is ad-hoc signed, and macOS shows *"Plume est endommagé et
ne peut pas être ouvert"* on first launch — Gatekeeper's message for a
quarantined unsigned app. The workaround is right-click → **Ouvrir**, which is
exactly the obstacle this project exists to avoid, so this is temporary by
design.

To turn signing on, add these repository secrets (Settings → Secrets and
variables → Actions). **Generate and add them yourself** — they are credentials
and should not pass through anyone else's hands:

| Secret | What it is |
|---|---|
| `APPLE_CERTIFICATE` | Developer ID Application certificate, exported as `.p12`, base64-encoded |
| `APPLE_CERTIFICATE_PASSWORD` | The password set when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | The Apple ID of the developer account |
| `APPLE_PASSWORD` | An **app-specific password**, not the account password |
| `APPLE_TEAM_ID` | The 10-character team identifier |

Windows code signing follows the same shape (`WINDOWS_CERTIFICATE`,
`WINDOWS_CERTIFICATE_PASSWORD`) and needs a certificate from a commercial
authority; without it, SmartScreen warns on first run.

## Updates

Plume checks for a new version and installs it on demand, from the settings
panel. **Only the check is automatic**, and only when the teacher leaves
"Rechercher une mise à jour au démarrage" on. Installing always waits for a
click: replacing the application someone is working in is not a decision to take
on their behalf.

The release workflow publishes `latest.json` beside the packages; the app polls
`https://github.com/ClementVandvl/plume/releases/latest/download/latest.json`.
Change the URL in `tauri.conf.json` if the repository is named differently.

### Turning updates on

The updater has **its own signing key**, unrelated to code signing: it is what
the installed app checks before replacing itself. Nothing updates until it
exists.

Generate it yourself — it is a private key and should not pass through anyone
else's hands:

```bash
npm run tauri signer generate -- -w ~/.tauri/plume.key
```

The public key is already in `plugins.updater.pubkey`. What remains is to add
two repository secrets:

| Secret | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | the contents of `~/.tauri/plume.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the password chosen when generating it |

Keep the private key. Losing it means no already-installed copy can ever accept
another update — they would all have to be reinstalled by hand.

If the public key is ever cleared, the settings panel says updates are not
configured rather than reporting "up to date". Claiming nothing is available
when nothing was checked would be a lie.

Only the public half belongs in the repository. Decoding
`plugins.updater.pubkey` should read `minisign public key`; anything mentioning
a secret key means the wrong half was pasted, and the key must be regenerated.

### macOS and unsigned builds

On macOS the updater replaces the application bundle, which Gatekeeper only
accepts smoothly for a signed and notarised app. Until code signing is on,
expect updates to work on Windows and to be unreliable on macOS.

## The LaTeX engine

Plume installs Tectonic itself, from the settings panel, into its own
application data directory — see [`engine.rs`](../src-tauri/src/engine.rs).
Nothing has to be preinstalled.

It is downloaded rather than bundled: the copy stays current, and updating the
engine later will not mean shipping a new version of Plume. The download uses
`curl` and `tar`, both present on macOS, Windows 10+ and Linux, so no HTTP or
TLS crate is vendored. The newest matching release is resolved from the GitHub
API, with a pinned version as a fallback when the API cannot be reached.

The installed binary is run once with `--version` before being accepted: a
truncated download extracts happily and would otherwise fail mid-compilation.

### Measured

| | |
|---|---|
| Download and install | ~2 s |
| First compile | ~60 s — Tectonic fetches the LaTeX packages it needs |
| Later compiles | ~0.7 s |

The first compile therefore needs a network connection, and is slow **once**.

### Engine equivalence

Tectonic is XeTeX-based while the bundled template targets `pdflatex`, so the
two were compared on the same document. The output is equivalent: chapter box,
coloured headings, bracketed environments, TikZ and maths all render the same.

The one difference is French punctuation spacing — Tectonic sets the thin space
before a colon (`Définition :`), pdflatex does not. Tectonic is the more correct
of the two.
