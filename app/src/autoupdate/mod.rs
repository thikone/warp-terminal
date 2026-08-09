//! Inert auto-update module for the terminal-only build.
//!
//! The original module polled Warp's release servers every 10 minutes, fetched
//! `channel_versions.json`, downloaded platform installers, staged them, and
//! relaunched the app. All of that has been removed:
//!
//! * the network layer (`channel_versions.rs`, `changelog.rs`) is deleted, so
//!   this build never contacts a release server;
//! * the platform installers (`mac.rs`, `linux.rs`, `windows.rs`) are deleted,
//!   so no downloaded payload can ever be executed;
//! * the update UI (title-bar button, avatar-menu entries, the non-dismissible
//!   "your app is out of date" banner, the Settings version widget, and the
//!   `workspace:update_and_relaunch` / `workspace:check_for_updates` bindings)
//!   is deleted.
//!
//! What remains is the type surface the rest of the app still references, with
//! every operation a no-op. [`get_update_state`] always reports
//! [`AutoupdateStage::NoUpdateAvailable`], which is what makes the remaining
//! `match` arms in `terminal/view.rs` and `settings_view/main_page.rs` fall
//! through to "nothing to do".
//!
//! Kept as a stub rather than deleted outright so the app's startup, shutdown,
//! and single-instance paths (`lib.rs`) keep their shape.

use anyhow::Result;
use channel_versions::VersionInfo;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

/// Stage of the (now permanently idle) auto-updater.
///
/// Only [`Self::NoUpdateAvailable`] is ever constructed. The other variants are
/// retained so existing exhaustive `match`es keep compiling.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AutoupdateStage {
    /// The only stage this build ever reports.
    #[default]
    NoUpdateAvailable,
    CheckingForUpdate,
    DownloadingUpdate,
    UnableToUpdateToNewVersion {
        new_version: VersionInfo,
    },
    UpdateReady {
        new_version: VersionInfo,
        update_id: String,
    },
    Updating {
        new_version: VersionInfo,
        update_id: String,
    },
    UnableToLaunchNewVersion {
        new_version: VersionInfo,
    },
    UpdatedPendingRestart {
        new_version: VersionInfo,
    },
}

/// Result of an update check. Never produced as anything but [`Self::No`].
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateReady {
    Yes {
        new_version: VersionInfo,
        update_id: String,
    },
    CanDownload {
        new_version: VersionInfo,
        update_id: String,
    },
    No,
}

/// Whether the app is ready to relaunch. Never produced.
#[allow(dead_code)]
pub enum ReadyForRelaunch {
    Yes,
    No,
}

/// Kind of update check that was requested. No check is ever performed.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestType {
    ManualCheck,
    Poll,
    DailyCheck,
}

/// Events emitted by [`AutoupdateState`]. Never emitted in this build.
#[allow(dead_code)]
pub enum AutoupdateStateEvent {
    CheckComplete {
        result: Box<Result<UpdateReady>>,
        request_type: RequestType,
    },
    UpdateAvailable,
}

/// Singleton retained so `AutoupdateState::handle(ctx)` keeps working; it holds
/// a permanently idle stage and performs no work.
pub struct AutoupdateState;

impl AutoupdateState {
    pub fn new() -> Self {
        Self
    }

    /// Registers the singleton. The `server_api` the real updater used is no
    /// longer needed, so the parameter is ignored.
    pub fn register<T>(ctx: &mut AppContext, _server_api: T) {
        ctx.add_singleton_model(|_ctx| Self::new());
    }

    /// No-op: there is no polling loop, so nothing is ever scheduled and no
    /// request is ever made.
    pub fn start_polling(&mut self, _ctx: &mut ModelContext<Self>) {}

    /// No-op: manual "check for updates" is not reachable from the UI anymore,
    /// and would do nothing if it were.
    pub fn manually_check_for_update(&mut self, _ctx: &mut ModelContext<Self>) {}

    /// No-op counterpart of the old once-a-day check.
    pub fn maybe_daily_check_for_update(&mut self, _ctx: &mut ModelContext<Self>) {}
}

impl Default for AutoupdateState {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for AutoupdateState {
    type Event = AutoupdateStateEvent;
}

impl SingletonEntity for AutoupdateState {}

/// Retained so the shutdown path and the `Workspace` observer keep compiling.
/// No relaunch is ever requested.
#[derive(Clone, Copy, Default)]
pub struct RelaunchModel;

impl RelaunchModel {
    pub fn new() -> Self {
        Default::default()
    }
}

impl Entity for RelaunchModel {
    type Event = ();
}

impl SingletonEntity for RelaunchModel {}

/// Always [`AutoupdateStage::NoUpdateAvailable`].
pub fn get_update_state(_app: &AppContext) -> AutoupdateStage {
    AutoupdateStage::NoUpdateAvailable
}

/// No-op: nothing is ever staged, so there is nothing to relaunch into.
pub fn initiate_relaunch_for_update(_app: &mut AppContext) {}

/// Always `false` — there is never a pending update, so shutdown proceeds
/// normally and `on_update_complete` is never invoked.
pub fn apply_pending_update<F>(_app: &mut AppContext, _on_update_complete: F) -> bool
where
    F: FnOnce(&mut AppContext) + Send + 'static,
{
    false
}

/// No-op: no relaunch can be pending.
pub fn cancel_relaunch(_app: &mut AppContext) {}

/// No-op: the app never re-spawns itself to apply an update.
pub fn spawn_child_if_necessary(_app: &mut AppContext) {}

/// No-op: this build never writes an old executable to clean up.
pub(crate) fn check_and_report_update_errors(_ctx: &mut AppContext) {}

/// No-op: there is never a leftover updated executable to remove.
pub fn remove_old_executable() -> Result<()> {
    Ok(())
}
