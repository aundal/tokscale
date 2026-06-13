use chrono::Local;
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, Table,
};

use super::widgets::{
    format_cache_hit_rate, format_cost, format_cost_per_million, format_tokens,
    get_client_display_name, get_provider_display_name, viewport_scrollbar_state,
};
use crate::tui::app::{App, SortDirection, SortField, Tab};

/// Display noun for the active period tab title (e.g. "Weekly").
fn period_noun(app: &App) -> &'static str {
    match app.current_tab {
        Tab::Weekly => "Weekly",
        Tab::Monthly => "Monthly",
        Tab::Yearly => "Yearly",
        _ => "Period",
    }
}

/// First-column header for the active period tab (e.g. "Week").
fn period_col_header(app: &App) -> &'static str {
    match app.current_tab {
        Tab::Weekly => "Week",
        Tab::Monthly => "Month",
        Tab::Yearly => "Year",
        _ => "Period",
    }
}

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.is_period_detail_active() {
        render_detail(frame, app, area);
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(Span::styled(
            format!(" {} Usage ", period_noun(app)),
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(app.theme.background));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height.saturating_sub(1) as usize;
    app.set_max_visible_items(visible_height);

    let periods = app.get_sorted_periods();
    if periods.is_empty() {
        let empty_msg = Paragraph::new("No usage data found. Press 'r' to refresh.")
            .style(Style::default().fg(app.theme.muted))
            .alignment(Alignment::Center);
        frame.render_widget(empty_msg, inner);
        return;
    }

    let is_narrow = app.is_narrow();
    let is_very_narrow = app.is_very_narrow();
    let has_turn_data = periods.iter().any(|p| p.turn_count > 0);
    let label_col_header = period_col_header(app);
    let sort_field = app.sort_field;
    let sort_direction = app.sort_direction;
    let scroll_offset = app.scroll_offset;
    let selected_index = app.selected_index;
    let theme_accent = app.theme.accent;
    let theme_selection = app.theme.selection;
    let metric_input_style = app.theme.metric_input_style();
    let metric_output_style = app.theme.metric_output_style();
    let metric_cache_read_style = app.theme.metric_cache_read_style();
    let metric_cache_write_style = app.theme.metric_cache_write_style();
    let current_row_style = app.theme.current_row_style();
    let striped_row_style = app.theme.striped_row_style();
    let today = Local::now().date_naive();

    // Period labels ("2026-W23", "2026-05", "2026") are short and fixed-width,
    // so unlike the Daily tab there is no year-dropping/compaction logic.
    let label_col_width: u16 = 9;

    let header_cells = if is_very_narrow {
        vec![label_col_header, "Cost"]
    } else if is_narrow {
        if has_turn_data {
            vec![label_col_header, "Turn", "Msgs", "Tokens", "Cost"]
        } else {
            vec![label_col_header, "Msgs", "Tokens", "Cost"]
        }
    } else if has_turn_data {
        vec![
            label_col_header,
            "Turn",
            "Msgs",
            "Input",
            "Output",
            "Cache R",
            "Cache W",
            "Cache×",
            "Total",
            "Cost",
            "Cost/1M",
        ]
    } else {
        vec![
            label_col_header,
            "Msgs",
            "Input",
            "Output",
            "Cache R",
            "Cache W",
            "Cache×",
            "Total",
            "Cost",
            "Cost/1M",
        ]
    };

    let sort_indicator = |field: SortField| -> &'static str {
        if sort_field == field {
            match sort_direction {
                SortDirection::Ascending => " ▲",
                SortDirection::Descending => " ▼",
            }
        } else {
            ""
        }
    };

    let header = Row::new(
        header_cells
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let indicator = match (i, is_narrow, is_very_narrow) {
                    (0, _, _) => sort_indicator(SortField::Date),
                    (8, false, false) if has_turn_data => sort_indicator(SortField::Tokens),
                    (7, false, false) if !has_turn_data => sort_indicator(SortField::Tokens),
                    (3, true, false) if has_turn_data => sort_indicator(SortField::Tokens),
                    (2, true, false) if !has_turn_data => sort_indicator(SortField::Tokens),
                    (9, false, false) if has_turn_data => sort_indicator(SortField::Cost),
                    (8, false, false) if !has_turn_data => sort_indicator(SortField::Cost),
                    (4, true, false) if has_turn_data => sort_indicator(SortField::Cost),
                    (3, true, false) if !has_turn_data => sort_indicator(SortField::Cost),
                    (1, _, true) => sort_indicator(SortField::Cost),
                    _ => "",
                };
                Cell::from(format!("{}{}", h, indicator))
            })
            .collect::<Vec<_>>(),
    )
    .style(
        Style::default()
            .fg(theme_accent)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let periods_len = periods.len();
    let start = scroll_offset.min(periods_len);
    let end = (start + visible_height).min(periods_len);

    if start >= periods_len {
        return;
    }

    let rows: Vec<Row> = periods[start..end]
        .iter()
        .enumerate()
        .map(|(i, period)| {
            let idx = i + start;
            let is_selected = idx == selected_index;
            let is_striped = idx % 2 == 1;
            let is_current = period.start_date <= today && today <= period.end_date;

            let cells: Vec<Cell> = if is_very_narrow {
                vec![
                    Cell::from(period.label.clone()).style(if is_current {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    }),
                    Cell::from(format_cost(period.cost)).style(Style::default().fg(Color::Green)),
                ]
            } else if is_narrow {
                let mut cells = vec![Cell::from(period.label.clone()).style(if is_current {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                })];
                if has_turn_data {
                    let turn_str = if period.turn_count > 0 {
                        period.turn_count.to_string()
                    } else {
                        "\u{2014}".to_string()
                    };
                    cells.push(Cell::from(turn_str));
                }
                cells.extend([
                    Cell::from(period.message_count.to_string()),
                    Cell::from(format_tokens(period.tokens.total())),
                    Cell::from(format_cost(period.cost)).style(Style::default().fg(Color::Green)),
                ]);
                cells
            } else {
                let mut cells = vec![Cell::from(period.label.clone()).style(if is_current {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                })];
                if has_turn_data {
                    let turn_str = if period.turn_count > 0 {
                        period.turn_count.to_string()
                    } else {
                        "\u{2014}".to_string()
                    };
                    cells.push(Cell::from(turn_str));
                }
                cells.extend([
                    Cell::from(period.message_count.to_string()),
                    Cell::from(format_tokens(period.tokens.input)).style(metric_input_style),
                    Cell::from(format_tokens(period.tokens.output)).style(metric_output_style),
                    Cell::from(format_tokens(period.tokens.cache_read))
                        .style(metric_cache_read_style),
                    Cell::from(format_tokens(period.tokens.cache_write))
                        .style(metric_cache_write_style),
                    Cell::from(format_cache_hit_rate(
                        period.tokens.cache_read,
                        period.tokens.input,
                        period.tokens.cache_write,
                    ))
                    .style(Style::default().fg(Color::Cyan)),
                    Cell::from(format_tokens(period.tokens.total())),
                    Cell::from(format_cost(period.cost)).style(Style::default().fg(Color::Green)),
                    Cell::from(format_cost_per_million(period.cost, period.tokens.total()))
                        .style(Style::default().fg(Color::Rgb(150, 200, 150))),
                ]);
                cells
            };

            let row_style = if is_selected {
                Style::default().bg(theme_selection)
            } else if is_current {
                current_row_style
            } else if is_striped {
                striped_row_style
            } else {
                Style::default()
            };

            Row::new(cells).style(row_style).height(1)
        })
        .collect();

    let widths = if is_very_narrow {
        vec![Constraint::Percentage(60), Constraint::Percentage(40)]
    } else if is_narrow && has_turn_data {
        vec![
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ]
    } else if is_narrow {
        vec![
            Constraint::Percentage(35),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
            Constraint::Percentage(20),
        ]
    } else if has_turn_data {
        vec![
            Constraint::Length(label_col_width),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ]
    } else {
        vec![
            Constraint::Length(label_col_width),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ]
    };

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(theme_selection));

    frame.render_widget(table, inner);

    if periods_len > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));

        let mut scrollbar_state =
            viewport_scrollbar_state(periods_len, scroll_offset, visible_height);

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut scrollbar_state,
        );
    }
}

fn render_detail(frame: &mut Frame, app: &mut App, area: Rect) {
    let noun = period_noun(app);
    let title = app
        .period_detail_label()
        .map(|label| format!(" {} Detail: {} ", noun, label))
        .unwrap_or_else(|| format!(" {} Detail ", noun));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(Span::styled(
            title,
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(app.theme.background));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height.saturating_sub(1) as usize;
    app.set_max_visible_items(visible_height);

    let rows_data = app.get_sorted_period_detail_rows();
    if rows_data.is_empty() {
        let empty_msg =
            Paragraph::new("No model details found for this period. Press Esc to go back.")
                .style(Style::default().fg(app.theme.muted))
                .alignment(Alignment::Center);
        frame.render_widget(empty_msg, inner);
        return;
    }

    let is_narrow = app.is_narrow();
    let is_very_narrow = app.is_very_narrow();
    let sort_field = app.sort_field;
    let sort_direction = app.sort_direction;
    let scroll_offset = app.scroll_offset;
    let selected_index = app.selected_index;
    let theme_accent = app.theme.accent;
    let theme_muted = app.theme.muted;
    let theme_selection = app.theme.selection;
    let metric_input_style = app.theme.metric_input_style();
    let metric_output_style = app.theme.metric_output_style();
    let metric_cache_read_style = app.theme.metric_cache_read_style();
    let metric_cache_write_style = app.theme.metric_cache_write_style();
    let striped_row_style = app.theme.striped_row_style();

    let header_cells = if is_very_narrow {
        vec!["Model", "Cost"]
    } else if is_narrow {
        vec!["Model", "Source", "Msgs", "Tokens", "Cost"]
    } else {
        vec![
            "#", "Model", "Provider", "Source", "Msgs", "Input", "Output", "Cache R", "Cache W",
            "Cache×", "Total", "Cost",
        ]
    };

    let sort_indicator = |field: SortField| -> &'static str {
        if sort_field == field {
            match sort_direction {
                SortDirection::Ascending => " ▲",
                SortDirection::Descending => " ▼",
            }
        } else {
            ""
        }
    };

    let header = Row::new(
        header_cells
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let indicator = match (i, is_narrow, is_very_narrow) {
                    (10, false, false) => sort_indicator(SortField::Tokens),
                    (11, false, false) => sort_indicator(SortField::Cost),
                    (3, true, false) => sort_indicator(SortField::Tokens),
                    (4, true, false) => sort_indicator(SortField::Cost),
                    (1, _, true) => sort_indicator(SortField::Cost),
                    _ => "",
                };
                Cell::from(format!("{}{}", h, indicator))
            })
            .collect::<Vec<_>>(),
    )
    .style(
        Style::default()
            .fg(theme_accent)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let detail_len = rows_data.len();
    let start = scroll_offset.min(detail_len);
    let end = (start + visible_height).min(detail_len);

    if start >= detail_len {
        return;
    }

    let rows: Vec<Row> = rows_data[start..end]
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let idx = i + start;
            let is_selected = idx == selected_index;
            let is_striped = idx % 2 == 1;
            let model_color = app.model_color_for(row.provider, row.color_key);

            let cells: Vec<Cell> = if is_very_narrow {
                vec![
                    Cell::from(truncate(row.model, 18)).style(
                        Style::default()
                            .fg(model_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Cell::from(format_cost(row.cost)).style(Style::default().fg(Color::Green)),
                ]
            } else if is_narrow {
                vec![
                    Cell::from(truncate(row.model, 24)).style(
                        Style::default()
                            .fg(model_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Cell::from(get_client_display_name(row.source))
                        .style(Style::default().fg(theme_muted)),
                    Cell::from(row.messages.to_string()),
                    Cell::from(format_tokens(row.tokens.total())),
                    Cell::from(format_cost(row.cost)).style(Style::default().fg(Color::Green)),
                ]
            } else {
                vec![
                    Cell::from(format!("{}", idx + 1)).style(Style::default().fg(theme_muted)),
                    Cell::from(truncate(row.model, 30)).style(
                        Style::default()
                            .fg(model_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Cell::from(get_provider_display_name(row.provider)),
                    Cell::from(get_client_display_name(row.source))
                        .style(Style::default().fg(theme_muted)),
                    Cell::from(row.messages.to_string()),
                    Cell::from(format_tokens(row.tokens.input)).style(metric_input_style),
                    Cell::from(format_tokens(row.tokens.output)).style(metric_output_style),
                    Cell::from(format_tokens(row.tokens.cache_read)).style(metric_cache_read_style),
                    Cell::from(format_tokens(row.tokens.cache_write))
                        .style(metric_cache_write_style),
                    Cell::from(format_cache_hit_rate(
                        row.tokens.cache_read,
                        row.tokens.input,
                        row.tokens.cache_write,
                    ))
                    .style(Style::default().fg(Color::Cyan)),
                    Cell::from(format_tokens(row.tokens.total())),
                    Cell::from(format_cost(row.cost)).style(Style::default().fg(Color::Green)),
                ]
            };

            let row_style = if is_selected {
                Style::default().bg(theme_selection)
            } else if is_striped {
                striped_row_style
            } else {
                Style::default()
            };

            Row::new(cells).style(row_style).height(1)
        })
        .collect();

    let widths = if is_very_narrow {
        vec![Constraint::Percentage(70), Constraint::Percentage(30)]
    } else if is_narrow {
        vec![
            Constraint::Percentage(42),
            Constraint::Percentage(18),
            Constraint::Percentage(12),
            Constraint::Percentage(15),
            Constraint::Percentage(13),
        ]
    } else {
        vec![
            Constraint::Length(3),
            Constraint::Min(20),
            Constraint::Length(16),
            Constraint::Length(14),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
        ]
    };

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(theme_selection));

    frame.render_widget(table, inner);

    if detail_len > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));

        let mut scrollbar_state =
            viewport_scrollbar_state(detail_len, scroll_offset, visible_height);

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut scrollbar_state,
        );
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else if max_chars <= 3 {
        s.chars().take(max_chars).collect()
    } else {
        let head: String = s.chars().take(max_chars - 3).collect();
        format!("{}...", head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::TuiConfig;
    use crate::tui::data::{DailyModelInfo, DailySourceInfo, DailyUsage, TokenBreakdown, UsageData};
    use chrono::NaiveDate;
    use ratatui::{backend::TestBackend, Terminal};
    use std::collections::BTreeMap;

    fn day(date: &str, model: &str, cost: f64) -> DailyUsage {
        let tokens = TokenBreakdown {
            input: 100,
            output: 10,
            cache_read: 5,
            cache_write: 0,
            reasoning: 0,
        };
        let mut models = BTreeMap::new();
        models.insert(
            model.to_string(),
            DailyModelInfo {
                provider: "anthropic".to_string(),
                display_name: model.to_string(),
                color_key: model.to_string(),
                tokens: tokens.clone(),
                cost,
                messages: 1,
            },
        );
        let mut source_breakdown = BTreeMap::new();
        source_breakdown.insert(
            "claude".to_string(),
            DailySourceInfo {
                tokens: tokens.clone(),
                cost,
                models,
            },
        );
        DailyUsage {
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            tokens,
            cost,
            source_breakdown,
            message_count: 1,
            turn_count: 0,
        }
    }

    fn make_app(tab: Tab, width: u16) -> App {
        let config = TuiConfig {
            theme: "blue".to_string(),
            refresh: 0,
            sessions_path: None,
            clients: None,
            since: None,
            until: None,
            year: None,
            initial_tab: Some(tab),
        };
        let mut app = App::new_with_cached_data(config, None).unwrap();
        app.terminal_width = width;
        app.update_data(UsageData {
            daily: vec![
                day("2026-05-19", "claude-sonnet-4-5", 2.0),
                day("2026-05-20", "claude-sonnet-4-5", 3.0),
            ],
            ..Default::default()
        });
        app.current_tab = tab;
        app
    }

    fn render_body(app: &mut App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, app, Rect::new(0, 0, width, height)))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(width as usize)
            .map(|row| {
                row.iter()
                    .map(|c| c.symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn monthly_tab_renders_title_and_month_label() {
        let mut app = make_app(Tab::Monthly, 130);
        let body = render_body(&mut app, 130, 12);
        assert!(body.contains("Monthly Usage"), "expected title\n{body}");
        assert!(body.contains("2026-05"), "expected month label\n{body}");
    }

    #[test]
    fn weekly_tab_renders_week_header() {
        let mut app = make_app(Tab::Weekly, 130);
        let body = render_body(&mut app, 130, 12);
        assert!(body.contains("Weekly Usage"), "expected title\n{body}");
        // ISO week label for May 2026 is week 21 (2026-W21).
        assert!(body.contains("2026-W21"), "expected ISO week label\n{body}");
    }

    #[test]
    fn yearly_tab_renders_detail_after_enter() {
        let mut app = make_app(Tab::Yearly, 130);
        app.selected_index = 0;
        app.handle_key_event(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        let body = render_body(&mut app, 130, 12);
        assert!(body.contains("Yearly Detail"), "expected detail title\n{body}");
        assert!(
            body.contains("claude-sonnet-4-5"),
            "expected merged model row\n{body}"
        );
    }
}
