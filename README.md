# Warp Terminal — terminal-only fork

A fork of [warpdotdev/warp](https://github.com/warpdotdev/warp) stripped down to **just the terminal**.

Auto-update is gone, the AI/agent surfaces are gone, telemetry never runs, and all
user state lives next to the binary so the build is genuinely portable. The one thing
it adds is [saving and streaming output to files](#11-save-and-stream-output-to-files),
in place of the cloud sharing it removed.

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
| File capture | Cloud "Share block/session" replaced by six commands that save or live-stream output to a local file |
| Tool panel | Agent conversations and Warp Drive tabs removed; a Session history tab added in their place |

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

## 11. Save and stream output to files

The one feature this fork *adds*. It replaces `Share block...` and `Share session...`,
which uploaded to Warp's cloud and are therefore dead here, with six commands that
write the same content to a local file.

| Command | Binding name | Writes | Keybinding |
|---|---|---|---|
| Save output to file... | Save Output | The block's output, as rendered | `Ctrl+Shift+S` |
| Stream output to file... | Stream Output | The same, then keeps appending while the command runs | `Ctrl+Alt+Shift+S` |
| Save block to file... | Save Block | Command line and output | unbound |
| Stream block to file... | Stream Block | The same, live (only offered while a command is running) | unbound |
| Save session to file... | Save Session | Every block in the session | unbound |
| Stream session to file... | Stream Session | The same, then follows the session across block boundaries | unbound |

The two keybindings act on the selected block(s) if there is a selection, otherwise on
the last block. `Stream` degrades to `Save` when nothing is running. With more than one
block selected the labels pluralise and the session commands are hidden.

All six are registered as editable bindings, so they appear in the **Terminal** section
of the Resource Center's keyboard shortcuts panel and in **Settings → Keyboard
Shortcuts**, where the four unbound ones can be given a key. No default chords were
invented for them — the modifier space around `Ctrl+Shift+S` is already crowded.

**New:** `app/src/terminal/file_capture.rs` — scopes, filename-pattern expansion and
`FileStream`, which appends when the rendered text still starts with what it last wrote
and rewrites from the recorded offset when it does not (a block's tail can be redrawn in
place). `app/src/terminal/file_capture_tests.rs` covers it with 13 tests.

**Changed:**

- `app/src/terminal/view.rs` — the six actions, the wakeup tick that pumps live streams,
  and `session_capture_context_menu_items`, one builder shared by all four menus that
  used to offer session sharing (block right-click, the block "..." button, the
  input-area right-click and the empty-session menu).
- `app/src/terminal/model/block.rs` — `command_and_output_with_secrets_unobfuscated()`
  and `command_line_with_pwd()`, which prefixes the command with the directory it ran
  in, prompt-style, using each block's own recorded pwd and shell type.
- `app/src/tab.rs` and `app/src/workspace/view.rs` — the same two session commands on the
  tab right-click menu, resolving the tab by index so saving another tab's session does
  not steal focus.
- `app/src/terminal/settings.rs` — three `String` settings, `SyncToCloud::Never`.
- `app/src/settings_view/features_page.rs` — those three settings under
  **Settings → Features → Session**, one text field each.
- `app/src/terminal/view/init.rs` — all six registered as editable bindings, named
  `terminal:{save,stream}_{output,block,session}_to_file`.
- `app/src/resource_center/utils.rs` — the six names added to `TERMINAL_KEYBINDINGS`
  so they land in the panel's Terminal section instead of Fundamentals.
- `app/src/resource_center/keybindings_page.rs` — `build_bindings` dropped every
  binding without a trigger, which would have hidden the four unbound captures. It now
  keeps those six by name.

### Filename patterns

| toml key | Default |
|---|---|
| `terminal.file_capture.output_pattern` | `{command}_{timestamp}.log` |
| `terminal.file_capture.block_pattern` | `{command}_{timestamp}.log` |
| `terminal.file_capture.session_pattern` | `{session-or-command}_{timestamp}.log` |

Tokens: `{command}` (first block's command), `{timestamp}` (now),
`{finished-timestamp}` (last block's finish time), `{session-or-command}` (the tab name
if you renamed it, otherwise the first command). Values are sanitised for the
filesystem, and the expansion only seeds the save dialog — the final name is yours.
Clearing a field in the settings page restores its default.

> **Captured files contain secrets in plaintext.** The terminal masks them on screen;
> these files do not. That is deliberate — the point is a faithful transcript — but
> treat the output as sensitive.

## 12. Tool panel: Session history

The tool panel (left panel) had four tabs. The two cloud/AI ones are gone and a
new one takes their place:

| Tab | Shortcut | Status |
|---|---|---|
| Project explorer | `Alt+1` | unchanged |
| **Session history** | `Alt+2` | **new** |
| Global search | `Alt+3` | unchanged |
| ~~Agent conversations~~ | ~~`Alt+2`~~ | removed |
| ~~Warp Drive~~ | ~~`Alt+4`~~ | removed |

Session history lists the commands run in the active session. Each row is three
lines — the command, its working directory, then how long it took, its exit code
if it failed, and how much output it produced:

```
cargo build --release
~/projects/app
1m 04s · 612 lines · 38.6 KB

cargo test
~/projects/app
22.1s · exit 101 · 88 lines · 4.2 KB
```

Failed commands get the same treatment the block list gives a failed block: a
red left stripe over a 10% wash. Running commands appear immediately with a
ticking duration and growing size. Rows are in terminal order, oldest first.

Exit codes are shown only for commands that failed, in decimal. On Windows a
code of 256 or more also gets its hexadecimal form —
`exit -1073741819 (0xC0000005)` — because anything past the 0-255 range a shell
can report is a Windows status code, which documentation and search results
give in hex. The decimal stays signed while the hex is the unsigned
reinterpretation, since those status codes overflow `i32`. The cutoff is
`EXIT_CODE_HEX_THRESHOLD` in `app/src/terminal/session_history.rs`.

### Selection

Selection is shared with the block list: select in either and the other
highlights and scrolls to match. Rows use the block list's own modifier rules —
**Shift-click** extends a range, **Ctrl-click** toggles one in or out, a plain
click selects one. **Double-click** hands the keyboard to the terminal.

Copy and capture shortcuts work while the panel is focused: `Ctrl+Shift+C`,
`Ctrl+Alt+Shift+C`, `Ctrl+Shift+S` and `Ctrl+Alt+Shift+S`, acting on the shared
selection. They are registered under the terminal's own binding names, so
Settings → Keyboard Shortcuts shows one row per command, not two.

### Filtering

The query field holds the regex and case-sensitive toggles and a clear button,
laid out like the find bar and the block filter. Below it are five toggles in
two groups:

- **scope** — command, working directory, output. Independent; command and
  directory start on, output starts off because it is the only one that has to
  read every block's text. With it on, rows show a match count.
- **on-select** — what happens when you select a row:

| filter-on-select | find-on-select | behaviour |
|---|---|---|
| off | off | scroll to the block only |
| on | off | open the block filter toolbar, pre-filled |
| off | on | open the find bar, scoped to that block |
| on | on | apply the filter silently, and open the find bar |

These fire only for a plain click in the panel. A modified click is building a
selection, and a selection made in the terminal must not silently filter the
output of a block you merely clicked on there.

### Where the code lives

**New:** `app/src/terminal/session_history.rs` (the row snapshot type, output
size measurement, and the duration/byte formatting, with unit tests in
`session_history_tests.rs`) and `app/src/workspace/view/session_history.rs`
(the panel itself).

**Changed:**

- `app/src/workspace/view.rs` — `compute_left_panel_views` no longer offers the
  two removed tabs; restoring a session onto one falls back to Project explorer.
- `app/src/workspace/view/left_panel.rs` — the `SessionHistory` tab, its
  toolbelt button (`Icon::History`) and its render/focus wiring.
- `app/src/terminal/view.rs` — the panel-facing API: `session_history_entries`,
  `block_output_size`, `reveal_block_from_session_history`, the filter/find
  entry points, and `select_block_from_session_history`, which shares the block
  list's modifier rules via `apply_modified_block_selection` so the two cannot
  drift.
- `app/src/terminal/block_list_element.rs` — a `SessionHistoryRow` menu source
  carrying a row-relative position, so the block menu can open over the panel.
- `app/src/workspace/mod.rs` — eight bindings in the panel's context, repeating
  the terminal bindings' names and descriptions so the keybindings page dedupes
  them into one row each.
- `app/src/settings/code.rs` — `code.editor.show_session_history`.

The `ToolPanelView::WarpDrive` and `::ConversationListView` variants still
exist, and their panel views are still compiled. They are unreachable: keeping
them lets tab state persisted before the removal still deserialize instead of
being discarded.

## 13. Housekeeping

Compiler warnings introduced by the removals were cleaned up: unused imports and
variables removed, five dead constants deleted, and genuinely unreachable code
(the changelog parsing methods, the settings-umbrella machinery) annotated with
`#[allow(dead_code)]` and a comment explaining why. `app/src` builds warning-free.

New files: `script/windows/portable/install.ps1` and `uninstall.ps1` — optional
convenience only (Start Menu shortcut, `Unblock-File`, optional PATH entry). The
portable folder runs without them.

---

## Building (Windows)

Two scripts wrap everything below. Prefer them — the flags are easy to get wrong,
and getting them wrong produces a binary that looks fine on the build machine and
is broken everywhere else.

```powershell
.\script\windows\build-portable.ps1            # derives the next tag and builds
.\script\windows\package-release.ps1           # zips it, hashes it, writes notes
.\script\windows\package-release.ps1 -Build    # both in one step
```

`build-portable.ps1` refuses to finish unless the linked binary reports the tag it
was asked for and links as PE subsystem 2, which is the only direct evidence that
`release_bundle` applied. `package-release.ps1` takes the version from the exe
rather than a parameter, so an archive cannot claim a version the binary does not
have; it audits the staged folder for user data before archiving, and asks
`claude` to write the release notes from the commits since the previous release
tag. Neither script creates a tag or pushes anything.

The rest of this section is what those scripts do, and why.

Prerequisites: Rust 1.98.1 (pinned by `rust-toolchain.toml`), VS Build Tools with
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

## Building (Linux and macOS)

```bash
script/portable/build-portable.sh            # derives the next tag and builds
script/portable/package-release.sh           # stages and archives it
script/portable/package-release.sh --build   # both in one step
```

Same contract as the Windows pair: the build script owns the flags and verifies them
against the linked binary, and the package script reads the tag out of the binary
rather than taking it as an argument. Artifacts are
`warp-terminal-<tag>-linux-x64.tar.gz` and `warp-terminal-<tag>-macos-arm64.tar.gz`,
each holding the binary, a `data/` marker, and on Linux an optional `install.sh`
that adds a desktop entry and icon under `$HOME`.

Three differences from the Windows build, all deliberate:

- **No `gui` feature.** `gui = ["voice_input"]` does not gate the GUI front-end, so a
  terminal-only build has no use for it, and leaving it out keeps ALSA off the
  runtime dependency list. `release_bundle` is still required, for `debug-embed`.
- **`CARGO_PROFILE_RLTO_DEBUG=0`.** ThinLTO with debuginfo needs more than 16 GB and
  the final `rustc` gets OOM-killed — on a 16 GB VM and on GitHub's runners alike.
  Line tables are the dominant consumer. The cost is that panics on these platforms
  carry addresses rather than `file:line`; Windows keeps `debug = 1`.
- **macOS binaries are ad-hoc codesigned** (`codesign --sign -`). An unsigned arm64
  Mach-O is killed by the kernel, so this is required rather than cosmetic. Only the
  binary is signed and no bundle resources are sealed, so the app writing into a
  sibling `data/` at runtime cannot invalidate the signature. No `.app` is produced:
  `app/src/bin/oss.rs` embeds its own `Info.plist` through `embed_plist`, so the bare
  binary already carries `CFBundleIdentifier` and the rest.

### Linux runtime dependencies

The binary `dlopen`s its windowing and graphics libraries rather than linking them, so
`ldd` lists only seven entries and a missing one shows up as a failure to open a
window rather than as a load error. On a minimal Ubuntu install:

```bash
sudo apt install libx11-6 libx11-xcb1 libxcb1 libxi6 libxcursor1                  libxkbcommon0 libxkbcommon-x11-0                  libwayland-client0 libwayland-egl1 libegl1                  libvulkan1 mesa-vulkan-drivers fontconfig
```

A Vulkan driver is needed as well as the loader — a present `libvulkan1` with nothing
behind it fails the same way. `script/portable/install.sh` checks for all of these and
prints the apt line for whatever is missing, so it is worth running once even if you
do not want the desktop entry.

Linux build dependencies come from `script/linux/install_build_deps`. If the checkout
sits on a network or VM share, set `CARGO_TARGET_DIR` to a local path first — building
into a share is slow, and if that share is also a Windows checkout it mixes two
platforms' artifacts in one profile directory.

`.github/workflows/build-portable.yml` runs both on `ubuntu-24.04` and `macos-14`, on
a pushed `v0.*.terminal_*` tag or on manual dispatch. Windows is not in CI: the local
PowerShell script already does it in about twelve minutes with a warm cache.

Building on Ubuntu 24.04 rather than 26.04 is deliberate — the older glibc means the
artifact also runs on newer distributions, which the reverse would not.

---

## Known limitations

- **Session history is per-session.** It reads the live block list, so it does not
  survive a restart, it swaps when you change tab, and blocks trimmed by
  `terminal.maximum_grid_size` drop out of it. The sqlite `commands` table would
  persist across restarts but stores no output size, which is one of the columns
  the tab shows.
- **Output-scope filtering is the slow one.** Matching against command and directory
  is free; matching against output has to read every block's text. It is off by
  default for that reason.
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
