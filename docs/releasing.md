# Releasing

Releases are built by GitHub Actions (`.github/workflows/release.yml`) for
macOS (Apple Silicon and Intel), Windows and Linux.

## Cut a release

1. Make sure `main` is green (the **CI** workflow runs tests and type checks on
   every push).
2. On GitHub, open **Releases → Draft a new release**, create a tag such as
   `v0.2.0`, write the notes, and click **Publish release**. From the command
   line: `gh release create v0.2.0 --generate-notes`.
3. The **Release** workflow starts and, after roughly 15–20 minutes, attaches the
   installers to the release:

| Platform | Files |
|---|---|
| macOS | `ContactSheet_<version>_aarch64.dmg` (Apple Silicon), `ContactSheet_<version>_x64.dmg` (Intel) |
| Windows | `ContactSheet_<version>_x64-setup.exe`, `ContactSheet_<version>_x64_en-US.msi` |
| Linux | `.AppImage`, `.deb`, `.rpm` |

The version comes from the tag (`v0.2.0` → 0.2.0); there is no need to edit
version numbers in the repo. Tags must look like `v1.2.3`, or `v1.2.3-4` for a
pre-release, because Windows installers only accept numeric versions.

If a platform fails, fix the problem and re-run the failed job from the
workflow page; assets already uploaded are kept.

## Test build without a release

**Actions → Release → Run workflow** builds every platform without touching any
release. The installers are kept as downloadable artifacts on the workflow run
page.

## Signing

### macOS

Without credentials, macOS builds are **ad-hoc signed**: they run, but macOS
warns on first open because the developer can't be verified (see
[installing an unsigned build](#installing-an-unsigned-build)).

To sign and notarize, you need an Apple Developer account ($99/year). Add these
repository secrets (**Settings → Secrets and variables → Actions**, or
`gh secret set NAME`):

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | Your **Developer ID Application** certificate exported from Keychain Access as `.p12`, base64-encoded (`base64 -i cert.p12 \| pbcopy`) |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | The certificate name, e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | Your Apple ID email |
| `APPLE_PASSWORD` | An app-specific password from appleid.apple.com |
| `APPLE_TEAM_ID` | Your 10-character team ID |

When `APPLE_CERTIFICATE` is set, the workflow signs and notarizes macOS builds
automatically; nothing else needs to change.

### Windows

Windows builds are not code-signed yet, so SmartScreen shows a warning on first
run. Signing needs a code-signing certificate or a cloud signing service, and
can be added to the workflow later.

## Installing an unsigned build

- **macOS**: open the DMG and drag ContactSheet to Applications. The first time,
  macOS refuses to open it: go to **System Settings → Privacy & Security** and
  click **Open Anyway**. If macOS says the app "is damaged", run
  `xattr -dr com.apple.quarantine /Applications/ContactSheet.app` once.
- **Windows**: in the SmartScreen dialog, click **More info → Run anyway**.
- **Linux**: make the AppImage executable (`chmod +x`) and run it, or install
  the `.deb` / `.rpm`.

macOS is the development platform. Windows and Linux builds are produced but
less tested.

## Building locally

```sh
npm run tauri build
```

produces the bundles for your own platform under `target/release/bundle/`.
