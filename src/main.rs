use std::collections::{BTreeMap, BTreeSet};
use zellij_tile::prelude::*;
use actions::{Action, SearchDirection, SearchOption};


const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const SCROLLOFF: usize = 3;


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ActionCategory {
    FocusPane,
    FocusTab,
    MovePane,
    MoveTab,
    NewPane,
    ClosePane,
    NewTab,
    CloseTab,
    RenamePane,
    RenameTab,
    Toggle,
    ResizeGrow,
    ResizeShrink,
    ResizeOther,
    Layout,
    Scroll,
    Search,
    Copy,
    Session,
    Plugin,
    ModeSwitching,
    Other,
}

fn categorize_action(action: &Action) -> ActionCategory {
    match action {
        Action::MoveFocus(_)
        | Action::FocusNextPane | Action::FocusPreviousPane
        | Action::SwitchFocus
            => ActionCategory::FocusPane,

        Action::MoveFocusOrTab(_)
        | Action::GoToTab(_) | Action::GoToTabName(..)
        | Action::GoToNextTab | Action::GoToPreviousTab | Action::ToggleTab
            => ActionCategory::FocusTab,

        Action::MovePane(_) | Action::MovePaneBackwards
            => ActionCategory::MovePane,

        Action::MoveTab(_)
            => ActionCategory::MoveTab,

        Action::NewPane(..) | Action::NewFloatingPane(..) | Action::NewTiledPane(..)
        | Action::NewInPlacePane(..)
            => ActionCategory::NewPane,

        Action::CloseFocus
            => ActionCategory::ClosePane,

        Action::NewTab(..)
            => ActionCategory::NewTab,

        Action::CloseTab
            => ActionCategory::CloseTab,

        Action::PaneNameInput(_) | Action::UndoRenamePane
            => ActionCategory::RenamePane,

        Action::TabNameInput(_) | Action::UndoRenameTab
            => ActionCategory::RenameTab,

        Action::TogglePaneEmbedOrFloating | Action::ToggleFloatingPanes
        | Action::ToggleFocusFullscreen | Action::TogglePaneFrames
        | Action::ToggleActiveSyncTab | Action::ToggleMouseMode
            => ActionCategory::Toggle,

        Action::Resize(Resize::Increase, _) => ActionCategory::ResizeGrow,
        Action::Resize(Resize::Decrease, _) => ActionCategory::ResizeShrink,
        Action::BreakPane | Action::BreakPaneRight | Action::BreakPaneLeft
            => ActionCategory::ResizeOther,

        Action::PreviousSwapLayout | Action::NextSwapLayout
            => ActionCategory::Layout,

        Action::ScrollUp | Action::ScrollDown
        | Action::ScrollToTop | Action::ScrollToBottom
        | Action::PageScrollUp | Action::PageScrollDown
        | Action::HalfPageScrollUp | Action::HalfPageScrollDown
            => ActionCategory::Scroll,

        Action::Search(_) | Action::SearchInput(_) | Action::SearchToggleOption(_)
            => ActionCategory::Search,

        Action::Copy | Action::EditScrollback | Action::ClearScreen
            => ActionCategory::Copy,

        Action::Detach | Action::Quit | Action::RenameSession(_)
            => ActionCategory::Session,

        Action::LaunchOrFocusPlugin(..) | Action::LaunchPlugin(..)
        | Action::StartOrReloadPlugin(_)
            => ActionCategory::Plugin,

        Action::SwitchToMode(_) | Action::SwitchModeForAllClients(_)
            => ActionCategory::ModeSwitching,

        _ => ActionCategory::Other,
    }
}

fn primary_category(actions: &[Action]) -> ActionCategory {
    actions.iter()
        .map(categorize_action)
        .min()
        .unwrap_or(ActionCategory::Other)
}


#[derive(Clone)]
struct Binding {
    action_label: String,
    key_label: String,
    category: ActionCategory,
    direction_order: u8,
}

fn direction_sort_order(actions: &[Action]) -> u8 {
    for a in actions {
        match a {
            Action::MoveFocus(d) | Action::MoveFocusOrTab(d) | Action::MoveTab(d) => {
                return dir_order(d);
            }
            Action::MovePane(Some(d)) | Action::Resize(_, Some(d)) => {
                return dir_order(d);
            }
            Action::NewPane(Some(d), _, _) => {
                return dir_order(d);
            }
            Action::ScrollUp | Action::PageScrollUp | Action::HalfPageScrollUp => return 2,
            Action::ScrollDown | Action::PageScrollDown | Action::HalfPageScrollDown => return 1,
            _ => {}
        }
    }
    4 // no direction
}

fn dir_order(d: &Direction) -> u8 {
    match d {
        Direction::Left => 0,
        Direction::Down => 1,
        Direction::Up => 2,
        Direction::Right => 3,
    }
}

#[derive(Clone)]
struct Section {
    mode: InputMode,
    bindings: Vec<Binding>,
}



#[derive(Clone)]
enum LineType {
    SectionHeader(InputMode),
    Binding,
    Empty,
}

#[derive(Clone)]
struct RenderLine {
    text: String,
    line_type: LineType,
}


#[derive(Default, PartialEq)]
enum PluginMode {
    #[default]
    Browse,
    Search,
    Help,
}


const GUTTER_WIDTH: usize = 2;  // fixed left gutter: indicator + space
const CURSOR_CHAR: char = '❯';  // customizable cursor indicator
const MAX_WIDTH: usize = 55;    // max content width to keep layout tight


#[derive(Default, Clone, PartialEq)]
enum ModifierStyle {
    #[default]
    Text,   // Ctrl Alt Shift Super  (verbose, universal)
    MacOS,  // ⌃⌥⇧⌘                 (compact, macOS symbols)
    Linux,  // C- A- S- S-           (compact, vim-like)
}

impl ModifierStyle {
    fn ctrl(&self) -> &'static str {
        match self {
            ModifierStyle::Text => "Ctrl",
            ModifierStyle::MacOS => "⌃",
            ModifierStyle::Linux => "C",
        }
    }
    fn alt(&self) -> &'static str {
        match self {
            ModifierStyle::Text => "Alt",
            ModifierStyle::MacOS => "⌥",
            ModifierStyle::Linux => "A",
        }
    }
    fn shift(&self) -> &'static str {
        match self {
            ModifierStyle::Text => "Shift",
            ModifierStyle::MacOS => "⇧",
            ModifierStyle::Linux => "S",
        }
    }
    fn super_key(&self) -> &'static str {
        match self {
            ModifierStyle::Text => "Super",
            ModifierStyle::MacOS => "⌘",
            ModifierStyle::Linux => "Su",
        }
    }
}

struct State {
    current_mode: InputMode,
    sections: Vec<Section>,
    palette: Option<Palette>,
    modifier_style: ModifierStyle,

    total_bindings: usize,
    global_max_key: usize,
    global_max_action: usize,
    plugin_id: Option<u32>,

    plugin_mode: PluginMode,
    cursor: usize,
    scroll_offset: usize,
    collapsed: BTreeSet<InputMode>,
    search_query: String,
    rows: usize,
    cols: usize,

    lines: Vec<RenderLine>,
    section_header_indices: Vec<usize>,
    initial_jump_done: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            current_mode: InputMode::Normal,
            sections: Vec::new(),
            palette: None,
            modifier_style: ModifierStyle::default(),
            total_bindings: 0,
            global_max_key: 0,
            global_max_action: 0,
            plugin_id: None,
            plugin_mode: PluginMode::Browse,
            cursor: 0,
            scroll_offset: 0,
            collapsed: BTreeSet::new(),
            search_query: String::new(),
            rows: 0,
            cols: 0,
            lines: Vec::new(),
            section_header_indices: Vec::new(),
            initial_jump_done: false,
        }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[EventType::ModeUpdate, EventType::Key, EventType::Mouse]);
        set_selectable(true);

        self.modifier_style = match configuration.get("modifier_style").map(|s| s.as_str()) {
            Some("macos") => ModifierStyle::MacOS,
            Some("linux") => ModifierStyle::Linux,
            _ => ModifierStyle::Text,
        };
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::ModeUpdate(mode_info) => {
                let new_palette = Palette::from(mode_info.style.colors);
                let same_mode = self.current_mode == mode_info.mode;
                let same_palette = self.palette.map_or(false, |p| p == new_palette);

                // Skip if nothing changed
                if same_mode && same_palette && !self.sections.is_empty() {
                    return false;
                }

                let prev_mode = self.current_mode;
                self.current_mode = mode_info.mode;
                self.palette = Some(new_palette);

                let first_load = self.sections.is_empty();
                if first_load {
                    self.sections = parse_sections(&mode_info.keybinds, &self.modifier_style);
                    let (mut total, mut max_k, mut max_a) = (0usize, 0usize, 0usize);
                    for s in &self.sections {
                        if s.mode != self.current_mode {
                            self.collapsed.insert(s.mode);
                        }
                        for b in &s.bindings {
                            total += 1;
                            max_k = max_k.max(b.key_label.chars().count());
                            max_a = max_a.max(b.action_label.chars().count());
                        }
                    }
                    self.total_bindings = total;
                    self.global_max_key = max_k.max(10);
                    self.global_max_action = max_a.max(10);
                    let pid = get_plugin_ids().plugin_id;
                    self.plugin_id = Some(pid);
                    rename_plugin_pane(pid, "⌨ Keymap");
                }

                if !first_load && prev_mode != self.current_mode {
                    self.collapsed.insert(prev_mode);
                    self.collapsed.remove(&self.current_mode);
                    self.search_query.clear();
                    self.plugin_mode = PluginMode::Browse;
                }

                self.invalidate_lines();
                self.resize_to_fit();
                true
            }
            Event::Key(key) => {
                if self.handle_key(key) {
                    self.invalidate_lines();
                    true
                } else {
                    false
                }
            }
            Event::Mouse(mouse) => {
                match mouse {
                    Mouse::ScrollUp(_) => { self.move_cursor_up(3); self.invalidate_lines(); true }
                    Mouse::ScrollDown(_) => { self.move_cursor_down(3); self.invalidate_lines(); true }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.rows = rows;

        let palette = match self.palette {
            Some(p) if !self.sections.is_empty() => p,
            _ => return,
        };

        let effective = cols.min(MAX_WIDTH);
        self.cols = effective;
        let lpad = cols.saturating_sub(effective) / 2;
        let pad = " ".repeat(lpad);

        if effective < 30 || rows < 6 {
            return;
        }

        if self.lines.is_empty() {
            self.rebuild_lines();
            if !self.initial_jump_done {
                self.initial_jump_done = true;
                self.jump_to_current_mode_section();
            }
        }

        // ── Help overlay ──
        if self.plugin_mode == PluginMode::Help {
            let b = |s: &str| format!("{}{}{}", fg_color(&palette.blue), s, RESET);
            let d = |s: &str| format!("{}{}{}", DIM, s, RESET);
            let sh = |s: &str| format!("{}  {}{}{}", pad, BOLD, s, RESET);

            let mut help: Vec<String> = Vec::new();

            let k = |s: &str| -> String {
                s.split('/')
                    .map(|p| format!("{}{}{}", fg_color(&palette.blue), p, RESET))
                    .collect::<Vec<_>>()
                    .join("/")
            };

            let row = |colored: &str, vis_len: usize, col: usize, desc: &str| -> String {
                let gap = if vis_len < col { col - vis_len } else { 1 };
                format!("{}    {}{}{}", pad, colored, " ".repeat(gap), desc)
            };

            // Navigation (align at col 22)
            let c = 22;
            let nav1 = format!("{} {} {}", k("j/k"), d("or"), k("↑/↓"));
            let nav2 = format!("{} {} {}", k("Ctrl+d/u"), d("or"), k("PgDn/Up"));
            help.push(sh("Navigation"));
            help.push(row(&nav1, 10, c, "scroll up/down"));
            help.push(row(&nav2, 19, c, "half page"));
            help.push(row(&k("g/G"), 3, c, "top/bottom"));
            help.push(String::new());

            // Sections (align at col 16)
            let c = 16;
            let sec1 = format!("{}/{}", b("Tab"), b("Shift+Tab"));
            let sec2 = format!("{} {} {}", k("l/h"), d("or"), k("→/←"));
            help.push(sh("Sections"));
            help.push(row(&sec1, 13, c, "next/prev"));
            help.push(row(&sec2, 10, c, "expand/collapse"));
            help.push(row(&k("e/c"), 3, c, "expand/collapse all"));
            help.push(String::new());

            // Search & Exit (align at col 5)
            let c = 5;
            help.push(sh("Search & Exit"));
            help.push(row(&b("?"), 1, c, "search"));
            help.push(row(&b("Esc"), 3, c, "close / clear filter"));
            help.push(row(&b("q"), 1, c, "quit"));

            let (legend_str, _) = modifier_legend(&self.modifier_style, &palette);
            if !legend_str.is_empty() {
                help.push(String::new());
                help.push(sh("Modifier Symbols"));
                help.push(format!("{}  {}", pad, legend_str));
            }

            for line in &help { println!("{}", line); }
            for _ in help.len()..rows.saturating_sub(1) { println!(); }
            print!("{}  {}Press any key to close{}", pad, DIM, RESET);
            return;
        }

        // ── Header (search only) ──
        let has_search_bar = self.plugin_mode == PluginMode::Search;
        if has_search_bar {
            let match_count = self.lines.iter()
                .filter(|l| matches!(l.line_type, LineType::Binding)).count();
            println!(
                "{} {} Search: {}{}{}  {}({}/{}){} ",
                pad,
                fg_color(&palette.yellow),
                fg_color(&palette.green),
                self.search_query,
                RESET,
                DIM, match_count, self.total_bindings, RESET,
            );
            println!("{}{}{}{}", pad, fg_color(&palette.gray), "─".repeat(effective), RESET);
        }

        // ── Content ──
        let header_rows = if has_search_bar { 2 } else { 0 };
        let content_rows = rows.saturating_sub(header_rows + 2);
        let total = self.lines.len();

        self.adjust_scroll(content_rows);

        let offset = self.scroll_offset;
        for (i, line) in self.lines.iter().enumerate().skip(offset).take(content_rows) {
            let is_cursor = i == self.cursor
                && self.plugin_mode == PluginMode::Browse
                && self.is_selectable(i);
            if is_cursor {
                print!("{}{}{}{}{} ", pad, BOLD, fg_color(&palette.green), CURSOR_CHAR, RESET);
            } else {
                print!("{}  ", pad);
            }
            println!("{}", line.text);
        }

        let rendered = total.saturating_sub(offset).min(content_rows);
        for _ in rendered..content_rows {
            println!();
        }

        // ── Footer ──
        println!("{}{}{}{}", pad, fg_color(&palette.gray), "─".repeat(effective), RESET);

        let b = |s: &str| format!("{}{}{}", fg_color(&palette.blue), s, RESET);
        let style = &self.modifier_style;

        if self.plugin_mode == PluginMode::Search {
            print!(
                "{} {} navigate  {} confirm  {} cancel",
                pad, b("↑↓"), b("↵"), b("Esc"),
            );
        } else {
            let (legend, legend_len) = modifier_legend(style, &palette);
            let f1_text = format!("{} help", b("F1"));
            let f1_len = 7;
            let gap = effective.saturating_sub(f1_len + legend_len + 2);
            print!("{} {}{}{}", pad, f1_text, " ".repeat(gap), legend);
        }
    }
}


impl State {
    fn invalidate_lines(&mut self) {
        self.lines.clear();
    }

    fn set_pane_coords(&self, y: &str, height: &str) {
        if let Some(pid) = self.plugin_id {
            let coords = FloatingPaneCoordinates::new(
                None, Some(y.to_string()), None, Some(height.to_string()), None,
            );
            if let Some(c) = coords {
                change_floating_panes_coordinates(vec![(PaneId::Plugin(pid), c)]);
            }
        }
    }

    fn resize_to_fit(&mut self) {
        if self.plugin_id.is_none() || self.sections.is_empty() { return; }

        if self.current_mode == InputMode::Normal {
            self.set_pane_coords("15%", "70%");
            return;
        }

        let mut height = 0usize;
        for s in &self.sections {
            if s.mode == self.current_mode || !self.collapsed.contains(&s.mode) {
                let mut cats = 0usize;
                let mut prev_cat: Option<ActionCategory> = None;
                for b in &s.bindings {
                    if prev_cat.map_or(false, |pc| pc != b.category) { cats += 1; }
                    prev_cat = Some(b.category);
                }
                height += 1 + s.bindings.len() + cats;
            } else {
                height += 1;
            }
            height += 1;
        }
        height += 4;

        self.set_pane_coords("1", &format!("{}", height.max(10)));
    }

    fn reset_view(&mut self) {
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        match self.plugin_mode {
            PluginMode::Help => {
                self.plugin_mode = PluginMode::Browse;
                true
            }
            PluginMode::Search => self.handle_search_key(key),
            PluginMode::Browse => self.handle_browse_key(key),
        }
    }

    fn handle_browse_key(&mut self, key: KeyWithModifier) -> bool {
        let has_ctrl = key.key_modifiers.contains(&KeyModifier::Ctrl);
        let has_shift = key.key_modifiers.contains(&KeyModifier::Shift);

        match key.bare_key {
            BareKey::Char('j') | BareKey::Down => self.move_cursor_down(1),
            BareKey::Char('k') | BareKey::Up => self.move_cursor_up(1),
            BareKey::Char('d') if has_ctrl => self.move_cursor_down(self.rows / 2),
            BareKey::Char('u') if has_ctrl => self.move_cursor_up(self.rows / 2),
            BareKey::PageDown => self.move_cursor_down(self.rows.saturating_sub(4)),
            BareKey::PageUp => self.move_cursor_up(self.rows.saturating_sub(4)),
            BareKey::Char('g') => self.cursor = 0,
            BareKey::Char('G') => {
                self.cursor = self.lines.len().saturating_sub(1);
            }
            BareKey::Tab if has_shift => self.jump_to_prev_section(),
            BareKey::Tab => self.jump_to_next_section(),
            BareKey::Char(' ') | BareKey::Enter => self.toggle_section_at_cursor(),
            BareKey::Char('l') | BareKey::Right => self.expand_section_at_cursor(),
            BareKey::Char('h') | BareKey::Left => self.collapse_section_at_cursor(),
            BareKey::Char('e') => {
                self.collapsed.clear();
                self.rebuild_lines();
            }
            BareKey::Char('c') => {
                for s in &self.sections {
                    if s.mode != self.current_mode {
                        self.collapsed.insert(s.mode);
                    }
                }
                self.rebuild_lines();
            }
            BareKey::F(1) => { self.plugin_mode = PluginMode::Help; }
            BareKey::Char('?') => {
                self.plugin_mode = PluginMode::Search;
                self.search_query.clear();
            }
            BareKey::Esc => {
                if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.reset_view();
                } else {
                    hide_self();
                }
            }
            BareKey::Char('q') => { hide_self(); }
            _ => { return false; }
        }
        true
    }

    fn handle_search_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Esc => {
                self.plugin_mode = PluginMode::Browse;
                self.search_query.clear();
                self.reset_view();
            }
            BareKey::Enter => { self.plugin_mode = PluginMode::Browse; }
            BareKey::Backspace => {
                self.search_query.pop();
                self.reset_view();
            }
            BareKey::Down => self.move_cursor_down(1),
            BareKey::Up => self.move_cursor_up(1),
            BareKey::Char(c) => {
                self.search_query.push(c);
                self.reset_view();
            }
            _ => { return false; }
        }
        true
    }

    fn move_cursor_down(&mut self, n: usize) {
        let max = self.lines.len().saturating_sub(1);
        let mut remaining = n;
        let mut pos = self.cursor;
        while remaining > 0 && pos < max {
            pos += 1;
            if self.is_selectable(pos) {
                remaining -= 1;
            }
        }
        self.cursor = pos;
        // Snap to nearest selectable if landed on non-selectable
        self.snap_cursor_to_selectable_down();
    }

    fn move_cursor_up(&mut self, n: usize) {
        let mut remaining = n;
        let mut pos = self.cursor;
        while remaining > 0 && pos > 0 {
            pos -= 1;
            if self.is_selectable(pos) {
                remaining -= 1;
            }
        }
        self.cursor = pos;
        self.snap_cursor_to_selectable_up();
    }

    fn is_selectable(&self, idx: usize) -> bool {
        matches!(
            self.lines.get(idx).map(|l| &l.line_type),
            Some(LineType::SectionHeader(_)) | Some(LineType::Binding)
        )
    }

    fn snap_cursor_to_selectable_down(&mut self) {
        let max = self.lines.len();
        while self.cursor < max && !self.is_selectable(self.cursor) {
            self.cursor += 1;
        }
        if self.cursor >= max && max > 0 {
            self.cursor = max - 1;
            self.snap_cursor_to_selectable_up();
        }
    }

    fn snap_cursor_to_selectable_up(&mut self) {
        while self.cursor > 0 && !self.is_selectable(self.cursor) {
            self.cursor -= 1;
        }
    }

    fn adjust_scroll(&mut self, viewport: usize) {
        if viewport == 0 { return; }
        let total = self.lines.len();
        if total <= viewport {
            self.scroll_offset = 0;
            return;
        }
        let max_offset = total.saturating_sub(viewport);
        let margin = SCROLLOFF.min(viewport / 4);

        // Keep cursor within viewport with margin
        let top_bound = self.scroll_offset + margin;
        let bottom_bound = self.scroll_offset + viewport - 1 - margin;

        if self.cursor < top_bound {
            self.scroll_offset = self.cursor.saturating_sub(margin);
        } else if self.cursor > bottom_bound {
            self.scroll_offset = self.cursor + margin + 1 - viewport;
        }

        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    fn jump_to_current_mode_section(&mut self) {
        // Find the section header line for the current mode
        for &idx in &self.section_header_indices {
            if let Some(line) = self.lines.get(idx) {
                if let LineType::SectionHeader(mode) = line.line_type {
                    if mode == self.current_mode {
                        self.cursor = idx;
                        self.scroll_offset = idx;
                        return;
                    }
                }
            }
        }
        // Fallback: top
        self.cursor = 0;
        self.scroll_offset = 0;
        self.snap_cursor_to_selectable_down();
    }

    fn jump_to_next_section(&mut self) {
        if self.section_header_indices.is_empty() { return; }
        for &idx in &self.section_header_indices {
            if idx > self.cursor {
                self.cursor = idx;
                return;
            }
        }
        // Wrap to first
        self.cursor = self.section_header_indices[0];
    }

    fn jump_to_prev_section(&mut self) {
        if self.section_header_indices.is_empty() { return; }
        for &idx in self.section_header_indices.iter().rev() {
            if idx < self.cursor {
                self.cursor = idx;
                return;
            }
        }
        // Wrap to last
        self.cursor = *self.section_header_indices.last().unwrap();
    }

    fn collapse_key_at_cursor(&self) -> Option<InputMode> {
        self.lines.get(self.cursor).and_then(|l| match l.line_type {
            LineType::SectionHeader(mode) => Some(mode),
            _ => None,
        })
    }

    fn toggle_section_at_cursor(&mut self) {
        if let Some(key) = self.collapse_key_at_cursor() {
            if self.collapsed.contains(&key) {
                self.collapsed.remove(&key);
            } else {
                self.collapsed.insert(key);
            }
        }
    }

    fn expand_section_at_cursor(&mut self) {
        if let Some(key) = self.collapse_key_at_cursor() {
            self.collapsed.remove(&key);
        }
    }

    fn collapse_section_at_cursor(&mut self) {
        if let Some(key) = self.collapse_key_at_cursor() {
            self.collapsed.insert(key);
        }
    }

    // ── Line building ───────────────────────────────────────────────────────

    fn rebuild_lines(&mut self) {
        let palette = match self.palette {
            Some(p) => p,
            None => return,
        };
        let cols = if self.cols > 0 { self.cols } else { 80 };
        let searching = !self.search_query.is_empty();
        let query = self.search_query.to_lowercase();


        let mut lines = Vec::new();
        let mut header_indices = Vec::new();

        // Order: current mode first, then others
        let mut ordered: Vec<usize> = Vec::new();
        for (i, s) in self.sections.iter().enumerate() {
            if s.mode == self.current_mode { ordered.push(i); }
        }
        for (i, s) in self.sections.iter().enumerate() {
            if s.mode != self.current_mode { ordered.push(i); }
        }

        for idx in ordered {
            let section = &self.sections[idx];
            let mode = section.mode;

            let is_current = mode == self.current_mode;
            let is_collapsed = self.collapsed.contains(&mode) && !searching;

            // Filter bindings if searching
            let filtered: Vec<&Binding> = if searching {
                section.bindings.iter()
                    .filter(|b| {
                        b.action_label.to_lowercase().contains(&query)
                            || b.key_label.to_lowercase().contains(&query)
                    })
                    .collect()
            } else {
                section.bindings.iter().collect()
            };

            if searching && filtered.is_empty() {
                continue;
            }

            // Section header
            header_indices.push(lines.len());
            let display_name = format_mode(mode).to_uppercase();
            let icon = mode_icon(mode);
            let count = section.bindings.len();
            let name_len = display_name.len();

            // Available width after gutter
            let hw = cols.saturating_sub(GUTTER_WIDTH);

            let lt = LineType::SectionHeader(mode);

            let header_color = if is_current { fg_color(&palette.green) } else { fg_color(&palette.fg) };

            if is_collapsed {
                let count_str = format!("({})", count);
                let rule_len = hw.saturating_sub(name_len + count_str.len() + 7);
                let header = format!(
                    "{}{}▸ {} {} {}{} {}{}",
                    if is_current { BOLD } else { "" },
                    header_color,
                    icon,
                    display_name,
                    RESET,
                    DIM,
                    format!("{} {}", count_str, "─".repeat(rule_len)),
                    RESET,
                );
                lines.push(RenderLine { text: header, line_type: lt });
                continue;
            }

            let rule_len = hw.saturating_sub(name_len + 6);
            let header_text = format!(
                "{}{}▾ {} {} {}{}",
                BOLD,
                header_color,
                icon,
                display_name,
                "─".repeat(rule_len),
                RESET,
            );
            lines.push(RenderLine { text: header_text, line_type: lt });

            // Bindings with grouping
            let bindings_to_render: Vec<&Binding> = if searching {
                filtered.into_iter().collect()
            } else {
                section.bindings.iter().collect()
            };

            let available = cols.saturating_sub(GUTTER_WIDTH + 3);
            let content_width = (self.global_max_action + 4 + self.global_max_key).min(available);
            let action_col = self.global_max_action.min(available.saturating_sub(self.global_max_key + 4));

            // Split bindings into groups by category
            let mut groups: Vec<Vec<&Binding>> = Vec::new();
            let mut current_group: Vec<&Binding> = Vec::new();
            let mut prev_cat: Option<ActionCategory> = None;
            for b in &bindings_to_render {
                if let Some(pc) = prev_cat {
                    if pc != b.category {
                        if !current_group.is_empty() {
                            groups.push(current_group);
                            current_group = Vec::new();
                        }
                    }
                }
                prev_cat = Some(b.category);
                current_group.push(b);
            }
            if !current_group.is_empty() {
                groups.push(current_group);
            }

            let bracket_color = fg_color(&palette.blue);

            for group in groups.iter() {
                let group_len = group.len();

                // Find common prefix within group
                let common_prefix = if group_len > 1 {
                    let first = &group[0].action_label;
                    let mut prefix_len = first.chars().count();
                    for b in group.iter().skip(1) {
                        let match_len = first.chars().zip(b.action_label.chars())
                            .take_while(|(a, c)| a == c)
                            .count();
                        prefix_len = prefix_len.min(match_len);
                    }
                    // Trim to last space boundary (work in chars, not bytes)
                    let prefix_chars: Vec<char> = first.chars().take(prefix_len).collect();
                    // Find last space position in char indices
                    let last_space = prefix_chars.iter().rposition(|c| *c == ' ');
                    match last_space {
                        Some(i) if i > 0 => prefix_chars[..=i].iter().collect::<String>(),
                        _ => String::new(),
                    }
                } else {
                    String::new()
                };
                let prefix_chars = common_prefix.chars().count();

                for (bi, b) in group.iter().enumerate() {
                    let bracket = if group_len == 1 {
                        "─"
                    } else if bi == 0 {
                        "╭"
                    } else if bi == group_len - 1 {
                        "╰"
                    } else {
                        "│"
                    };

                    let action_display = if bi == 0 || common_prefix.is_empty() {
                        truncate(&b.action_label, action_col)
                    } else {
                        let suffix: String = b.action_label.chars().skip(prefix_chars).collect();
                        format!("{}{}", " ".repeat(prefix_chars), suffix)
                    };

                    let key_display = &b.key_label;
                    let action_chars = action_display.chars().count();
                    let key_chars = key_display.chars().count();
                    let used = action_chars + key_chars;
                    let gap = content_width.saturating_sub(used);

                    let colored_key = color_key_label(key_display, &palette, is_current);

                    let line_text = format!(
                        "{}{}{} {}{}{}{}{}",
                        bracket_color,
                        bracket,
                        RESET,
                        fg_color(&palette.fg),
                        action_display,
                        RESET,
                        " ".repeat(gap),
                        colored_key,
                    );

                    lines.push(RenderLine {
                        text: line_text,
                        line_type: LineType::Binding,
                    });
                }
            }

            // Blank line after section
            lines.push(RenderLine {
                text: String::new(),
                line_type: LineType::Empty,
            });
        }

        self.lines = lines;
        self.section_header_indices = header_indices;

        // Clamp cursor
        if !self.lines.is_empty() {
            self.cursor = self.cursor.min(self.lines.len() - 1);
        }
    }
}


type KeybindsVec = Vec<(InputMode, Vec<(KeyWithModifier, Vec<Action>)>)>;

/// Normalize modes that should be merged into one section.
fn normalize_mode(mode: InputMode) -> InputMode {
    match mode {
        InputMode::EnterSearch => InputMode::Search,
        InputMode::RenameTab => InputMode::Tab,
        InputMode::RenamePane => InputMode::Pane,
        other => other,
    }
}

fn parse_sections(keybinds: &KeybindsVec, style: &ModifierStyle) -> Vec<Section> {
    let mut section_map: BTreeMap<InputMode, Vec<Binding>> = BTreeMap::new();

    for (mode, binds) in keybinds {
        let normalized = normalize_mode(*mode);
        let bindings = section_map.entry(normalized).or_default();

        for (key, actions) in binds {
            let action_str = format_actions(actions);
            if action_str.is_empty() || action_str == "NoOp" {
                continue;
            }
            let key_str = format_key(key, style);
            let cat = primary_category(actions);
            let dir_ord = direction_sort_order(actions);

            if let Some(existing) = bindings.iter_mut().find(|b: &&mut Binding| b.action_label == action_str) {
                existing.key_label = merge_key_labels(&existing.key_label, &key_str);
            } else {
                bindings.push(Binding {
                    action_label: action_str,
                    key_label: key_str,
                    category: cat,
                    direction_order: dir_ord,
                });
            }
        }
    }

    let mut sections = Vec::new();
    for (mode, mut bindings) in section_map {
        bindings.sort_by(|a, b| {
            a.category.cmp(&b.category)
                .then(a.direction_order.cmp(&b.direction_order))
        });
        if !bindings.is_empty() {
            sections.push(Section { mode, bindings });
        }
    }
    sections
}

/// Merge two key labels with common prefix.
/// e.g. "Alt+h" + "Alt+←" → "Alt+h/←"
///      "Ctrl+d" + "Alt+d" → "Ctrl+d  Alt+d" (no common prefix)
/// Split a key label into (modifier_prefix, bare_key) at the last separator.
/// Tries "+", "-", then falls back to last single-char boundary for macOS style.
/// Split "⌥ + h" into ("⌥ +", "h") — prefix includes up to last separator, bare is the key part.
fn split_modifier_and_bare(s: &str) -> (String, String) {
    // Find the last separator pattern: " + ", "+", " - ", "-"
    for sep in [" + ", "+", " - ", "-"] {
        if let Some(i) = s.rfind(sep) {
            let prefix = s[..i + sep.len()].trim_end().to_string();
            let bare = s[i + sep.len()..].trim_start().to_string();
            return (prefix, bare);
        }
    }
    (String::new(), s.to_string())
}

/// Merge two key labels with common modifier prefix.
/// e.g. "⌥ + h" + "⌥ + ←" → "⌥ + h/←"
/// Skips if the new key is already present.
fn merge_key_labels(existing: &str, new: &str) -> String {
    // Skip if exact duplicate
    if existing == new {
        return existing.to_string();
    }

    let (prefix_b, bare_b) = split_modifier_and_bare(new);

    // Check if bare_b already appears as a /-separated segment
    if existing.split('/').any(|s| s.trim() == bare_b.trim()) {
        return existing.to_string();
    }

    let effective_prefix = if existing.contains('/') {
        split_modifier_and_bare(existing.split('/').next().unwrap_or("")).0
    } else {
        split_modifier_and_bare(existing).0
    };

    if !effective_prefix.is_empty() && effective_prefix == prefix_b {
        format!("{}/{}", existing, bare_b)
    } else {
        format!("{}  {}", existing, new)
    }
}


fn format_key(key: &KeyWithModifier, style: &ModifierStyle) -> String {
    let sep = " + ";
    let mut parts = Vec::new();
    for m in &key.key_modifiers {
        match m {
            KeyModifier::Ctrl => parts.push(style.ctrl().to_string()),
            KeyModifier::Alt => parts.push(style.alt().to_string()),
            KeyModifier::Shift => parts.push(style.shift().to_string()),
            KeyModifier::Super => parts.push(style.super_key().to_string()),
        }
    }
    let bare = match key.bare_key {
        BareKey::Char(' ') => "␣".to_string(),
        BareKey::Char(c) => {
            if key.key_modifiers.contains(&KeyModifier::Shift) && c.is_ascii_lowercase() {
                c.to_uppercase().to_string()
            } else {
                c.to_string()
            }
        }
        BareKey::F(n) => format!("F{}", n),
        BareKey::Enter => "↵".to_string(),
        BareKey::Tab => "⇥".to_string(),
        BareKey::Esc => "Esc".to_string(),
        BareKey::Backspace => "⌫".to_string(),
        BareKey::Delete => "Del".to_string(),
        BareKey::Insert => "Ins".to_string(),
        BareKey::Home => "Home".to_string(),
        BareKey::End => "End".to_string(),
        BareKey::PageUp => "PgUp".to_string(),
        BareKey::PageDown => "PgDn".to_string(),
        BareKey::Left => "←".to_string(),
        BareKey::Right => "→".to_string(),
        BareKey::Up => "↑".to_string(),
        BareKey::Down => "↓".to_string(),
        _ => format!("{:?}", key.bare_key),
    };
    parts.push(bare);
    parts.join(sep)
}


fn format_actions(actions: &[Action]) -> String {
    let mut seen = Vec::new();
    for a in actions {
        let s = format_action(a);
        if !s.is_empty() && !seen.contains(&s) {
            seen.push(s);
        }
    }
    seen.join(", ")
}

fn format_action(action: &Action) -> String {
    match action {
        Action::Quit => "Quit".into(),
        Action::Detach => "Detach".into(),
        Action::Confirm => "Confirm".into(),
        Action::Deny => "Deny".into(),
        Action::Copy => "Copy".into(),
        Action::NoOp => String::new(),

        Action::SwitchToMode(mode) => format!("{} mode", format_mode(*mode)),
        Action::SwitchModeForAllClients(mode) => format!("{} mode (all)", format_mode(*mode)),

        Action::FocusNextPane => "Focus next".into(),
        Action::FocusPreviousPane => "Focus prev".into(),
        Action::SwitchFocus => "Switch focus".into(),
        Action::MoveFocus(dir) => format!("Focus {}", format_dir(dir)),
        Action::MoveFocusOrTab(dir) => format!("Focus/tab {}", format_dir(dir)),
        Action::MovePane(Some(dir)) => format!("Move pane {}", format_dir(dir)),
        Action::MovePane(None) => "Move pane".into(),
        Action::MovePaneBackwards => "Move pane back".into(),

        Action::NewPane(dir, _, _) => match dir {
            Some(d) => format!("New pane {}", format_dir(d)),
            None => "New pane".into(),
        },
        Action::NewFloatingPane(..) => "New float pane".into(),
        Action::NewTiledPane(..) => "New tiled pane".into(),
        Action::NewInPlacePane(..) => "New in-place pane".into(),
        Action::CloseFocus => "Close pane".into(),
        Action::TogglePaneEmbedOrFloating => "Toggle embed/float".into(),
        Action::ToggleFloatingPanes => "Toggle float".into(),
        Action::ToggleFocusFullscreen => "Fullscreen".into(),
        Action::TogglePaneFrames => "Toggle frames".into(),
        Action::ToggleActiveSyncTab => "Sync input".into(),
        Action::PaneNameInput(_) => "Rename pane".into(),
        Action::UndoRenamePane => "Undo rename pane".into(),
        Action::BreakPane => "Break pane".into(),
        Action::BreakPaneRight => "Break pane →".into(),
        Action::BreakPaneLeft => "Break pane ←".into(),

        Action::NewTab(..) => "New tab".into(),
        Action::GoToNextTab => "Next tab".into(),
        Action::GoToPreviousTab => "Prev tab".into(),
        Action::CloseTab => "Close tab".into(),
        Action::GoToTab(n) => format!("Tab {}", n),
        Action::GoToTabName(name, _) => format!("Tab '{}'", name),
        Action::ToggleTab => "Toggle tab".into(),
        Action::TabNameInput(_) => "Rename tab".into(),
        Action::UndoRenameTab => "Undo rename tab".into(),
        Action::MoveTab(dir) => format!("Move tab {}", format_dir(dir)),

        Action::Resize(resize, dir) => {
            let r = match resize {
                Resize::Increase => "Grow",
                Resize::Decrease => "Shrink",
            };
            match dir {
                Some(d) => format!("{} {}", r, format_dir(d)),
                None => r.to_string(),
            }
        }

        Action::ScrollUp => "Scroll ↑".into(),
        Action::ScrollDown => "Scroll ↓".into(),
        Action::ScrollToTop => "Scroll top".into(),
        Action::ScrollToBottom => "Scroll bottom".into(),
        Action::PageScrollUp => "Page ↑".into(),
        Action::PageScrollDown => "Page ↓".into(),
        Action::HalfPageScrollUp => "Half page ↑".into(),
        Action::HalfPageScrollDown => "Half page ↓".into(),
        Action::EditScrollback => "Edit scrollback".into(),
        Action::ClearScreen => "Clear screen".into(),

        Action::Search(SearchDirection::Down) => "Search next".into(),
        Action::Search(SearchDirection::Up) => "Search prev".into(),
        Action::SearchInput(_) => "Search input".into(),
        Action::SearchToggleOption(opt) => match opt {
            SearchOption::CaseSensitivity => "Toggle case".into(),
            SearchOption::WholeWord => "Toggle word".into(),
            SearchOption::Wrap => "Toggle wrap".into(),
        },

        Action::Write(..) => "Write".into(),
        Action::WriteChars(s) => format!("Write '{}'", truncate(s, 10)),

        Action::PreviousSwapLayout => "Prev layout".into(),
        Action::NextSwapLayout => "Next layout".into(),
        Action::DumpLayout => "Dump layout".into(),
        Action::DumpScreen(_, _) => "Dump screen".into(),

        Action::RenameSession(_) => "Rename session".into(),
        Action::ToggleMouseMode => "Toggle mouse".into(),

        Action::LaunchOrFocusPlugin(..) => "Plugin".into(),
        Action::LaunchPlugin(..) => "Plugin".into(),
        Action::StartOrReloadPlugin(_) => "Reload plugin".into(),

        Action::Run(_) => "Run command".into(),

        _ => {
            let s = format!("{:?}", action);
            truncate(&s, 25)
        }
    }
}


fn format_mode(mode: InputMode) -> String {
    match mode {
        InputMode::Normal => "Normal".into(),
        InputMode::Locked => "Locked".into(),
        InputMode::Resize => "Resize".into(),
        InputMode::Pane => "Pane".into(),
        InputMode::Tab => "Tab".into(),
        InputMode::Scroll => "Scroll".into(),
        InputMode::EnterSearch => "Search".into(),
        InputMode::Search => "Search".into(),
        InputMode::RenameTab => "Rename Tab".into(),
        InputMode::RenamePane => "Rename Pane".into(),
        InputMode::Session => "Session".into(),
        InputMode::Move => "Move".into(),
        InputMode::Prompt => "Prompt".into(),
        InputMode::Tmux => "Tmux".into(),
    }
}

fn mode_icon(mode: InputMode) -> &'static str {
    match mode {
        InputMode::Normal => "○",
        InputMode::Locked => "⊘",
        InputMode::Pane => "□",
        InputMode::Tab => "◫",
        InputMode::Resize => "↔",
        InputMode::Move => "✥",
        InputMode::Scroll => "↕",
        InputMode::EnterSearch | InputMode::Search => "⌕",
        InputMode::RenameTab | InputMode::RenamePane => "✎",
        InputMode::Session => "◎",
        InputMode::Prompt => "❯",
        InputMode::Tmux => "⊞",
    }
}


fn format_dir(dir: &Direction) -> &'static str {
    match dir {
        Direction::Left => "←",
        Direction::Right => "→",
        Direction::Up => "↑",
        Direction::Down => "↓",
    }
}

/// Color bare keys blue, "/" separator in dim fg
fn color_bare_keys(bare: &str, palette: &Palette, bold: bool) -> String {
    let bold_str = if bold { BOLD } else { "" };
    let blue = fg_color(&palette.blue);
    let fg = fg_color(&palette.fg);
    bare.split('/')
        .map(|k| format!("{}{}{}{}", bold_str, blue, k, RESET))
        .collect::<Vec<_>>()
        .join(&format!("{}/{}", fg, RESET))
}

/// Color key label: modifier parts in yellow, bare key in blue, "/" in dim
fn color_key_label(key: &str, palette: &Palette, bold: bool) -> String {
    let yellow = fg_color(&palette.yellow);

    let parts: Vec<&str> = key.split(" + ").collect();
    if parts.len() <= 1 {
        return color_bare_keys(key, palette, bold);
    }
    let modifiers = &parts[..parts.len() - 1];
    let bare = parts[parts.len() - 1];

    let colored_mods: Vec<String> = modifiers.iter()
        .map(|m| format!("{}{}{}", yellow, m, RESET))
        .collect();
    format!(
        "{} + {}",
        colored_mods.join(&format!("{} + ", RESET)),
        color_bare_keys(bare, palette, bold),
    )
}

fn modifier_legend(style: &ModifierStyle, palette: &Palette) -> (String, usize) {
    let y = fg_color(&palette.yellow);
    match style {
        ModifierStyle::MacOS => (
            format!("  {}{}{} Ctrl  {}{}{} Alt  {}{}{} Shift  {}{}{} Super",
                y, style.ctrl(), RESET, y, style.alt(), RESET,
                y, style.shift(), RESET, y, style.super_key(), RESET),
            33,
        ),
        ModifierStyle::Linux => (
            format!("  {}C{}trl  {}A{}lt  {}S{}hift  {}Su{}per",
                y, RESET, y, RESET, y, RESET, y, RESET),
            22,
        ),
        ModifierStyle::Text => (String::new(), 0),
    }
}

fn fg_color(color: &PaletteColor) -> String {
    match color {
        PaletteColor::Rgb((r, g, b)) => format!("\x1b[38;2;{};{};{}m", r, g, b),
        PaletteColor::EightBit(n) => format!("\x1b[38;5;{}m", n),
    }
}


fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", t)
    }
}

