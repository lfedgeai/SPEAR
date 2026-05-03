use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use anyhow::Context;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;

use crate::config;
use crate::config::Config;
use crate::deploy;

#[path = "tui_menu.rs"]
mod tui_menu;
#[path = "tui_ui.rs"]
mod tui_ui;

#[derive(Clone, Copy, PartialEq, Eq)]
enum UiMode {
    Menu,
    EditText,
    ViewText,
    Confirm,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfirmChoice {
    Ok,
    Cancel,
}

#[derive(Clone)]
struct MenuItem {
    label: String,
    value: String,
    kind: ItemKind,
}

#[derive(Clone)]
enum ItemKind {
    Submenu(MenuScreen),
    ToggleBool(AccessorBool),
    ToggleScope(ScopePart),
    EditString(AccessorString),
    EditAppString(AccessorAppString),
    Action(Action),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopePart {
    Release,
    Secret,
    Namespace,
    Kind,
    Images,
}

impl ScopePart {
    fn key(self) -> &'static str {
        match self {
            ScopePart::Release => "release",
            ScopePart::Secret => "secret",
            ScopePart::Namespace => "namespace",
            ScopePart::Kind => "kind",
            ScopePart::Images => "images",
        }
    }
}

#[derive(Clone)]
enum Action {
    Back,
    StartPortForwardWebAdmin,
    StopPortForwardWebAdmin,
    StartPortForwardConsole,
    StopPortForwardConsole,
}

#[derive(Clone)]
struct AccessorBool {
    get: Arc<dyn Fn(&Config) -> bool + Send + Sync>,
    set: Arc<dyn Fn(&mut Config, bool) + Send + Sync>,
}

#[derive(Clone)]
struct AccessorString {
    get: Arc<dyn Fn(&Config) -> String + Send + Sync>,
    set: Arc<dyn Fn(&mut Config, String) + Send + Sync>,
}

#[derive(Clone)]
struct AccessorAppString {
    get: Arc<dyn Fn(&App) -> String + Send + Sync>,
    set: Arc<dyn Fn(&mut App, String) + Send + Sync>,
}

#[derive(Clone)]
struct MenuScreen {
    title: String,
    items: Vec<MenuItem>,
    selected: usize,
    visible_when: Option<fn(&Config) -> bool>,
}

struct App {
    cfg: Config,
    config_path: PathBuf,
    stack: Vec<MenuScreen>,
    mode: UiMode,
    dirty: bool,
    should_exit: bool,
    port_forward_web_admin: Option<PortForwardState>,
    port_forward_console: Option<PortForwardState>,
    input: String,
    input_title: String,
    input_apply: Option<Arc<dyn Fn(&mut App, String) + Send + Sync>>,
    cleanup_scope: String,
    view_title: String,
    view_text: String,
    view_scroll: u16,
    confirm_title: String,
    confirm_text: String,
    confirm_action: Option<PendingAction>,
    confirm_choice: ConfirmChoice,
    pending: Option<PendingAction>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingAction {
    Save,
    ShowPlan,
    Apply,
    Status,
    Cleanup,
    StartPortForwardWebAdmin,
    StopPortForwardWebAdmin,
    StartPortForwardConsole,
    StopPortForwardConsole,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PortForwardTarget {
    WebAdmin,
    Console,
}

struct PortForwardState {
    child: Child,
    local_port: u16,
    remote_port: u16,
    url: String,
    log_path: PathBuf,
}

pub fn edit_config_tui(cfg: &mut Config, config_path: &Path) -> anyhow::Result<()> {
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("enter alt screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    let mut app = App::new(cfg.clone(), config_path.to_path_buf());
    let res = run(&mut terminal, &mut app);

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    let app = res?;
    *cfg = app.cfg;
    Ok(())
}

impl App {
    fn new(cfg: Config, config_path: PathBuf) -> Self {
        let root = tui_menu::build_root_menu(&cfg);
        let mut app = Self {
            cfg,
            config_path,
            stack: vec![root],
            mode: UiMode::Menu,
            dirty: false,
            should_exit: false,
            port_forward_web_admin: None,
            port_forward_console: None,
            input: String::new(),
            input_title: String::new(),
            input_apply: None,
            cleanup_scope: "release".to_string(),
            view_title: String::new(),
            view_text: String::new(),
            view_scroll: 0,
            confirm_title: String::new(),
            confirm_text: String::new(),
            confirm_action: None,
            confirm_choice: ConfirmChoice::Ok,
            pending: None,
        };
        tui_menu::refresh_current_values(&mut app);
        app
    }

    fn current_mut(&mut self) -> &mut MenuScreen {
        self.stack.last_mut().unwrap()
    }

    fn current(&self) -> &MenuScreen {
        self.stack.last().unwrap()
    }

    fn push(&mut self, screen: MenuScreen) {
        self.stack.push(screen);
    }

    fn pop(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<App> {
    loop {
        refresh_port_forward_status(app);
        terminal.draw(|f| tui_ui::ui(f, app)).context("draw")?;

        let ev = event::read().context("read event")?;
        match ev {
            Event::Key(k) if k.kind == KeyEventKind::Press => {
                if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
                match app.mode {
                    UiMode::Menu => handle_menu_key(app, k.code)?,
                    UiMode::EditText => handle_input_key(app, k.code)?,
                    UiMode::ViewText => handle_view_key(app, k.code)?,
                    UiMode::Confirm => handle_confirm_key(app, k.code)?,
                }
            }
            _ => {}
        }

        if let Some(p) = app.pending.take() {
            execute_pending(terminal, app, p)?;
        }
        if app.should_exit {
            break;
        }
    }

    stop_port_forward(app, PortForwardTarget::WebAdmin);
    stop_port_forward(app, PortForwardTarget::Console);
    Ok(std::mem::replace(
        app,
        App::new(app.cfg.clone(), app.config_path.clone()),
    ))
}

fn screen_is_available(cfg: &Config, screen: &MenuScreen) -> bool {
    screen.visible_when.map(|f| f(cfg)).unwrap_or(true)
}

fn sanitize_cleanup_scope_for_mode(app: &mut App) {
    let allowed: &[&str] = match app.cfg.mode.name.as_str() {
        "k8s-kind" => &["release", "secret", "namespace", "kind"],
        "k8s-existing" => &["release", "secret", "namespace"],
        "docker-local" => &["release", "kind", "images"],
        _ => &["release"],
    };

    let mut parts: Vec<String> = app
        .cleanup_scope
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter(|s| allowed.iter().any(|a| a == s))
        .map(|s| s.to_string())
        .collect();

    let order = ["release", "secret", "namespace", "kind", "images"];
    parts.sort_by_key(|p| order.iter().position(|x| *x == p.as_str()).unwrap_or(99));
    parts.dedup();

    if parts.is_empty() {
        app.cleanup_scope = "release".to_string();
    } else {
        app.cleanup_scope = parts.join(",");
    }
}

fn rebuild_root_menu(app: &mut App) {
    if app.stack.is_empty() {
        return;
    }

    sanitize_cleanup_scope_for_mode(app);

    let old_selected = app
        .stack
        .first()
        .and_then(|s| s.items.get(s.selected))
        .map(|it| it.label.clone());

    let mut root = tui_menu::build_root_menu(&app.cfg);
    if let Some(label) = old_selected {
        if let Some(i) = root.items.iter().position(|it| it.label == label) {
            root.selected = i;
        }
    }
    app.stack.clear();
    app.stack.push(root);
}

fn handle_menu_key(app: &mut App, code: KeyCode) -> anyhow::Result<()> {
    let len = app.current().items.len();
    if len == 0 {
        return Ok(());
    }

    match code {
        KeyCode::F(1) => {
            app.view_title = "Help / 帮助".to_string();
            app.view_text = help_text();
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
        }
        KeyCode::F(2) => {
            app.pending = Some(PendingAction::Save);
        }
        KeyCode::F(3) => {
            app.pending = Some(PendingAction::ShowPlan);
        }
        KeyCode::F(4) => {
            app.confirm_title = "Apply / 部署".to_string();
            app.confirm_text =
                "Run apply now? This will change your environment.\n现在执行 apply？这会修改你的环境。"
                    .to_string();
            app.confirm_action = Some(PendingAction::Apply);
            app.confirm_choice = ConfirmChoice::Ok;
            app.mode = UiMode::Confirm;
        }
        KeyCode::F(5) => {
            app.pending = Some(PendingAction::Status);
        }
        KeyCode::F(6) => {
            let scope = app.cleanup_scope.trim();
            let dangerous = scope.split(',').any(|s| {
                let s = s.trim();
                s == "namespace" || s == "kind" || s == "images"
            });
            app.confirm_title = "Cleanup / 清理".to_string();
            if dangerous {
                app.confirm_text = format!(
                    "Cleanup scope: {scope}\n\nThis may delete namespace/kind cluster.\nContinue?\n\n清理范围：{scope}\n\n可能会删除命名空间/Kind集群，确认继续？"
                );
            } else {
                app.confirm_text = format!(
                    "Cleanup scope: {scope}\n\nContinue?\n\n清理范围：{scope}\n\n确认继续？"
                );
            }
            app.confirm_action = Some(PendingAction::Cleanup);
            app.confirm_choice = ConfirmChoice::Ok;
            app.mode = UiMode::Confirm;
        }
        KeyCode::F(10) => {
            app.should_exit = true;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let s = app.current_mut().selected;
            app.current_mut().selected = s.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let s = app.current_mut().selected;
            app.current_mut().selected = (s + 1).min(len - 1);
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.pop();
        }
        KeyCode::Enter => {
            let idx = app.current().selected;
            let item = app.current().items[idx].clone();
            match item.kind {
                ItemKind::Submenu(mut s) => {
                    s.selected = 0;
                    if !screen_is_available(&app.cfg, &s) {
                        app.view_title = "Not available / 不可用".to_string();
                        app.view_text =
                            "This menu is not available under the current mode.\n该菜单在当前模式下不可用。\n"
                                .to_string();
                        app.view_scroll = 0;
                        app.mode = UiMode::ViewText;
                        return Ok(());
                    }
                    app.push(s);
                    tui_menu::refresh_current_values(app);
                }
                ItemKind::ToggleBool(acc) => {
                    let before_mode = app.cfg.mode.name.clone();
                    let before = (acc.get)(&app.cfg);
                    (acc.set)(&mut app.cfg, !before);
                    let after = (acc.get)(&app.cfg);
                    if after != before {
                        app.dirty = true;
                    }
                    if before_mode != app.cfg.mode.name {
                        rebuild_root_menu(app);
                    }
                    tui_menu::refresh_current_values(app);
                }
                ItemKind::ToggleScope(part) => {
                    let before = tui_menu::scope_contains(&app.cleanup_scope, part.key());
                    tui_menu::scope_set(&mut app.cleanup_scope, part.key(), !before);
                    tui_menu::refresh_current_values(app);
                }
                ItemKind::EditString(acc) => {
                    app.mode = UiMode::EditText;
                    app.input_title = item.label;
                    app.input = (acc.get)(&app.cfg);
                    app.input_apply = Some(Arc::new(move |a: &mut App, v: String| {
                        (acc.set)(&mut a.cfg, v);
                        a.dirty = true;
                    }));
                }
                ItemKind::EditAppString(acc) => {
                    app.mode = UiMode::EditText;
                    app.input_title = item.label;
                    app.input = (acc.get)(app);
                    app.input_apply = Some(Arc::new(move |a: &mut App, v: String| {
                        (acc.set)(a, v);
                    }));
                }
                ItemKind::Action(Action::Back) => {
                    app.pop();
                }
                ItemKind::Action(Action::StartPortForwardWebAdmin) => {
                    app.pending = Some(PendingAction::StartPortForwardWebAdmin);
                }
                ItemKind::Action(Action::StopPortForwardWebAdmin) => {
                    app.confirm_title = "Stop port-forward / 停止端口转发".to_string();
                    app.confirm_text =
                        "Stop the running port-forward now?\n现在停止端口转发？".to_string();
                    app.confirm_action = Some(PendingAction::StopPortForwardWebAdmin);
                    app.confirm_choice = ConfirmChoice::Ok;
                    app.mode = UiMode::Confirm;
                }
                ItemKind::Action(Action::StartPortForwardConsole) => {
                    app.pending = Some(PendingAction::StartPortForwardConsole);
                }
                ItemKind::Action(Action::StopPortForwardConsole) => {
                    app.confirm_title = "Stop port-forward / 停止端口转发".to_string();
                    app.confirm_text =
                        "Stop the running port-forward now?\n现在停止端口转发？".to_string();
                    app.confirm_action = Some(PendingAction::StopPortForwardConsole);
                    app.confirm_choice = ConfirmChoice::Ok;
                    app.mode = UiMode::Confirm;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn handle_input_key(app: &mut App, code: KeyCode) -> anyhow::Result<()> {
    match code {
        KeyCode::Esc => {
            app.mode = UiMode::Menu;
            app.input.clear();
            app.input_title.clear();
            app.input_apply = None;
        }
        KeyCode::Enter => {
            if let Some(apply) = app.input_apply.take() {
                apply(app, app.input.trim().to_string());
            }
            app.mode = UiMode::Menu;
            app.input.clear();
            app.input_title.clear();
            tui_menu::refresh_current_values(app);
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Char(ch) => {
            app.input.push(ch);
        }
        _ => {}
    }
    Ok(())
}

fn handle_view_key(app: &mut App, code: KeyCode) -> anyhow::Result<()> {
    match code {
        KeyCode::F(10) => {
            app.should_exit = true;
        }
        KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
            app.mode = UiMode::Menu;
            app.view_title.clear();
            app.view_text.clear();
            app.view_scroll = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.view_scroll = app.view_scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.view_scroll = app.view_scroll.saturating_add(1);
        }
        KeyCode::PageUp => {
            app.view_scroll = app.view_scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
            app.view_scroll = app.view_scroll.saturating_add(10);
        }
        _ => {}
    }
    Ok(())
}

fn handle_confirm_key(app: &mut App, code: KeyCode) -> anyhow::Result<()> {
    match code {
        KeyCode::F(10) => {
            app.should_exit = true;
        }
        KeyCode::Esc | KeyCode::Char('n') => {
            app.mode = UiMode::Menu;
            app.confirm_title.clear();
            app.confirm_text.clear();
            app.confirm_action = None;
            app.confirm_choice = ConfirmChoice::Ok;
        }
        KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
            app.confirm_choice = match app.confirm_choice {
                ConfirmChoice::Ok => ConfirmChoice::Cancel,
                ConfirmChoice::Cancel => ConfirmChoice::Ok,
            };
        }
        KeyCode::Char('y') => {
            app.confirm_choice = ConfirmChoice::Ok;
        }
        KeyCode::Char('c') => {
            app.confirm_choice = ConfirmChoice::Cancel;
        }
        KeyCode::Enter => match app.confirm_choice {
            ConfirmChoice::Ok => {
                if let Some(p) = app.confirm_action.take() {
                    app.mode = UiMode::Menu;
                    app.confirm_title.clear();
                    app.confirm_text.clear();
                    app.confirm_choice = ConfirmChoice::Ok;
                    app.pending = Some(p);
                } else {
                    app.mode = UiMode::Menu;
                }
            }
            ConfirmChoice::Cancel => {
                app.mode = UiMode::Menu;
                app.confirm_title.clear();
                app.confirm_text.clear();
                app.confirm_action = None;
                app.confirm_choice = ConfirmChoice::Ok;
            }
        },
        _ => {}
    }
    Ok(())
}

fn execute_pending(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    action: PendingAction,
) -> anyhow::Result<()> {
    match action {
        PendingAction::Save => {
            config::save_config(&app.config_path, &app.cfg)?;
            app.dirty = false;
            app.view_title = "Saved / 已保存".to_string();
            app.view_text = format!("Saved to:\n{}\n", app.config_path.display());
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
        PendingAction::ShowPlan => {
            app.view_title = "Plan / 计划".to_string();
            app.view_text = deploy::render_plan(&app.cfg)?;
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
        PendingAction::Apply => {
            let _ = config::save_config(&app.config_path, &app.cfg);
            suspend_tui(terminal).ok();
            let res = deploy::apply(&app.cfg, true);
            let status_text = if res.is_ok() {
                deploy::status_text(&app.cfg)
            } else {
                Ok(String::new())
            };
            resume_tui(terminal).ok();
            match res {
                Ok(()) => {
                    app.view_title = "Apply done / 部署完成".to_string();
                    app.view_text = "Apply finished.\n".to_string();
                    match status_text {
                        Ok(s) => {
                            if !s.trim().is_empty() {
                                app.view_text.push_str("\nStatus:\n");
                                app.view_text.push_str(&s);
                                if !app.view_text.ends_with('\n') {
                                    app.view_text.push('\n');
                                }
                            }
                        }
                        Err(e) => {
                            app.view_text
                                .push_str(&format!("\nStatus failed:\n{e:?}\n"));
                        }
                    }
                    if app.cfg.mode.name == "k8s-kind" {
                        let mut any_started = false;
                        if app.cfg.k8s.port_forward.enabled && app.cfg.k8s.port_forward.auto_start {
                            match start_port_forward(app, PortForwardTarget::WebAdmin) {
                                Ok(()) => any_started = true,
                                Err(e) => app.view_text.push_str(&format!(
                                    "\nPort-forward(web admin) failed:\n{e:?}\n"
                                )),
                            }
                        }
                        if app.cfg.k8s.port_forward_console.enabled
                            && app.cfg.k8s.port_forward_console.auto_start
                        {
                            match start_port_forward(app, PortForwardTarget::Console) {
                                Ok(()) => any_started = true,
                                Err(e) => app.view_text.push_str(&format!(
                                    "\nPort-forward(console) failed:\n{e:?}\n"
                                )),
                            }
                        }
                        if any_started {
                            app.view_text.push_str(&format!(
                                "\nPort-forward: {}\n",
                                port_forward_summary(app)
                            ));
                        }
                    }
                }
                Err(e) => {
                    app.view_title = "Apply failed / 部署失败".to_string();
                    app.view_text = format!("{e:?}\n");
                }
            }
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
        PendingAction::Status => {
            suspend_tui(terminal).ok();
            let res = deploy::status_text(&app.cfg);
            resume_tui(terminal).ok();
            match res {
                Ok(s) => {
                    app.view_title = "Status done / 状态完成".to_string();
                    app.view_text = s;
                    if app.view_text.trim().is_empty() {
                        app.view_text = "No status output.\n".to_string();
                    }
                }
                Err(e) => {
                    app.view_title = "Status failed / 状态失败".to_string();
                    app.view_text = format!("{e:?}\n");
                }
            }
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
        PendingAction::Cleanup => {
            let _ = config::save_config(&app.config_path, &app.cfg);
            stop_port_forward(app, PortForwardTarget::WebAdmin);
            stop_port_forward(app, PortForwardTarget::Console);
            let scope = app.cleanup_scope.trim().to_string();
            suspend_tui(terminal).ok();
            let res = deploy::cleanup(&app.cfg, &scope, true);
            resume_tui(terminal).ok();
            match res {
                Ok(()) => {
                    app.view_title = "Cleanup done / 清理完成".to_string();
                    app.view_text = "Cleanup finished.\n".to_string();
                }
                Err(e) => {
                    app.view_title = "Cleanup failed / 清理失败".to_string();
                    app.view_text = format!("{e:?}\n");
                }
            }
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
        PendingAction::StartPortForwardWebAdmin => {
            match start_port_forward(app, PortForwardTarget::WebAdmin) {
                Ok(()) => {
                    app.view_title = "Port-forward started / 端口转发已启动".to_string();
                    if let Some(pf) = &app.port_forward_web_admin {
                        app.view_text = format!(
                            "{}\nlog: {}\n",
                            port_forward_summary(app),
                            pf.log_path.display()
                        );
                    } else {
                        app.view_text = format!("{}\n", port_forward_summary(app));
                    }
                }
                Err(e) => {
                    app.view_title = "Port-forward failed / 端口转发失败".to_string();
                    app.view_text = format!("{e:?}\n");
                }
            }
            tui_menu::refresh_current_values(app);
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
        PendingAction::StopPortForwardWebAdmin => {
            stop_port_forward(app, PortForwardTarget::WebAdmin);
            tui_menu::refresh_current_values(app);
            app.view_title = "Port-forward stopped / 端口转发已停止".to_string();
            app.view_text = "Stopped.\n".to_string();
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
        PendingAction::StartPortForwardConsole => {
            match start_port_forward(app, PortForwardTarget::Console) {
                Ok(()) => {
                    app.view_title = "Port-forward started / 端口转发已启动".to_string();
                    if let Some(pf) = &app.port_forward_console {
                        app.view_text = format!(
                            "{}\nlog: {}\n",
                            port_forward_summary(app),
                            pf.log_path.display()
                        );
                    } else {
                        app.view_text = format!("{}\n", port_forward_summary(app));
                    }
                }
                Err(e) => {
                    app.view_title = "Port-forward failed / 端口转发失败".to_string();
                    app.view_text = format!("{e:?}\n");
                }
            }
            tui_menu::refresh_current_values(app);
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
        PendingAction::StopPortForwardConsole => {
            stop_port_forward(app, PortForwardTarget::Console);
            tui_menu::refresh_current_values(app);
            app.view_title = "Port-forward stopped / 端口转发已停止".to_string();
            app.view_text = "Stopped.\n".to_string();
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
    }
}

fn suspend_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode().context("disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).context("leave alt screen")?;
    terminal.show_cursor().context("show cursor")?;
    Ok(())
}

fn resume_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    enable_raw_mode().context("enable raw mode")?;
    execute!(terminal.backend_mut(), EnterAlternateScreen).context("enter alt screen")?;
    terminal.clear().ok();
    Ok(())
}

fn help_text() -> String {
    let mut s = String::new();
    s.push_str("SPEAR Quickstart TUI\n\n");
    s.push_str("Navigation:\n");
    s.push_str("  - Up/Down or j/k: move\n");
    s.push_str("  - Enter: select/edit\n");
    s.push_str("  - Esc/q: back/close\n\n");
    s.push_str("Function keys:\n");
    s.push_str("  - F1: Help\n");
    s.push_str("  - F2: Save config\n");
    s.push_str("  - F3: Plan\n");
    s.push_str("  - F4: Apply\n");
    s.push_str("  - F5: Status\n");
    s.push_str("  - F6: Cleanup\n");
    s.push_str("  - F10: Exit\n\n");
    s.push_str("Notes:\n");
    s.push_str("  - Apply/Cleanup will temporarily suspend the TUI and run commands.\n");
    s.push_str("  - Cleanup scope is configurable in the main menu.\n");
    s.push_str("  - Port-forward is configurable in the main menu.\n");
    s
}

fn port_forward_title(app: &App) -> String {
    let console = port_forward_short(app, PortForwardTarget::Console);
    let web = port_forward_short(app, PortForwardTarget::WebAdmin);
    format!("c={console} w={web}")
}

fn port_forward_status_style(app: &App) -> Style {
    if !app.cfg.k8s.port_forward.enabled && !app.cfg.k8s.port_forward_console.enabled {
        return Style::default().fg(Color::DarkGray);
    }
    if app.port_forward_web_admin.is_some() || app.port_forward_console.is_some() {
        Style::default().fg(Color::LightGreen)
    } else {
        Style::default().fg(Color::LightRed)
    }
}

fn port_forward_summary(app: &App) -> String {
    format!(
        "console: {} | web_admin: {}",
        port_forward_long(app, PortForwardTarget::Console),
        port_forward_long(app, PortForwardTarget::WebAdmin),
    )
}

fn port_forward_summary_web_admin(app: &App) -> String {
    port_forward_long(app, PortForwardTarget::WebAdmin)
}

fn port_forward_summary_console(app: &App) -> String {
    port_forward_long(app, PortForwardTarget::Console)
}

fn port_forward_short(app: &App, target: PortForwardTarget) -> String {
    let (cfg, state) = match target {
        PortForwardTarget::WebAdmin => (&app.cfg.k8s.port_forward, &app.port_forward_web_admin),
        PortForwardTarget::Console => (
            &app.cfg.k8s.port_forward_console,
            &app.port_forward_console,
        ),
    };
    if !cfg.enabled {
        return "Disabled".to_string();
    }
    match state {
        Some(pf) => format!("On:{}", pf.local_port),
        None => {
            if cfg.auto_start {
                format!("Off(auto):{}", cfg.local_port)
            } else {
                format!("Off:{}", cfg.local_port)
            }
        }
    }
}

fn port_forward_long(app: &App, target: PortForwardTarget) -> String {
    let (cfg, state) = match target {
        PortForwardTarget::WebAdmin => (&app.cfg.k8s.port_forward, &app.port_forward_web_admin),
        PortForwardTarget::Console => (
            &app.cfg.k8s.port_forward_console,
            &app.port_forward_console,
        ),
    };

    if !cfg.enabled {
        return "Disabled".to_string();
    }
    match state {
        Some(pf) => format!("On {} ({}->{})", pf.url, pf.local_port, pf.remote_port),
        None => {
            if cfg.auto_start {
                format!("Off (auto) :{}", cfg.local_port)
            } else {
                format!("Off :{}", cfg.local_port)
            }
        }
    }
}

fn refresh_port_forward_status(app: &mut App) {
    let mut changed = false;
    if let Some(pf) = &mut app.port_forward_web_admin {
        if let Ok(Some(_)) = pf.child.try_wait() {
            app.port_forward_web_admin = None;
            changed = true;
        }
    }
    if let Some(pf) = &mut app.port_forward_console {
        if let Ok(Some(_)) = pf.child.try_wait() {
            app.port_forward_console = None;
            changed = true;
        }
    }
    if changed {
        tui_menu::refresh_current_values(app);
    }
}

fn stop_port_forward(app: &mut App, target: PortForwardTarget) {
    let state = match target {
        PortForwardTarget::WebAdmin => &mut app.port_forward_web_admin,
        PortForwardTarget::Console => &mut app.port_forward_console,
    };
    if let Some(mut pf) = state.take() {
        let _ = pf.child.kill();
        let _ = pf.child.wait();
    }
}

fn start_port_forward(app: &mut App, target: PortForwardTarget) -> anyhow::Result<()> {
    stop_port_forward(app, target);
    if app.cfg.mode.name != "k8s-kind" {
        return Err(anyhow::anyhow!(
            "port-forward currently supports mode=k8s-kind only"
        ));
    }

    let (cfg, log_name, url_path) = match target {
        PortForwardTarget::WebAdmin => {
            (&app.cfg.k8s.port_forward, "port-forward-web-admin.log", "/")
        }
        PortForwardTarget::Console => (
            &app.cfg.k8s.port_forward_console,
            "port-forward-console.log",
            "/console",
        ),
    };
    if !cfg.enabled {
        return Err(anyhow::anyhow!("port-forward is disabled"));
    }

    let repo_root = config::repo_root()?;
    let kubeconfig = repo_root.join(&app.cfg.k8s.kind.kubeconfig_file);
    let kubeconfig_str = kubeconfig.to_string_lossy().to_string();
    let ns = app.cfg.k8s.namespace.clone();
    let release = app.cfg.k8s.release_name.clone();
    let local_port = cfg.local_port;
    let remote_port = cfg.remote_port;
    let svc = format!("svc/{}-spear-sms", release);

    let log_dir = repo_root.join(&app.cfg.paths.state_dir);
    std::fs::create_dir_all(&log_dir).ok();
    let log_path = log_dir.join(log_name);
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .with_context(|| format!("open {}", log_path.display()))?;
    let log_err = log.try_clone().context("clone log file")?;

    let mut cmd = Command::new("kubectl");
    cmd.current_dir(&repo_root);
    cmd.env("KUBECONFIG", kubeconfig_str);
    cmd.args(["-n", &ns, "port-forward", "--address", "127.0.0.1"]);
    cmd.arg(&svc);
    cmd.arg(format!("{local_port}:{remote_port}"));
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::from(log));
    cmd.stderr(Stdio::from(log_err));
    let child = cmd.spawn().context("spawn kubectl port-forward")?;

    let url = format!("http://127.0.0.1:{}{}", local_port, url_path);
    let state = PortForwardState {
        child,
        local_port,
        remote_port,
        url,
        log_path,
    };
    match target {
        PortForwardTarget::WebAdmin => app.port_forward_web_admin = Some(state),
        PortForwardTarget::Console => app.port_forward_console = Some(state),
    }
    tui_menu::refresh_current_values(app);
    Ok(())
}
