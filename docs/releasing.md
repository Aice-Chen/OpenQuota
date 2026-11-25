# Releasing OpenQuota (Windows)

This fork targets **Windows (x64)** and distributes signed auto-update installers through GitHub
Releases using Tauri's Minisign updater verification.

## Required Repository Secret

Configure the following GitHub Actions secret before publishing a release:

| Kind           | Name                                 | Required | Purpose                                                            |
| -------------- | ------------------------------------ | -------- | ------------------------------------------------------------------ |
| Actions secret | `TAURI_SIGNING_PRIVATE_KEY`          | Yes      | Minisign private key matching `plugins.updater.pubkey`             |
| Actions secret | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Optional | Password if the Minisign private key is encrypted (leave blank if) |

Updater signatures are cryptographically verified by the installed OpenQuota app using the embedded
public key in `src-tauri/tauri.conf.json`. Native Authenticode EV signing is not enabled, so Windows
SmartScreen may prompt on first direct download; in-app updates still enforce strict Minisign
signature verification before installing.

## Release Workflow

1. **Bump versions in lockstep** across `package.json`, `src-tauri/tauri.conf.json`,
   `src-tauri/Cargo.toml`, and `src-tauri/Cargo.lock`:

   ```sh
   pnpm bump 0.6.0
   ```

2. **Verify locally and push to `main`**:

   ```sh
   pnpm verify:versions && pnpm verify:contracts && pnpm lint && pnpm check && pnpm test && pnpm build
   git add -A && git commit -m "chore: release v0.6.0" && git push origin main
   ```

3. **Trigger the Windows release workflow** (`.github/workflows/build-windows.yml`) via GitHub UI or
   `gh`:

   ```sh
   gh workflow run build-windows.yml --repo Aice-Chen/OpenQuota -f create_release=v0.6.0
   ```

   The workflow:
   - Runs all contract, frontend, and Rust checks (`fmt`, `clippy`, unit tests) on `windows-latest`.
   - Builds the Windows NSIS installer with `src-tauri/tauri.release.conf.json` to emit the `.sig`
     signature alongside `OpenQuota_<version>_x64-setup.exe`.
   - Runs the packaged Windows NSIS install/start/uninstall smoke test.
   - Generates `latest.json` via `.github/scripts/create-updater-json.mjs`.
   - Creates or updates the GitHub Release and uploads the installer, `.sig`, and `latest.json` with
     automatic retries.

4. **Verify auto-update**: Installed clients automatically check
   `https://github.com/Aice-Chen/OpenQuota/releases/latest/download/latest.json` (or any configured
   update mirror) and verify the Minisign signature before applying the update.
