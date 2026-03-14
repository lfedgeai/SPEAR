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
use ratatui::text::{Line, Span};
use ratatui::widgets::*;
use unicode_width::UnicodeWidthChar;

use crate::config;
use crate::config::Config;
use crate::deploy;

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
}

impl ScopePart {
    fn key(self) -> &'static str {
        match self {
            ScopePart::Release => "release",
            ScopePart::Secret => "secret",
            ScopePart::Namespace => "namespace",
            ScopePart::Kind => "kind",
        }
    }
}

#[derive(Clone)]
enum Action {
    Back,
    StartPortForward,
    StopPortForward,
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
}

struct App {
    cfg: Config,
    config_path: PathBuf,
    stack: Vec<MenuScreen>,
    mode: UiMode,
    dirty: bool,
    should_exit: bool,
    port_forward: Option<PortForwardState>,
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
    StartPortForward,
    StopPortForward,
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
        let root = build_root_menu();
        let mut app = Self {
            cfg,
            config_path,
            stack: vec![root],
            mode: UiMode::Menu,
            dirty: false,
            should_exit: false,
            port_forward: None,
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
        refresh_current_values(&mut app);
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
        terminal.draw(|f| ui(f, app)).context("draw")?;

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

    stop_port_forward(app);
    Ok(std::mem::replace(
        app,
        App::new(app.cfg.clone(), app.config_path.clone()),
    ))
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
                s == "namespace" || s == "kind"
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
                    app.push(s);
                    refresh_current_values(app);
                }
                ItemKind::ToggleBool(acc) => {
                    let before = (acc.get)(&app.cfg);
                    (acc.set)(&mut app.cfg, !before);
                    let after = (acc.get)(&app.cfg);
                    if after != before {
                        app.dirty = true;
                    }
                    refresh_current_values(app);
                }
                ItemKind::ToggleScope(part) => {
                    let before = scope_contains(&app.cleanup_scope, part.key());
                    scope_set(&mut app.cleanup_scope, part.key(), !before);
                    refresh_current_values(app);
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
                ItemKind::Action(Action::StartPortForward) => {
                    app.pending = Some(PendingAction::StartPortForward);
                }
                ItemKind::Action(Action::StopPortForward) => {
                    app.confirm_title = "Stop port-forward / 停止端口转发".to_string();
                    app.confirm_text =
                        "Stop the running port-forward now?\n现在停止端口转发？".to_string();
                    app.confirm_action = Some(PendingAction::StopPortForward);
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
            refresh_current_values(app);
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

fn refresh_current_values(app: &mut App) {
    for si in 0..app.stack.len() {
        let kinds: Vec<ItemKind> = app.stack[si]
            .items
            .iter()
            .map(|it| it.kind.clone())
            .collect();
        for (ii, kind) in kinds.into_iter().enumerate() {
            let v = render_value(app, &kind);
            app.stack[si].items[ii].value = v;
        }
    }
}

fn render_value(app: &App, kind: &ItemKind) -> String {
    match kind {
        ItemKind::ToggleBool(acc) => {
            if (acc.get)(&app.cfg) {
                "[*]".to_string()
            } else {
                "[ ]".to_string()
            }
        }
        ItemKind::ToggleScope(part) => {
            if scope_contains(&app.cleanup_scope, part.key()) {
                "[*]".to_string()
            } else {
                "[ ]".to_string()
            }
        }
        ItemKind::EditString(acc) => (acc.get)(&app.cfg),
        ItemKind::EditAppString(acc) => (acc.get)(app),
        ItemKind::Submenu(screen) => {
            if screen.title.starts_with("Cleanup scope") {
                app.cleanup_scope.to_string()
            } else if screen.title.starts_with("Mode") {
                app.cfg.mode.name.clone()
            } else if screen.title.starts_with("Port forward") {
                port_forward_summary(app)
            } else {
                "".to_string()
            }
        }
        ItemKind::Action(_) => "".to_string(),
    }
}

fn build_root_menu() -> MenuScreen {
    MenuScreen {
        title: "SPEAR Quickstart".to_string(),
        items: vec![
            MenuItem {
                label: "Mode / 模式".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_mode_menu()),
            },
            MenuItem {
                label: "Cleanup scope / 清理范围".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_cleanup_scope_menu()),
            },
            MenuItem {
                label: "Port forward / 端口转发".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_port_forward_menu()),
            },
            MenuItem {
                label: "K8s / Kubernetes".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_k8s_menu()),
            },
            MenuItem {
                label: "Build / 构建".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_build_menu()),
            },
            MenuItem {
                label: "Images / 镜像".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_images_menu()),
            },
            MenuItem {
                label: "Components / 组件".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_components_menu()),
            },
            MenuItem {
                label: "Logging / 日志".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_logging_menu()),
            },
            MenuItem {
                label: "Secrets / 密钥".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_secrets_menu()),
            },
        ],
        selected: 0,
    }
}

fn build_port_forward_menu() -> MenuScreen {
    MenuScreen {
        title: "Port forward / 端口转发".to_string(),
        items: vec![
            MenuItem {
                label: "Enabled / 启用".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.k8s.port_forward.enabled),
                    set: Arc::new(|c, v| c.k8s.port_forward.enabled = v),
                }),
            },
            MenuItem {
                label: "Auto start after apply / 部署后自动启动".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.k8s.port_forward.auto_start),
                    set: Arc::new(|c, v| c.k8s.port_forward.auto_start = v),
                }),
            },
            MenuItem {
                label: "Local port / 本地端口".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.k8s.port_forward.local_port.to_string()),
                    set: Arc::new(|c, v| {
                        if let Ok(p) = v.trim().parse::<u16>() {
                            if p != 0 {
                                c.k8s.port_forward.local_port = p;
                            }
                        }
                    }),
                }),
            },
            MenuItem {
                label: "Remote port / 远端端口".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.k8s.port_forward.remote_port.to_string()),
                    set: Arc::new(|c, v| {
                        if let Ok(p) = v.trim().parse::<u16>() {
                            if p != 0 {
                                c.k8s.port_forward.remote_port = p;
                            }
                        }
                    }),
                }),
            },
            MenuItem {
                label: "Start / 启动".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::StartPortForward),
            },
            MenuItem {
                label: "Stop / 停止".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::StopPortForward),
            },
            MenuItem {
                label: "Back / 返回".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::Back),
            },
        ],
        selected: 0,
    }
}

fn build_cleanup_scope_menu() -> MenuScreen {
    MenuScreen {
        title: "Cleanup scope / 清理范围".to_string(),
        items: vec![
            MenuItem {
                label: "release (helm uninstall)".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleScope(ScopePart::Release),
            },
            MenuItem {
                label: "secret (k8s secret)".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleScope(ScopePart::Secret),
            },
            MenuItem {
                label: "namespace (DANGEROUS)".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleScope(ScopePart::Namespace),
            },
            MenuItem {
                label: "kind (DANGEROUS)".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleScope(ScopePart::Kind),
            },
            MenuItem {
                label: "Raw csv / 原始csv".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditAppString(AccessorAppString {
                    get: Arc::new(|a| a.cleanup_scope.clone()),
                    set: Arc::new(|a, v| a.cleanup_scope = v),
                }),
            },
            MenuItem {
                label: "Back / 返回".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::Back),
            },
        ],
        selected: 0,
    }
}

fn build_mode_menu() -> MenuScreen {
    let mut items = vec![];

    let set_mode = |mode: &'static str| -> AccessorBool {
        let get = move |c: &Config| c.mode.name == mode;
        let set = move |c: &mut Config, v: bool| {
            if v {
                c.mode.name = mode.to_string();
            }
        };
        AccessorBool {
            get: Arc::new(get),
            set: Arc::new(set),
        }
    };

    items.push(MenuItem {
        label: "k8s-kind".to_string(),
        value: "".to_string(),
        kind: ItemKind::ToggleBool(set_mode("k8s-kind")),
    });
    items.push(MenuItem {
        label: "k8s-existing".to_string(),
        value: "".to_string(),
        kind: ItemKind::ToggleBool(set_mode("k8s-existing")),
    });
    items.push(MenuItem {
        label: "docker-local".to_string(),
        value: "".to_string(),
        kind: ItemKind::ToggleBool(set_mode("docker-local")),
    });

    items.push(MenuItem {
        label: "Back / 返回".to_string(),
        value: "".to_string(),
        kind: ItemKind::Action(Action::Back),
    });

    MenuScreen {
        title: "Mode / 模式".to_string(),
        items,
        selected: 0,
    }
}

fn build_k8s_menu() -> MenuScreen {
    MenuScreen {
        title: "K8s / Kubernetes".to_string(),
        items: vec![
            MenuItem {
                label: "Namespace / 命名空间".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.k8s.namespace.clone()),
                    set: Arc::new(|c, v| c.k8s.namespace = v),
                }),
            },
            MenuItem {
                label: "Release / 发布名".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.k8s.release_name.clone()),
                    set: Arc::new(|c, v| c.k8s.release_name = v),
                }),
            },
            MenuItem {
                label: "Kind cluster name / kind 集群名".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.k8s.kind.cluster_name.clone()),
                    set: Arc::new(|c, v| c.k8s.kind.cluster_name = v),
                }),
            },
            MenuItem {
                label: "Reuse existing kind cluster / 复用已有 kind 集群".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.k8s.kind.reuse_cluster),
                    set: Arc::new(|c, v| c.k8s.kind.reuse_cluster = v),
                }),
            },
            MenuItem {
                label: "Keep kind cluster after run / 运行结束保留 kind 集群".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.k8s.kind.keep_cluster),
                    set: Arc::new(|c, v| c.k8s.kind.keep_cluster = v),
                }),
            },
            MenuItem {
                label: "Kind kubeconfig file / kind kubeconfig 文件".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.k8s.kind.kubeconfig_file.clone()),
                    set: Arc::new(|c, v| c.k8s.kind.kubeconfig_file = v),
                }),
            },
            MenuItem {
                label: "Back / 返回".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::Back),
            },
        ],
        selected: 0,
    }
}

fn build_build_menu() -> MenuScreen {
    MenuScreen {
        title: "Build / 构建".to_string(),
        items: vec![
            MenuItem {
                label: "Enabled / 启用".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.build.enabled),
                    set: Arc::new(|c, v| c.build.enabled = v),
                }),
            },
            MenuItem {
                label: "Pull base / 拉取基础镜像".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.build.pull_base),
                    set: Arc::new(|c, v| c.build.pull_base = v),
                }),
            },
            MenuItem {
                label: "No cache / 禁用缓存".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.build.no_cache),
                    set: Arc::new(|c, v| c.build.no_cache = v),
                }),
            },
            MenuItem {
                label: "Debian suite / Debian版本".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.build.debian_suite.clone()),
                    set: Arc::new(|c, v| c.build.debian_suite = v),
                }),
            },
            MenuItem {
                label: "Back / 返回".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::Back),
            },
        ],
        selected: 0,
    }
}

fn build_images_menu() -> MenuScreen {
    MenuScreen {
        title: "Images / 镜像".to_string(),
        items: vec![
            MenuItem {
                label: "Image tag / 镜像tag".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.images.tag.clone()),
                    set: Arc::new(|c, v| c.images.tag = v),
                }),
            },
            MenuItem {
                label: "SMS repo".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.images.sms_repo.clone()),
                    set: Arc::new(|c, v| c.images.sms_repo = v),
                }),
            },
            MenuItem {
                label: "Spearlet repo".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.images.spearlet_repo.clone()),
                    set: Arc::new(|c, v| c.images.spearlet_repo = v),
                }),
            },
            MenuItem {
                label: "Router filter agent repo".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.images.router_filter_agent_repo.clone()),
                    set: Arc::new(|c, v| c.images.router_filter_agent_repo = v),
                }),
            },
            MenuItem {
                label: "Back / 返回".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::Back),
            },
        ],
        selected: 0,
    }
}

fn build_components_menu() -> MenuScreen {
    MenuScreen {
        title: "Components / 组件".to_string(),
        items: vec![
            MenuItem {
                label: "Web admin / Web管理页".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.components.enable_web_admin),
                    set: Arc::new(|c, v| c.components.enable_web_admin = v),
                }),
            },
            MenuItem {
                label: "Router filter agent / 路由过滤Agent".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.components.enable_router_filter_agent),
                    set: Arc::new(|c, v| c.components.enable_router_filter_agent = v),
                }),
            },
            MenuItem {
                label: "E2E / 端到端".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.components.enable_e2e),
                    set: Arc::new(|c, v| c.components.enable_e2e = v),
                }),
            },
            MenuItem {
                label: "Spearlet with Node / Spearlet含Node".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.components.spearlet_with_node),
                    set: Arc::new(|c, v| c.components.spearlet_with_node = v),
                }),
            },
            MenuItem {
                label: "Llama server / Llama服务".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.components.spearlet_with_llama_server),
                    set: Arc::new(|c, v| c.components.spearlet_with_llama_server = v),
                }),
            },
            MenuItem {
                label: "Back / 返回".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::Back),
            },
        ],
        selected: 0,
    }
}

fn build_logging_menu() -> MenuScreen {
    MenuScreen {
        title: "Logging / 日志".to_string(),
        items: vec![
            MenuItem {
                label: "Debug / 调试模式".to_string(),
                value: "".to_string(),
                kind: ItemKind::ToggleBool(AccessorBool {
                    get: Arc::new(|c| c.logging.debug),
                    set: Arc::new(|c, v| c.logging.debug = v),
                }),
            },
            MenuItem {
                label: "Log level / 日志级别".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.logging.log_level.clone()),
                    set: Arc::new(|c, v| c.logging.log_level = v),
                }),
            },
            MenuItem {
                label: "Log format / 日志格式".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.logging.log_format.clone()),
                    set: Arc::new(|c, v| c.logging.log_format = v),
                }),
            },
            MenuItem {
                label: "Rollout timeout / 部署超时".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.timeouts.rollout.clone()),
                    set: Arc::new(|c, v| c.timeouts.rollout = v),
                }),
            },
            MenuItem {
                label: "Back / 返回".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::Back),
            },
        ],
        selected: 0,
    }
}

fn build_secrets_menu() -> MenuScreen {
    MenuScreen {
        title: "Secrets / 密钥".to_string(),
        items: vec![
            MenuItem {
                label: "OpenAI source / OpenAI来源".to_string(),
                value: "".to_string(),
                kind: ItemKind::Submenu(build_openai_source_menu()),
            },
            MenuItem {
                label: "OpenAI env / OpenAI环境变量".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.secrets.openai.env_name.clone()),
                    set: Arc::new(|c, v| c.secrets.openai.env_name = v),
                }),
            },
            MenuItem {
                label: "k8s secret name / Secret名".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.secrets.openai.k8s_secret_name.clone()),
                    set: Arc::new(|c, v| c.secrets.openai.k8s_secret_name = v),
                }),
            },
            MenuItem {
                label: "k8s secret key / Secret键".to_string(),
                value: "".to_string(),
                kind: ItemKind::EditString(AccessorString {
                    get: Arc::new(|c| c.secrets.openai.k8s_secret_key.clone()),
                    set: Arc::new(|c, v| c.secrets.openai.k8s_secret_key = v),
                }),
            },
            MenuItem {
                label: "Back / 返回".to_string(),
                value: "".to_string(),
                kind: ItemKind::Action(Action::Back),
            },
        ],
        selected: 0,
    }
}

fn build_openai_source_menu() -> MenuScreen {
    let set_source = |source: &'static str| -> AccessorBool {
        let get = move |c: &Config| c.secrets.openai.source == source;
        let set = move |c: &mut Config, v: bool| {
            if v {
                c.secrets.openai.source = source.to_string();
            }
        };
        AccessorBool {
            get: Arc::new(get),
            set: Arc::new(set),
        }
    };

    let items = vec![
        MenuItem {
            label: "from-env".to_string(),
            value: "".to_string(),
            kind: ItemKind::ToggleBool(set_source("from-env")),
        },
        MenuItem {
            label: "skip".to_string(),
            value: "".to_string(),
            kind: ItemKind::ToggleBool(set_source("skip")),
        },
        MenuItem {
            label: "Back / 返回".to_string(),
            value: "".to_string(),
            kind: ItemKind::Action(Action::Back),
        },
    ];

    MenuScreen {
        title: "OpenAI secret source".to_string(),
        items,
        selected: 0,
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let bg = Color::Blue;
    let fg = Color::White;
    let accent = Color::LightCyan;
    let highlight_bg = Color::Yellow;
    let highlight_fg = Color::Black;

    let size = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(size);

    frame.render_widget(Block::default().style(Style::default().bg(bg)), size);

    let pf = port_forward_title(app);
    let title = Line::from(vec![
        Span::raw(format!(
            "{}  (mode={}  ns={}  pf=",
            app.current().title,
            app.cfg.mode.name,
            app.cfg.k8s.namespace
        )),
        Span::styled(
            pf,
            port_forward_status_style(app).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  dirty={})", app.dirty)),
    ]);

    let items: Vec<ListItem> = app
        .current()
        .items
        .iter()
        .enumerate()
        .map(|(idx, it)| {
            let selected = idx == app.current().selected && app.mode == UiMode::Menu;
            let prefix = if selected { ">" } else { " " };
            let label = pad_display_width(&it.label, 32);
            let value = truncate_display_width(&it.value, 48);
            let has_value = !value.is_empty();

            let value_style = if is_port_forward_menu_item(&it.kind) {
                port_forward_status_style(app).bg(Color::Black)
            } else {
                Style::default()
                    .bg(Color::Black)
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            };
            let line = Line::from(vec![
                Span::raw(format!("{:<1} ", prefix)),
                Span::raw(label),
                Span::raw(" "),
                Span::styled(
                    if has_value {
                        format!(" {} ", value)
                    } else {
                        value
                    },
                    if has_value {
                        value_style
                    } else {
                        Style::default()
                    },
                ),
            ]);
            let style = if selected {
                Style::default().bg(highlight_bg).fg(highlight_fg)
            } else {
                Style::default().bg(bg).fg(fg)
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent).bg(bg))
            .title(title)
            .title_style(Style::default().fg(Color::Yellow).bg(bg)),
    );
    frame.render_widget(list, layout[0]);

    let footer = match app.mode {
        UiMode::Menu => {
            let bar_bg = Color::Cyan;
            let bar_fg = Color::Black;
            let key_style = Style::default()
                .bg(bar_bg)
                .fg(bar_fg)
                .add_modifier(Modifier::BOLD);
            let label_style = Style::default().bg(bar_bg).fg(bar_fg);
            let line = Line::from(vec![
                Span::styled(" F1 ", key_style),
                Span::styled("Help ", label_style),
                Span::styled(" F2 ", key_style),
                Span::styled("Save ", label_style),
                Span::styled(" F3 ", key_style),
                Span::styled("Plan ", label_style),
                Span::styled(" F4 ", key_style),
                Span::styled("Apply ", label_style),
                Span::styled(" F5 ", key_style),
                Span::styled("Status ", label_style),
                Span::styled(" F6 ", key_style),
                Span::styled("Cleanup ", label_style),
                Span::styled(" F10 ", key_style),
                Span::styled("Exit", label_style),
            ]);
            Paragraph::new(line).style(Style::default().bg(bar_bg).fg(bar_fg))
        }
        UiMode::EditText => {
            Paragraph::new("Enter save  Esc cancel  F10 exit").style(Style::default().bg(bg).fg(fg))
        }
        UiMode::ViewText => Paragraph::new("↑/↓/PgUp/PgDn scroll  Enter OK  q/esc close  F10 exit")
            .style(Style::default().bg(bg).fg(fg)),
        UiMode::Confirm => Paragraph::new("Tab/←/→ switch  Enter confirm  Esc cancel  F10 exit")
            .style(Style::default().bg(bg).fg(fg)),
    };
    frame.render_widget(footer, layout[1]);

    if app.mode == UiMode::EditText {
        let area = centered_rect(80, 20, size);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent).bg(bg))
            .title(app.input_title.as_str())
            .title_style(Style::default().fg(Color::Yellow).bg(bg))
            .style(Style::default().bg(bg).fg(fg));
        let p = Paragraph::new(app.input.as_str())
            .style(Style::default().bg(bg).fg(fg))
            .block(block);
        frame.render_widget(Clear, area);
        frame.render_widget(p, area);
    }

    if app.mode == UiMode::ViewText {
        let area = centered_rect(90, 70, size);
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
            .margin(1)
            .split(area);

        let p = Paragraph::new(app.view_text.as_str())
            .wrap(Wrap { trim: false })
            .scroll((app.view_scroll, 0))
            .style(Style::default().bg(bg).fg(fg));

        let btn_bg = Color::Cyan;
        let btn_fg = Color::Black;
        let ok_style = Style::default()
            .bg(btn_bg)
            .fg(btn_fg)
            .add_modifier(Modifier::BOLD);

        let buttons = Line::from(vec![Span::styled("[ OK ]", ok_style)]);
        let buttons = Paragraph::new(buttons)
            .alignment(Alignment::Center)
            .style(Style::default().bg(bg).fg(fg));

        let bordered = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent).bg(bg))
            .title(app.view_title.as_str())
            .title_style(Style::default().fg(Color::Yellow).bg(bg))
            .style(Style::default().bg(bg).fg(fg));

        frame.render_widget(Clear, area);
        frame.render_widget(bordered, area);
        frame.render_widget(p, inner[0]);
        frame.render_widget(buttons, inner[1]);
    }

    if app.mode == UiMode::Confirm {
        let area = centered_rect(70, 40, size);
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
            .margin(1)
            .split(area);

        let p = Paragraph::new(app.confirm_text.as_str())
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(bg).fg(fg));

        let btn_bg = Color::Cyan;
        let btn_fg = Color::Black;
        let btn_sel_style = Style::default()
            .bg(btn_bg)
            .fg(btn_fg)
            .add_modifier(Modifier::BOLD);
        let btn_style = Style::default().bg(bg).fg(fg);

        let ok_style = if app.confirm_choice == ConfirmChoice::Ok {
            btn_sel_style
        } else {
            btn_style
        };
        let cancel_style = if app.confirm_choice == ConfirmChoice::Cancel {
            btn_sel_style
        } else {
            btn_style
        };

        let buttons = Line::from(vec![
            Span::styled("[ OK ]", ok_style),
            Span::raw("  "),
            Span::styled("[ Cancel ]", cancel_style),
        ]);
        let buttons = Paragraph::new(buttons)
            .alignment(Alignment::Center)
            .style(Style::default().bg(bg).fg(fg));

        let bordered = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent).bg(bg))
            .title(app.confirm_title.as_str())
            .title_style(Style::default().fg(Color::Yellow).bg(bg))
            .style(Style::default().bg(bg).fg(fg));

        frame.render_widget(Clear, area);
        frame.render_widget(bordered, area);
        frame.render_widget(p, inner[0]);
        frame.render_widget(buttons, inner[1]);
    }
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
            resume_tui(terminal).ok();
            match res {
                Ok(()) => {
                    app.view_title = "Apply done / 部署完成".to_string();
                    app.view_text = "Apply finished.\n".to_string();
                    if app.cfg.mode.name == "k8s-kind"
                        && app.cfg.k8s.port_forward.enabled
                        && app.cfg.k8s.port_forward.auto_start
                    {
                        if let Err(e) = start_port_forward(app) {
                            app.view_text
                                .push_str(&format!("\nPort-forward failed:\n{e:?}\n"));
                        } else {
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
            let res = deploy::status(&app.cfg);
            resume_tui(terminal).ok();
            match res {
                Ok(()) => {
                    app.view_title = "Status done / 状态完成".to_string();
                    app.view_text = "Status finished.\n".to_string();
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
            stop_port_forward(app);
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
        PendingAction::StartPortForward => {
            match start_port_forward(app) {
                Ok(()) => {
                    app.view_title = "Port-forward started / 端口转发已启动".to_string();
                    if let Some(pf) = &app.port_forward {
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
            refresh_current_values(app);
            app.view_scroll = 0;
            app.mode = UiMode::ViewText;
            Ok(())
        }
        PendingAction::StopPortForward => {
            stop_port_forward(app);
            refresh_current_values(app);
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
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
    match &app.port_forward {
        Some(pf) => format!("On:{}", pf.local_port),
        None => {
            if app.cfg.k8s.port_forward.enabled {
                "Off".to_string()
            } else {
                "Disabled".to_string()
            }
        }
    }
}

fn is_port_forward_menu_item(kind: &ItemKind) -> bool {
    matches!(kind, ItemKind::Submenu(screen) if screen.title.starts_with("Port forward"))
}

fn port_forward_status_style(app: &App) -> Style {
    if !app.cfg.k8s.port_forward.enabled {
        return Style::default().fg(Color::DarkGray);
    }
    if app.port_forward.is_some() {
        Style::default().fg(Color::LightGreen)
    } else {
        Style::default().fg(Color::LightRed)
    }
}

fn port_forward_summary(app: &App) -> String {
    if !app.cfg.k8s.port_forward.enabled {
        return "Disabled".to_string();
    }
    match &app.port_forward {
        Some(pf) => format!("On {} ({}->{})", pf.url, pf.local_port, pf.remote_port),
        None => {
            if app.cfg.k8s.port_forward.auto_start {
                format!("Off (auto) :{}", app.cfg.k8s.port_forward.local_port)
            } else {
                format!("Off :{}", app.cfg.k8s.port_forward.local_port)
            }
        }
    }
}

fn refresh_port_forward_status(app: &mut App) {
    let mut changed = false;
    if let Some(pf) = &mut app.port_forward {
        if let Ok(Some(_)) = pf.child.try_wait() {
            app.port_forward = None;
            changed = true;
        }
    }
    if changed {
        refresh_current_values(app);
    }
}

fn stop_port_forward(app: &mut App) {
    if let Some(mut pf) = app.port_forward.take() {
        let _ = pf.child.kill();
        let _ = pf.child.wait();
    }
}

fn start_port_forward(app: &mut App) -> anyhow::Result<()> {
    stop_port_forward(app);
    if app.cfg.mode.name != "k8s-kind" {
        return Err(anyhow::anyhow!(
            "port-forward currently supports mode=k8s-kind only"
        ));
    }
    if !app.cfg.k8s.port_forward.enabled {
        return Err(anyhow::anyhow!("k8s.port_forward.enabled is false"));
    }

    let repo_root = config::repo_root()?;
    let kubeconfig = repo_root.join(&app.cfg.k8s.kind.kubeconfig_file);
    let kubeconfig_str = kubeconfig.to_string_lossy().to_string();
    let ns = app.cfg.k8s.namespace.clone();
    let release = app.cfg.k8s.release_name.clone();
    let local_port = app.cfg.k8s.port_forward.local_port;
    let remote_port = app.cfg.k8s.port_forward.remote_port;
    let svc = format!("svc/{}-spear-sms", release);

    let log_dir = repo_root.join(&app.cfg.paths.state_dir);
    std::fs::create_dir_all(&log_dir).ok();
    let log_path = log_dir.join("port-forward.log");
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

    let url = format!("http://127.0.0.1:{}/", local_port);
    app.port_forward = Some(PortForwardState {
        child,
        local_port,
        remote_port,
        url,
        log_path,
    });
    refresh_current_values(app);
    Ok(())
}

fn truncate_display_width(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max {
            if max >= 1 {
                out.push('…');
            }
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}

fn pad_display_width(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > width {
            if width >= 1 {
                out.push('…');
                w = width;
            }
            break;
        }
        out.push(ch);
        w += cw;
    }
    if w < width {
        out.push_str(&" ".repeat(width - w));
    }
    out
}

fn scope_contains(scope: &str, key: &str) -> bool {
    scope
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .any(|s| s == key)
}

fn scope_set(scope: &mut String, key: &str, enabled: bool) {
    let mut parts: Vec<String> = scope
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    parts.retain(|p| p != key);
    if enabled {
        parts.push(key.to_string());
    }

    let order = ["release", "secret", "namespace", "kind"];
    parts.sort_by_key(|p| order.iter().position(|x| *x == p.as_str()).unwrap_or(99));
    parts.dedup();

    if parts.is_empty() {
        *scope = "release".to_string();
    } else {
        *scope = parts.join(",");
    }
}
