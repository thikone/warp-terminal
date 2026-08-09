# Warp Terminal — terminal-only fork

A fork of [warpdotdev/warp](https://github.com/warpdotdev/warp) stripped down to **just the terminal**.

Auto-update is gone, the AI/agent surfaces are gone, telemetry never runs, and all
user state lives next to the binary so the build is genuinely portable.

Upstream licensing is unchanged: `warpui_core` and `warpui` are MIT
([LICENSE-MIT](LICENSE-MIT)); everything else is AGPL v3 ([LICENSE-AGPL](LICENSE-AGPL)).

---

## What changed

| Area | Effect |
|---|---|
| Auto-update | No version checks, no download/install code, no update UI, no "out of date" banner |
| AI / agents | Permanently disabled at the gate; all AI UI and settings pages removed |
| Onboarding | First launch goes straight to a terminal — no welcome slides, no sign-in/skip screen |
| Telemetry | Never initialises; no RudderStack traffic and no retry loop |
| Settings | 9 sidebar entries removed (plus MCP servers, reachable only from the Agents group); 6 remain |
| Shell bootstrap | Works even when the bootstrap script is unsigned or carries Mark-of-the-Web |
| Storage | Portable — everything beside the binary in `data\` |
| Version | Tagged `…​.terminal_NN` instead of `…​.stable_NN` |

Only the **`warp-oss`** binary is buildable from this tree. The `stable`, `preview`,
`dev` and `local` binaries call `warp_channel_config::load_config!`, which shells out
to an internal `warp-channel-config` generator that is not part of the open-source
repo. `warp-oss` is the one entry point with a fully hardcoded `ChannelConfig`
(`app/src/bin/oss.rs`), and it already ships `telemetry_config: None`,
`crash_reporting_config: None` and `autoupdate_config: None`.

---

## 1. Auto-update removed

**Deleted** from `app/src/autoupdate/`: `channel_versions.rs` and `changelog.rs`
(the only two network callers), `mac.rs`, `linux.rs`, `windows.rs` (all
download/install code), plus `mod_tests.rs`, `linux_tests.rs`, `windows_tests.rs`.

`app/src/autoupdate/mod.rs` is now an inert stub keeping only the type surface the
rest of the app references. `get_update_state()` always returns
`AutoupdateStage::NoUpdateAvailable`, `start_polling()` is a no-op, and
`apply_pending_update()` always returns `false` so shutdown proceeds normally.

UI removed from `app/src/workspace/view.rs`:

- the tab-bar overflow menu that hosted *Current version is…* / *Install update* /
  *Update Warp manually* (this was the window-caption button)
- the avatar-menu update entries and the avatar's red notification dot
- the tab-bar "Update Warp" pill
- `render_autoupdate_banner_element()` → returns `None` unconditionally. This is the
  function that produced the non-dismissible **"Your app is out of date and some
  features may not work as expected"** banner, driven by the server's `soft_cutoff`.
- the `AutoupdateState_UpdateReady` keybinding context flag
- constants `TAB_BAR_PILL_WIDTH`, `PILL_FONT_SIZE`, `UPDATE_READY_TEXT`,
  `VERSION_DEPRECATION_BANNER_TEXT`, `VERSION_DEPRECATION_WITHOUT_PERMISSIONS_BANNER_TEXT`

Also: `app/src/settings_view/main_page.rs` — the version widget's `Relaunch` /
`DownloadUpdate` / `CheckForUpdate` actions are no-ops.
`app/src/changelog_model.rs` — the changelog fetch is gone; it hit the same release
server.

> The TUI updater (`crates/warp_tui/src/autoupdate.rs`) is **untouched** — it belongs
> to a separate binary that this build does not produce.

## 2. AI and agents disabled

`app/src/settings/ai.rs` — `AISettings::is_any_ai_enabled()` returns a constant
`false`, and the backing setting's default flipped to `false` for the few call sites
that read the field directly.

This one gate is consulted by ~200 UI sites and is projected into the keybinding
context system as `flags::IS_ANY_AI_ENABLED`, so every AI command-palette entry and
keybinding predicated on that flag disappears with it.

**The AI code is still compiled, just unreachable.** `app/src/ai` alone is ~540 files;
deleting it would cascade through `workspace/view.rs` (~28k lines), the settings-view
event enum and `lib.rs` startup. That was judged not worth the risk.

## 3. Settings pages removed

Each page's `should_render()` returns `false`, which removes it from both the sidebar
*and* the content area through the single choke point
`SettingsView::filtered_pages()` in `app/src/settings_view/mod.rs` — so deep links
(`warp://settings`) and search cannot reach them either.

| Page | File |
|---|---|
| Account | `main_page.rs` |
| Agents *(umbrella)* | `ai_page.rs` |
| Code *(umbrella)* | `code_page.rs` |
| Cloud platform *(umbrella)* | `environments_page.rs`, `platform_page.rs` |
| MCP servers | `mcp_servers_page.rs` |
| Referrals | `referrals_page.rs` |
| Warp Drive | `warp_drive_page.rs` |
| Teams | `teams_page.rs` |
| Billing and usage | `billing_and_usage_dispatch.rs` |
| Shared blocks | `show_blocks_view.rs` |

In `app/src/settings_view/mod.rs` the `nav_items` list was trimmed to match, the
`Scripting` insert was re-anchored (it positioned itself relative to the now-absent
`SharedBlocks`), and `#[default]` moved from `Account` to `Appearance` — `Account`
was the default landing section and would otherwise have opened a blank pane.

**Remaining:** Appearance, Features, Keyboard shortcuts, Warpify, Privacy, About.

## 4. Avatar drop-down trimmed

`user_menu_items()` in `app/src/workspace/view.rs` now offers only **Settings**,
**Keyboard shortcuts**, **Documentation** and **View Warp logs**.

Removed: *What's new*, *Feedback*, *Join our Slack community*, *Sign up*, *Upgrade*,
*Billing and usage*, *Invite a friend*, *Log out*.

The command-palette mirrors of those entries were removed too, from
`add_overflow_menu_items_as_editable_binding()` in `app/src/workspace/mod.rs` —
otherwise they stayed reachable by typing.

## 5. Onboarding and login bypassed

`app/src/root_view.rs` — `auth_onboarding_state` resolves to
`AuthOnboardingState::Terminal` unconditionally on native builds. This bypasses the
agent onboarding slides, the post-onboarding **sign-in / skip** screen
(`app/src/auth/login_slide.rs`) and the `Auth` screen.

Side effect worth knowing: onboarding was also what wrote
`default_session_mode = "agent"` and a full agent execution-profile block into
`settings.toml`. Skipping it means a fresh profile contains only appearance defaults,
and new tabs open as terminals.

## 6. Telemetry never runs

`app/src/server/telemetry/mod.rs` — `send_batch_messages_to_rudder()` returns early
when `ChannelState::is_telemetry_available()` is false.

Nothing was ever transmitted before either: the OSS channel ships
`telemetry_config: None`, so the write key and root URL are empty and every request
failed in the HTTP builder. This removes the pointless retry loop, which was logging
`builder error` roughly every 30 seconds.

## 7. Shell bootstrap: execution policy

`app/assets/bundled/bootstrap/pwsh_init_shell.ps1` now sets **process-scope `Bypass`**
whenever `MachinePolicy` and `UserPolicy` are not `Restricted`. Previously it only
relaxed the policy when the effective policy was `Restricted`.

Why this matters: Warp's released `pwsh.ps1` is Authenticode-signed; a self-built one
is not. If the source tree came from a downloaded archive it also carries
Mark-of-the-Web. Under `RemoteSigned` that combination makes PowerShell refuse to
dot-source the bootstrap, and the terminal hangs forever on
**"Starting PowerShell Core…"** with the error only visible on the shell's stderr,
never in `warp-oss.log`.

Process scope only — nothing is written to machine or user policy, and Group Policy
still wins if it is set.

## 8. Portable storage

`crates/warp_core/src/paths.rs` adds `portable_root()`, using the convention VS Code
uses: **if a directory named `data` sits next to the executable, all user state goes
there.** Opt-in and reversible — delete the folder and the app falls straight back to
the per-user profile, which is left untouched.

Redirected: `config_local_dir()`, `data_dir()`, `state_dir()`, `cache_dir()`,
`gui_config_local_dir()`, `warp_home_config_dir()`. Everything else
(`themes_dir()`, `tui_config_local_dir()`, `tui_state_dir()`, the log directory)
derives from those.

```
warp-oss.exe
data\
  config\   settings.toml, keybindings.yaml, user_preferences.json, cli\
  state\    warp.sqlite, logs\, telemetry queue, index snapshots
  cache\
  .warp\    workflows, skills, .mcp.json
```

Roaming and local are collapsed into one `state\` — a portable install has no roaming
profile to sync with. Upstream splits these across **two** roots
(`%LOCALAPPDATA%` *and* `%APPDATA%`), which is easy to miss.

## 9. Build profile and version

`Cargo.toml` — `[profile.rlto]` gained `strip = "symbols"`. Note this buys almost
nothing on Windows/MSVC, where debug info goes to a separate `.pdb` rather than into
the `.exe`; it is kept for ELF/Mach-O targets.

The version string is set at build time via `GIT_RELEASE_TAG` (read through
`option_env!` in `crates/warp_core/src/channel/state.rs`) — no code change. The
channel segment is capture group 3 of the parser's regex in
`crates/channel_versions/src/lib.rs`, and `ParsedVersion` discards it, so
`.terminal_01` parses and orders identically to `.stable_01`.

## 10. Theme

`app/src/themes/default_themes.rs` — `dark_theme()` background restored to
`0x000000FF`. Upstream changed it to `0x050505FF`, which looks washed out on OLED.

## 11. Housekeeping

Compiler warnings introduced by the removals were cleaned up: unused imports and
variables removed, five dead constants deleted, and genuinely unreachable code
(the changelog parsing methods, the settings-umbrella machinery) annotated with
`#[allow(dead_code)]` and a comment explaining why. `app/src` builds warning-free.

New files: `script/windows/portable/install.ps1` and `uninstall.ps1` — optional
convenience only (Start Menu shortcut, `Unblock-File`, optional PATH entry). The
portable folder runs without them.

---

## Building (Windows)

Prerequisites: Rust 1.92.0 (pinned by `rust-toolchain.toml`), VS Build Tools with
the MSVC toolchain and a Windows SDK, CMake, protoc, NASM. **LLVM/libclang is not
needed** — `bindgen` only runs when the target OS is macOS
(`crates/warpui/build.rs`).

```powershell
$env:CARGO_FULL_PROFILE = 'rlto'      # app/build.rs stages the Windows DLLs here
$env:CARGO_BIN_NAME     = 'stable'    # picks app\channels\<name>\icon -> the exe icon
$env:WARP_APP_NAME      = 'WarpOss'
$env:GIT_RELEASE_TAG    = 'v0.2026.08.08.00.00.terminal_01'

cargo build -p warp --profile rlto --bin warp-oss --features release_bundle,gui
```

Do **not** pass `--target x86_64-pc-windows-msvc`. `app/build.rs` stages `conpty.dll`,
`dxcompiler.dll`, `dxil.dll` and `x64\OpenConsole.exe` into `target\<profile>\`, which
does not include the target triple — omitting the flag puts the exe in the same place
and the layout builds itself.

`release_bundle` is required, not optional: it enables `rust-embed`'s `debug-embed`,
without which assets are read from absolute build-machine paths and the binary is not
portable. It also sets `windows_subsystem = "windows"` so no console window flashes.

### Staging a portable folder

```
warp-oss.exe            conpty.dll        dxcompiler.dll   dxil.dll
msvcp140.dll            vcruntime140.dll  vcruntime140_1.dll
pwsh.ps1  icon.ico      x64\OpenConsole.exe
data\                   <- presence of this folder enables portable mode
```

The first four come from `target\rlto\` (already staged by `build.rs`); the VC++
runtime DLLs come from `app\assets\windows\x64\`. Fonts, themes, images and shell
bootstrap scripts are embedded in the exe — nothing else to copy.

Run `Unblock-File` over the staged folder if the source tree came from a downloaded
archive, otherwise the exe trips SmartScreen on first run.

---

## Known limitations

- **Not zero-footprint.** `app/src/app_services/windows/mod.rs` calls
  `register_uri_handler()` at startup, writing the `warposs://` handler to
  `HKCU\Software\Classes`. Per-user, but outside the portable folder.
- **The Resource Center panel still has community entries** — its footer offers
  *Join our Slack community* and *Feedback*, and its main page has an
  *Invite a friend to Warp* card that dispatches to the (now removed) referrals page.
- **The binary is ~350 MB.** Roughly 61 MB is embedded assets; the rest is compiled
  code, including the unreachable AI stack. Deleting `app/src/ai` is the only real
  lever.
- **Unsigned.** SmartScreen will warn on first run of a copy that carries
  Mark-of-the-Web.
- **Upstream warnings remain** in `crates/warpui_core` (8) and `crates/warp_terminal`
  (1). They predate this fork and were left alone.
