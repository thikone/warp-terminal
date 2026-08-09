use std::ffi::OsString;

use warp_core::channel::ChannelState;
use warp_errors::report_error;
use windows_registry::{CURRENT_USER, HSTRING};

/// Claims the Explorer folder context-menu entries ("Open Warp in new tab" /
/// "in new window") for *this* executable.
///
/// Written on every startup, exactly like [`register_uri_handler`], and
/// deliberately silent: whichever copy of the app launched most recently owns
/// the menu. That self-healing matters because a stale path here does not
/// merely fail -- it starts a second, different binary which shares this one's
/// single-instance identity, fails to hand off its arguments over IPC, and can
/// take the running instance down with it.
///
/// The entries are intentionally *not* removed on exit. Their whole purpose is
/// to open a folder when the app is not already running, and exit is not
/// guaranteed anyway -- a crash or force-kill would leave them behind
/// regardless. `script/windows/portable/uninstall.ps1` removes them on request.
///
/// The command passes a URI rather than a bare path, so the single-instance
/// manager forwards it to an existing window instead of starting a second
/// process.
pub(super) fn register_context_menu_entries() {
    let Ok(classes_key) = CURRENT_USER.open("Software\\Classes") else {
        report_error!("Failed to get current_user\\software\\classes for context menu");
        return;
    };

    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            report_error!(
                anyhow::Error::new(err)
                    .context("Could not get current executable for context-menu registration")
            );
            return;
        }
    };

    // Prefer the icon staged next to the executable; fall back to the icon
    // embedded in the executable itself.
    let icon = exe
        .parent()
        .map(|dir| dir.join("icon.ico"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| exe.clone());

    // `app_id()` returns by value, so it has to outlive the borrowed name.
    let app_id = ChannelState::app_id();
    let app_name = app_id.application_name();
    let uri_scheme = ChannelState::url_scheme();

    for (action, verb, label) in [
        ("new_tab", "Tab", "Open Warp in new tab"),
        ("new_window", "Window", "Open Warp in new window"),
    ] {
        // `%1` is the folder that was right-clicked. `%V` is the folder being
        // viewed when the background of an Explorer window is right-clicked.
        for (parent, path_arg) in [
            ("Directory\\shell", "%1"),
            ("Directory\\Background\\shell", "%V"),
        ] {
            let key_path = format!("{parent}\\{app_name}{verb}");
            let entry_key = match classes_key.create(&key_path) {
                Ok(key) => key,
                Err(err) => {
                    report_error!(
                        anyhow::Error::new(err)
                            .context(format!("Could not create context-menu key {key_path}"))
                    );
                    continue;
                }
            };

            // The empty string represents the "(Default)" value for a registry key.
            if let Err(err) = entry_key.set_string("", label) {
                report_error!(
                    anyhow::Error::new(err).context("Could not set context-menu entry label")
                );
                continue;
            }

            // Cosmetic only, so a failure here should not skip the command.
            if let Err(err) = entry_key.set_hstring("Icon", &HSTRING::from(icon.as_os_str())) {
                report_error!(
                    anyhow::Error::new(err).context("Could not set context-menu entry icon")
                );
            }

            let command_key = match entry_key.create("command") {
                Ok(key) => key,
                Err(err) => {
                    report_error!(
                        anyhow::Error::new(err)
                            .context(format!("Could not create {key_path}\\command"))
                    );
                    continue;
                }
            };

            let mut command = OsString::new();
            command.push("\"");
            command.push(exe.as_os_str());
            command.push(format!(
                "\" \"{uri_scheme}://action/{action}?path={path_arg}\""
            ));
            if let Err(err) = command_key.set_hstring("", &HSTRING::from(command.as_os_str())) {
                report_error!(
                    anyhow::Error::new(err).context("Could not set context-menu entry command")
                );
            }
        }
    }
}

pub(super) fn register_uri_handler() {
    // To change the settings for the user, changes must be made under
    // HKEY_CURRENT_USER\Software\Classes instead of under HKEY_CLASSES_ROOT since only an
    // administrator can modify it. It gets merged into HKEY_CLASSES_ROOT later.
    let Ok(classes_key) = CURRENT_USER.open("Software\\Classes") else {
        report_error!("Failed to get current_user\\software\\classes");
        return;
    };

    // The Windows Registry entry for Warp (assuming the channel is WarpLocal):
    // warplocal
    //   (Default) = "WarpLocal"
    //   URL Protocol = ""
    //   DefaultIcon
    //      (Default) = "{path_to_channel_icon},0" TODO(CORE-2860): Add icon file path here.
    //   shell
    //      open
    //         command
    //            (Default) = "{path_to_executable}" "%0"
    let uri_scheme = ChannelState::url_scheme();
    match classes_key.create(uri_scheme) {
        Ok(parent_key) => {
            // The empty string represents the "(Default)" value for a registry key.
            if let Err(err) = parent_key.set_string("", ChannelState::app_id().application_name()) {
                report_error!(
                    anyhow::Error::new(err).context("Could not set URI Scheme display name")
                );
                return;
            }
            if let Err(err) = parent_key.set_string("URL Protocol", "") {
                report_error!(
                    anyhow::Error::new(err).context("Could not set URI Scheme URL Protocol Key")
                );
                return;
            };

            // TODO(CORE-2861): Add the `DefaultIcon` Default value here with the file path to
            // Warp's icon once we figure out distribution on Windows.

            let command_key = match parent_key.create("shell\\open\\command") {
                Ok(command_key) => command_key,
                Err(err) => {
                    report_error!(
                        anyhow::Error::new(err)
                            .context("Could not create shell\\open\\command key")
                    );
                    return;
                }
            };
            let command = match std::env::current_exe() {
                Ok(path) => {
                    let mut command = OsString::new();
                    command.push("\"");
                    command.push(path.as_os_str());
                    command.push("\" \"%0\"");
                    HSTRING::from(command.as_os_str())
                }
                Err(err) => {
                    report_error!(anyhow::Error::new(err).context(
                        "Could not get path to current executable for registering URI scheme"
                    ));
                    return;
                }
            };
            // The empty string represents the "(Default)" value for a registry key.
            if let Err(err) = command_key.set_hstring("", &command) {
                report_error!(
                    anyhow::Error::new(err)
                        .context("Could not set shell command path for URI Scheme")
                );
            }
        }
        Err(err) => {
            report_error!(
                anyhow::Error::new(err).context("Failed to create URI Scheme registry entry")
            );
        }
    }
}
