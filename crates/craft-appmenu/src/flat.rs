//! Building a [`MenuModel`] from the flat item list an app's menu catalog already produces
//! (the same list the in-window menu bar renders), so an application implements its menus
//! *once* and both hosts are structurally identical: same grouping, same order, same
//! separator collapsing, same submenu nesting.
//!
//! The `FlatItem`s are the app's own rows; labels and path segments are expected
//! pre-translated by the caller (this crate knows nothing about i18n). `First-seen` order
//! rules throughout, matching how a menu catalog lists its items contiguously per menu.

use crate::model::{MenuEntry, MenuModel};

/// One row of an app's flat menu list, exactly as the in-window menu bar would render it.
///
/// A row with the label `"---"` is a horizontal rule (the same convention most editors
/// use in their catalogs). Everything else is a leaf: `label` is the shown text, `path`
/// the translated menu titles leading to it (`["File", "Import"]` for File → Import),
/// `action` the token the app receives on activation, plus state and the optional
/// display shortcut (`"Ctrl+Shift+N"`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatItem {
    /// The shown label, already translated.
    pub label: String,
    /// The translated menu titles above the item, empty for a top-level item.
    pub path: Vec<String>,
    /// The action token returned on activation (ignored for separators and submenu titles).
    pub action: String,
    /// Display shortcut, parsed with [`crate::Shortcut::parse`]; unparseable ones are dropped.
    pub shortcut: Option<String>,
    pub enabled: bool,
    /// Checkbox/radio state: `None` for plain items.
    pub checked: Option<bool>,
}

impl FlatItem {
    /// A leaf item at `path` with `label` shown and `action` returned on activation.
    pub fn command(
        label: impl Into<String>,
        path: &[&str],
        action: impl Into<String>,
        enabled: bool,
    ) -> FlatItem {
        FlatItem {
            label: label.into(),
            path: path.iter().map(|s| (*s).to_string()).collect(),
            action: action.into(),
            shortcut: None,
            enabled,
            checked: None,
        }
    }

    /// A horizontal rule at `path`.
    pub fn separator(path: &[&str]) -> FlatItem {
        FlatItem {
            label: "---".to_string(),
            path: path.iter().map(|s| (*s).to_string()).collect(),
            action: String::new(),
            shortcut: None,
            enabled: true,
            checked: None,
        }
    }

    /// Marks the item with a checkbox state.
    pub fn checked(mut self, checked: bool) -> FlatItem {
        self.checked = Some(checked);
        self
    }

    /// Adds a display shortcut (`"Ctrl+Shift+N"`).
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> FlatItem {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Sets the enabled state.
    pub fn enabled(mut self, enabled: bool) -> FlatItem {
        self.enabled = enabled;
        self
    }
}

/// The tree-building rules, one nesting level at a time: leaves sit at `path.len() ==
/// depth`, a submenu appears at the position of its first child, `"---"` means a separator
/// (never doubled, never leading or trailing), and submenu titles are always enabled
/// (opening a menu is never a disabled action; enablement is per item, like in the
/// in-window menu bar). Identical input yields an identical tree to what an in-window bar
/// implementing the same rules renders.
impl MenuModel {
    /// Builds the model: top-level menus keep first-seen order (a catalog lists its menus
    /// contiguously, so this restores the catalog's order); a top menu with no items is
    /// skipped (an in-window bar would show "(coming soon)" instead; the global shell has
    /// no place for a dead title).
    pub fn from_flat(items: &[FlatItem]) -> MenuModel {
        let mut tops: Vec<String> = Vec::new();
        for item in items {
            let Some(top) = item.path.first() else {
                continue;
            };
            if !tops.iter().any(|t| t == top) {
                tops.push(top.clone());
            }
        }
        let tops: Vec<MenuEntry> = tops
            .iter()
            .map(|top| {
                let mine: Vec<&FlatItem> = items
                    .iter()
                    .filter(|i| i.path.first().is_some_and(|p| p == top))
                    .collect();
                MenuEntry::command(top.clone(), "", true).submenu(entries(&mine, 1))
            })
            .collect();
        MenuModel::top(tops)
    }
}

/// One nesting level (see [`MenuModel::from_flat`] for the rules).
fn entries(items: &[&FlatItem], depth: usize) -> Vec<MenuEntry> {
    let mut out: Vec<MenuEntry> = Vec::new();
    let mut shown: Vec<String> = Vec::new();
    let mut last_was_sep = true;
    for (i, it) in items.iter().enumerate() {
        if it.path.len() == depth {
            if it.label == "---" {
                if !last_was_sep && i + 1 < items.len() {
                    out.push(MenuEntry::Separator);
                    last_was_sep = true;
                }
                continue;
            }
            let mut entry = MenuEntry::command(it.label.clone(), it.action.clone(), it.enabled);
            if let Some(c) = it.checked {
                entry = entry.checked(c);
            }
            if let Some(sc) = &it.shortcut {
                entry = entry.shortcut(sc.clone());
            }
            out.push(entry);
            last_was_sep = false;
        } else if it.path.len() > depth {
            let Some(name) = it.path.get(depth) else {
                continue;
            };
            if shown.iter().any(|s| s == name) {
                continue;
            }
            shown.push(name.clone());
            let child: Vec<&FlatItem> = items
                .iter()
                .copied()
                .filter(|c| c.path.len() > depth && c.path.get(depth).is_some_and(|p| p == name))
                .collect();
            let enabled = child.iter().any(|c| c.enabled && c.label != "---") || !child.is_empty();
            out.push(
                MenuEntry::command(name.clone(), "", enabled).submenu(entries(&child, depth + 1)),
            );
            last_was_sep = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tree drawn from the model, one line per entry (as the flat→tree tests print).
    fn tree(entries: &[MenuEntry], depth: usize) -> String {
        let mut out = String::new();
        for entry in entries {
            match entry {
                MenuEntry::Separator => {
                    let _ = std::fmt::Write::write_fmt(
                        &mut out,
                        format_args!("{}|\n", "  ".repeat(depth)),
                    );
                }
                MenuEntry::Command(c) => {
                    let _ = std::fmt::Write::write_fmt(
                        &mut out,
                        format_args!(
                            "{}{}({}){}\n",
                            "  ".repeat(depth),
                            c.label,
                            c.action,
                            c.checked
                                .map(|c| if c { "[✓]" } else { "[ ]]" })
                                .unwrap_or_default(),
                        ),
                    );
                    if let Some(children) = &c.children {
                        out.push_str(&tree(children, depth + 1));
                    }
                }
            }
        }
        out
    }

    fn item(label: &str, action: &str, path: &[&str]) -> FlatItem {
        FlatItem::command(label, path, action, true)
    }

    #[test]
    fn the_flat_item_list_becomes_the_same_tree_the_menu_bar_renders() {
        let items = vec![
            item("New…", "file.new", &["File"]),
            FlatItem::separator(&["File"]),
            item("Save", "file.save", &["File"]),
            item("Undo", "edit.undo", &["Edit"]),
            item("Zoom In", "view.zoomIn", &["View"]),
            item("Show Grid", "view.grid", &["View", "Extras"]),
            item("Show Rulers", "view.rulers", &["View", "Extras"]),
        ];
        let printed = tree(&MenuModel::from_flat(&items).children, 0);
        let expected = "File()\n  New…(file.new)\n  |\n  Save(file.save)\n\
Edit()\n  Undo(edit.undo)\nView()\n  Zoom In(view.zoomIn)\n  Extras()\n    Show Grid(view.grid)\n    Show Rulers(view.rulers)\n";
        assert_eq!(printed, expected, "{printed}");
    }

    #[test]
    fn adjacent_separators_collapse_and_empty_tops_are_skipped() {
        let items = vec![
            item("New…", "file.new", &["File"]),
            FlatItem::separator(&["File"]),
            FlatItem::separator(&["File"]),
            FlatItem::separator(&["Edit"]), // nothing before it in its menu: suppressed
            item("Undo", "edit.undo", &["Edit"]),
            // No items at all in "Type": the top menu stays away.
        ];
        let printed = tree(&MenuModel::from_flat(&items).children, 0);
        // Two adjacent "---" become one, a separator with nothing printed before it in its
        // level is suppressed, and a top without items is skipped.
        assert_eq!(
            printed, "File()\n  New…(file.new)\n  |\nEdit()\n  Undo(edit.undo)\n",
            "{printed}"
        );
    }

    #[test]
    fn tops_keep_first_seen_order_and_states_survive() {
        let items = vec![
            item("Zoom In", "view.zoomIn", &["View"]),
            FlatItem::command("New…", &["File"], "file.new", true).shortcut("Ctrl+N"),
            FlatItem::command("Show Grid", &["View", "Extras"], "view.grid", true).checked(true),
            FlatItem::command("Save", &["File"], "file.save", true),
            FlatItem::command("Forgot", &[], "", false), // a path-less row never becomes a menu
        ];
        let printed = tree(&MenuModel::from_flat(&items).children, 0);
        assert_eq!(
            printed,
            "View()\n  Zoom In(view.zoomIn)\n  Extras()\n    Show Grid(view.grid)[✓]\n\
File()\n  New…(file.new)\n  Save(file.save)\n",
            "{printed}"
        );
    }

    #[test]
    fn malformed_input_never_panics() {
        // Empty lists, separators as tops, paths that skip levels: all tolerated.
        let empty = MenuModel::from_flat(&[]).children;
        assert!(empty.is_empty());
        let items = vec![
            FlatItem::separator(&[]), // no path: the row is skipped, not a menu
            item("Deep", "x.y", &["A", "B", "C", "D"]),
            FlatItem::separator(&["Type"]),
        ];
        let printed = tree(&MenuModel::from_flat(&items).children, 0);
        // An odd-shaped path still produces a well-formed, safely renderable tree, and a
        // top menu whose first row is a separator stays present without misbehaving.
        assert!(printed.contains("Type"), "{printed}");
        assert!(printed.contains("Deep"), "{printed}");
    }
}
