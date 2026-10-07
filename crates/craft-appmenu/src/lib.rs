//! # craft-appmenu
//!
//! Publishes an application menu bar over the Canonical AppMenu global-menu protocol
//! ("dbusmenu"): the app serves the `com.canonical.dbusmenu` D-Bus interface and registers
//! its X11 window with the menu registrar (`com.canonical.AppMenu.Registrar`, and its
//! Ayatana fork `org.ayatana.AppMenu.Registrar` when present), so global-menu environments
//! (GNOME with an app-menu shell extension, Unity-like shells, Gershwin Menu) can host the
//! menu natively. Menu clicks come back as [`MenuEvent::Activated`] carrying the item's
//! `action` token from the model.
//!
//! The menu model is plain, toolkit-independent data ([`MenuModel`], [`MenuEntry`]) — no UI
//! crate is involved, so any application can drive it from its own menu tree.
//!
//! ```no_run
//! use craft_appmenu::{MenuEntry, MenuEvent, MenuModel, AppMenu};
//!
//! let model = MenuModel::top(vec![MenuEntry::command("File", "file.new", true)
//!     .submenu(vec![MenuEntry::command("New…", "file.new", true)])]);
//! // Linux/BSD with D-Bus: export and register our X11 top-level windows.
//! // Other platforms: Ok with a no-op handle (activation events never fire).
//! let menu = AppMenu::start("my-app").ok();
//! if let Some(menu) = &menu {
//!     menu.replace(&model);
//!     while let Some(ev) = menu.try_event() {
//!         if let MenuEvent::Activated { label, action, id } = ev {
//!             // dispatch `action` …
//!             println!("menu click: {label} ({action})");
//!         }
//!     }
//! }
//! ```
//!
//! Never panics (never-crash standard): every entry point returns a `Result` or a
//! no-op handle; malformed input from the bus is rejected; the worker thread retires
//! itself on unrecoverable D-Bus errors instead of crashing the app.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

#[cfg(not(all(unix, not(target_os = "macos"), not(target_arch = "wasm32"))))]
pub use stub::AppMenu;
#[cfg(not(all(unix, not(target_os = "macos"), not(target_arch = "wasm32"))))]
mod stub;

#[cfg(all(unix, not(target_os = "macos"), not(target_arch = "wasm32")))]
pub use live::AppMenu;
#[cfg(all(unix, not(target_os = "macos"), not(target_arch = "wasm32")))]
mod dbus;
#[cfg(all(unix, not(target_os = "macos"), not(target_arch = "wasm32")))]
mod layout;
/// The X11 window scan, exposed (hidden from docs) for diagnostics: which windows the
/// export registrar actually learns about.
#[cfg(all(unix, not(target_os = "macos"), not(target_arch = "wasm32")))]
#[doc(hidden)]
pub use x11::top_level_windows;
#[cfg(all(unix, not(target_os = "macos"), not(target_arch = "wasm32")))]
mod live;
#[cfg(all(unix, not(target_os = "macos"), not(target_arch = "wasm32")))]
mod registrar;
#[cfg(all(unix, not(target_os = "macos"), not(target_arch = "wasm32")))]
mod x11;

pub mod flat;
mod model;

/// Everything the exporter can fail with. Failures are always "feature absent" (no D-Bus
/// session or no registrar): the app keeps its in-window menu bar; activations never fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The D-Bus session bus, its services or the X server could not be used.
    #[error("{0}")]
    Platform(String),
}

pub use flat::FlatItem;
pub use model::{MenuCommand, MenuEntry, MenuModel, Shortcut, ShortcutMod};

/// A click the global menu host sent back.
#[derive(Clone, Debug)]
pub enum MenuEvent {
    /// A menu item was activated (`label` is the item's label, `action` its action token,
    /// `id` the dbusmenu item id).
    Activated {
        label: String,
        action: String,
        id: u32,
    },
}
