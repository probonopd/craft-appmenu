# craft-appmenu

Publishes an application menu bar over the **Canonical AppMenu protocol** (the
`com.canonical.dbusmenu` D-Bus interface, exported at `/MenuBar`) and registers the app's X11
top-level windows with the menu registrar, so global-menu environments (GNOME with the
app-menu shell extension, Unity-like shells, Gershwin Menu) can host the menus natively.
Menu clicks come back as events carrying a plain `action` token.

One of the **Crafting Apps' shared components** (like
[`craft-fonts`](https://github.com/storytold/craft-fonts)): standalone — no workspace
dependencies — and usable by every app in the org. The D-Bus and X11 code is Linux/BSD only;
on every other target (macOS, wasm32) the same API is served by a no-op stub whose `start`
fails gracefully — the in-window menu bar is the only menu there, and callers compile
unchanged. Never panics: every entry point returns a `Result` or a no-op handle; malformed
input from the bus is rejected; the worker thread retires itself on unrecoverable D-Bus
errors instead of crashing the app.

## How it works

1. **Model** (`model.rs`): toolkit-independent menu data — `MenuModel::top(children)`,
   `MenuEntry::command(label, action, enabled)` with builder methods (`.submenu`, `.checked`,
   `.shortcut("Ctrl+S")`), and `MenuEntry::Separator`. Submenus nest freely; every leaf
   carries an `action` token the application defines.
2. **Flat builder** (`flat.rs`): `MenuModel::from_flat(&[FlatItem])` turns the flat item
   list an app's menu catalog already produces — the same list an in-window menu bar
   renders — into the model: top menus keep first-seen order, empty tops are skipped,
   `"---"` rows become separators (never doubled, never leading or trailing), submenu titles
   reuse the nesting rules of a menu bar. Labels and paths are expected pre-translated by
   the caller (this crate does no i18n), so an app implements its menus **once** for both
   hosts.
3. **Flat layout** (`layout.rs`): builds a depth-limited flat item list with dbusmenu item ids
   assigned in reading order, dbusmenu-style properties (label, type, enabled, children-display,
   toggle-type/state, shortcut as `aas`), a property diff between models, and bounded
   traversal (max depth 32, max items 4096 — a cyclic or huge model never hangs the export).
4. **Wire** (`dbus.rs`): the `com.canonical.dbusmenu` interface — `GetLayout` returning
   `(revision, (ia{sv}av))` where each child row is a *variant* of `(ia{sv}av)` (importers
   demarshall children as variants; a mis-shaped row breaks every decoder), plus
   `GetProperty`, `GetGroupProperties`, `GetPropertyNames`, `Event`/`EventGroup` (clicks are
   recorded; unknown or disabled ids are reported back), `AboutToShow` (always false — menus
   are never lazily fetched). Changes are announced with `LayoutUpdated` (structure change)
   or `ItemsPropertiesUpdated` (property-only), so importers never poll.
5. **Registrar** (`registrar.rs`): keeps the X11 window list registered with
   `com.canonical.AppMenu.Registrar` (falling back to the Ayatana
   `org.ayatana.AppMenu.Registrar`); the first of the two that answers wins and stays sticky.
   Importers find windows through the registrar, so no owned bus name is needed for it.
6. **Live exporter** (`live.rs`): one background thread owns one session-bus connection,
   coalesces bursts of models into a single publish, and re-scans/re-registers windows once a
   second (a registrar can appear or disappear at runtime; windows come and go too; a
   registrar that "forgets" is re-registered the very next scan). Windows are found with the
   EWMH `_NET_CLIENT_LIST` on the root — window managers that reparent clients into frames
   would otherwise expose frame ids that never match the active window — with a root-children
   scan as fallback and a 32-window cap. The UI thread only calls `replace` (non-blocking)
   and `try_event`. On an unrecoverable bus error the worker retires itself instead of
   crashing the app.
7. **Hosted state**: `AppMenu::hosted()` reports whether a registrar currently holds a
   registration for one of our windows. While true, a global menu host is *serving* the
   menus and the app can hide its in-window menu bar (the shells bar owns the menus then);
   the flag flips at the same second the host disappears, so the in-window bar returns.

## Example

```rust
use craft_appmenu::{FlatItem, MenuEntry, MenuModel};

// Either build the model by hand…
let model = MenuModel::top(vec![
    MenuEntry::command("File", "", false).submenu(vec![
        MenuEntry::command("New…", "file.new", true).shortcut("Ctrl+N"),
        MenuEntry::Separator,
        MenuEntry::command("Save", "file.save", true).shortcut("Ctrl+S"),
    ]),
]);
// …or from the same flat list the in-window menu bar renders:
let model = MenuModel::from_flat(&[
    FlatItem::command("New…", &["File"], "file.new", true).shortcut("Ctrl+N"),
    FlatItem::separator(&["File"]),
    FlatItem::command("Save", &["File"], "file.save", true),
]);

let menu = craft_appmenu::AppMenu::start("my-app")?;
menu.replace(&model);
// in the frame loop:
while let Some(event) = menu.try_event() {
    // dispatch `event`'s action token like an in-window menu click
}
```

## Checklist for an app adopting it

* Take `craft-appmenu` as a dependency from its repository, pinned exactly (a tag or commit;
  review the diff before bumping — the crate pins its own D-Bus/X11 deps exactly the same
  way).
* Call `AppMenu::start(app_name)` once (lazily, on the first frame is fine), publish on a
  duty cycle or on a dirty flag, drain `try_event` every frame, and mirror `hosted()` into
  the shell.
* Map activations to the same dispatch path an in-window menu click uses, so behaviour is
  identical in both hosts.
* While `hosted()` is true, hide the in-window menu titles (a shell-global menu owns the
  menus then); with no host — no bus, opted out, other platform — never hide it.
* Pick a per-app opt-out env var (`<APP>_NO_GLOBAL_MENU=1`, the PhotoCraft convention) for
  debugging.

## Tests

Unit tests cover the flat→tree builder, the layout building/diff and the wire shapes
(including a byte-level `GetLayout` reply round trip that decodes nested rows exactly as
importers do), plus fake-based tests for the registrar sync (register-once, a registrar that
forgets, windows that close, absent registrar, failed registration). An integration test
exchanges `GetLayout`, `GetProperty` and `Event` clicks against the real session bus
(skipping gracefully when no bus exists, so CI stays green without one).
