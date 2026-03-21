use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::*;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::{help_text, port_forward_status_style, port_forward_title, App, ItemKind, UiMode};

fn truncate_left_display_width(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(s) <= max {
        return s.to_string();
    }
    if max == 1 {
        return "…".to_string();
    }

    let mut out = String::new();
    out.push('…');

    let mut w = 1usize;
    for ch in s.chars().rev() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max {
            break;
        }
        out.insert(1, ch);
        w += cw;
    }
    out
}

fn breadcrumb(app: &App, max_width: usize) -> String {
    let titles: Vec<&str> = app.stack.iter().map(|s| s.title.as_str()).collect();
    if titles.is_empty() {
        return String::new();
    }
    let full = titles.join(" > ");
    truncate_left_display_width(&full, max_width)
}

pub(super) fn ui(frame: &mut Frame, app: &App) {
    let bg = Color::Blue;
    let fg = Color::White;
    let accent = Color::LightCyan;
    let highlight_bg = Color::Yellow;
    let highlight_fg = Color::Black;
    let dialog_bg = Color::Gray;
    let dialog_fg = Color::Black;
    let dialog_border = Color::Black;
    let dialog_title_fg = Color::Blue;
    let dialog_button_sel_bg = Color::Cyan;
    let dialog_button_sel_fg = Color::Black;
    let dialog_button_fg = Color::Blue;

    let render_dialog_shadow = |frame: &mut Frame, area: Rect, bounds: Rect| {
        let sx = area.x.saturating_add(2);
        let sy = area.y.saturating_add(1);
        if sx >= bounds.width || sy >= bounds.height {
            return;
        }
        let max_w = bounds.width.saturating_sub(sx);
        let max_h = bounds.height.saturating_sub(sy);
        let shadow = Rect {
            x: sx,
            y: sy,
            width: area.width.min(max_w),
            height: area.height.min(max_h),
        };
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Black)),
            shadow,
        );
    };

    let size = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(size);

    frame.render_widget(Block::default().style(Style::default().bg(bg)), size);

    let list_area = layout[0];

    let pf = port_forward_title(app);
    let title_suffix_plain = format!(
        "  (mode={}  ns={}  pf={}  dirty={})",
        app.cfg.mode.name, app.cfg.k8s.namespace, pf, app.dirty
    );
    let title_area_width = list_area.width.saturating_sub(2) as usize;
    let suffix_w = UnicodeWidthStr::width(title_suffix_plain.as_str());
    let breadcrumb_w = title_area_width.saturating_sub(suffix_w);
    let nav = breadcrumb(app, breadcrumb_w);
    let title = Line::from(vec![
        Span::raw(format!(
            "{}  (mode={}  ns={}  pf=",
            nav, app.cfg.mode.name, app.cfg.k8s.namespace
        )),
        Span::styled(
            pf,
            port_forward_status_style(app).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  dirty={})", app.dirty)),
    ]);

    let inner_width = list_area.width.saturating_sub(2) as usize;
    let prefix_width = 2usize;
    let gap_width = 1usize;
    let min_value_content_width = 10usize;
    let label_cap = 48usize;
    let max_label_width = app
        .current()
        .items
        .iter()
        .map(|it| UnicodeWidthStr::width(it.label.as_str()))
        .max()
        .unwrap_or(0)
        .min(label_cap);

    let max_label_allowed = inner_width
        .saturating_sub(prefix_width)
        .saturating_sub(gap_width)
        .saturating_sub(min_value_content_width);
    let label_width = max_label_width.min(max_label_allowed);

    let items: Vec<ListItem> = app
        .current()
        .items
        .iter()
        .enumerate()
        .map(|(idx, it)| {
            let selected = idx == app.current().selected && app.mode == UiMode::Menu;
            let prefix = if selected { ">" } else { " " };
            let label = pad_display_width(&it.label, label_width);
            let value_width = inner_width
                .saturating_sub(prefix_width)
                .saturating_sub(label_width)
                .saturating_sub(gap_width);
            let wrap_value = !matches!(it.kind, ItemKind::ToggleBool(_) | ItemKind::ToggleScope(_));
            let value_content_width = value_width.saturating_sub(if wrap_value { 2 } else { 0 });
            let value = if it.value.is_empty() || value_content_width == 0 {
                String::new()
            } else {
                truncate_display_width(&it.value, value_content_width)
            };
            let has_value = !value.is_empty();

            let value_style = if has_value {
                Style::default().bg(Color::Black).fg(Color::White)
            } else {
                Style::default().bg(bg).fg(fg)
            };

            let mut spans = vec![Span::raw(format!("{:<1} ", prefix)), Span::raw(label)];
            if has_value {
                spans.push(Span::raw(" "));
                let rendered_value = if wrap_value {
                    format!("[{}]", value)
                } else {
                    value
                };
                spans.push(Span::styled(rendered_value, value_style));
            }
            let line = Line::from(spans);

            let style = if selected {
                Style::default()
                    .bg(highlight_bg)
                    .fg(highlight_fg)
                    .add_modifier(Modifier::BOLD)
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
            .style(Style::default().bg(bg).fg(fg))
            .title(title)
            .title_style(Style::default().fg(Color::Yellow).bg(bg)),
    );
    frame.render_widget(list, list_area);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("F1", Style::default().fg(Color::Yellow)),
        Span::raw(" Help  "),
        Span::styled("F2", Style::default().fg(Color::Yellow)),
        Span::raw(" Save  "),
        Span::styled("F3", Style::default().fg(Color::Yellow)),
        Span::raw(" Plan  "),
        Span::styled("F4", Style::default().fg(Color::Yellow)),
        Span::raw(" Apply  "),
        Span::styled("F5", Style::default().fg(Color::Yellow)),
        Span::raw(" Status  "),
        Span::styled("F6", Style::default().fg(Color::Yellow)),
        Span::raw(" Cleanup  "),
        Span::styled("F10", Style::default().fg(Color::Yellow)),
        Span::raw(" Exit"),
    ]))
    .style(Style::default().bg(bg).fg(fg))
    .alignment(Alignment::Center);
    frame.render_widget(footer, layout[1]);

    if app.mode == UiMode::EditText {
        let area = centered_rect(80, 20, size);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(dialog_border).bg(dialog_bg))
            .title(app.input_title.as_str())
            .title_style(
                Style::default()
                    .fg(dialog_title_fg)
                    .bg(dialog_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(dialog_bg).fg(dialog_fg));
        let p = Paragraph::new(app.input.as_str())
            .style(Style::default().bg(dialog_bg).fg(dialog_fg))
            .block(block);
        frame.render_widget(Clear, area);
        render_dialog_shadow(frame, area, size);
        frame.render_widget(p, area);
    }

    if app.mode == UiMode::ViewText {
        let area = centered_rect(90, 70, size);
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ]
                .as_ref(),
            )
            .margin(1)
            .split(area);

        let p = Paragraph::new(app.view_text.as_str())
            .wrap(Wrap { trim: false })
            .scroll((app.view_scroll, 0))
            .style(Style::default().bg(dialog_bg).fg(dialog_fg));

        let ok_style = Style::default()
            .bg(dialog_button_sel_bg)
            .fg(dialog_button_sel_fg)
            .add_modifier(Modifier::BOLD);

        let buttons = Line::from(vec![Span::styled("[ OK ]", ok_style)]);
        let buttons = Paragraph::new(buttons)
            .alignment(Alignment::Center)
            .style(Style::default().bg(dialog_bg).fg(dialog_fg));

        let bordered = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(dialog_border).bg(dialog_bg))
            .title(app.view_title.as_str())
            .title_style(
                Style::default()
                    .fg(dialog_title_fg)
                    .bg(dialog_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(dialog_bg).fg(dialog_fg));

        let divider_width = area.width.saturating_sub(2) as usize;
        let divider_line = "─".repeat(divider_width);
        let divider =
            Paragraph::new(divider_line).style(Style::default().bg(dialog_bg).fg(dialog_border));

        frame.render_widget(Clear, area);
        render_dialog_shadow(frame, area, size);
        frame.render_widget(bordered, area);
        frame.render_widget(p, inner[0]);
        frame.render_widget(divider, inner[1]);
        frame.render_widget(buttons, inner[2]);
    }

    if app.mode == UiMode::Confirm {
        let area = centered_rect(70, 40, size);
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ]
                .as_ref(),
            )
            .margin(1)
            .split(area);

        let p = Paragraph::new(app.confirm_text.as_str())
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(dialog_bg).fg(dialog_fg));

        let btn_sel_style = Style::default()
            .bg(dialog_button_sel_bg)
            .fg(dialog_button_sel_fg)
            .add_modifier(Modifier::BOLD);
        let btn_style = Style::default().bg(dialog_bg).fg(dialog_button_fg);

        let ok_style = if app.confirm_choice == super::ConfirmChoice::Ok {
            btn_sel_style
        } else {
            btn_style
        };
        let cancel_style = if app.confirm_choice == super::ConfirmChoice::Cancel {
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
            .style(Style::default().bg(dialog_bg).fg(dialog_fg));

        let bordered = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(dialog_border).bg(dialog_bg))
            .title(app.confirm_title.as_str())
            .title_style(
                Style::default()
                    .fg(dialog_title_fg)
                    .bg(dialog_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(dialog_bg).fg(dialog_fg));

        let divider_width = area.width.saturating_sub(2) as usize;
        let divider_line = "─".repeat(divider_width);
        let divider =
            Paragraph::new(divider_line).style(Style::default().bg(dialog_bg).fg(dialog_border));

        frame.render_widget(Clear, area);
        render_dialog_shadow(frame, area, size);
        frame.render_widget(bordered, area);
        frame.render_widget(p, inner[0]);
        frame.render_widget(divider, inner[1]);
        frame.render_widget(buttons, inner[2]);
    }

    if app.mode == UiMode::ViewText && app.view_title == "Help / 帮助" && app.view_text.is_empty()
    {
        let _ = help_text();
    }
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

fn pad_display_width(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if w + cw > width {
            break;
        }
        out.push(ch);
        w += cw;
    }
    while w < width {
        out.push(' ');
        w += 1;
    }
    out
}

fn truncate_display_width(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if w + cw > max {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}
