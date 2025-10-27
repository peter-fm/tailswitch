use crate::config::{Config, Tailnet};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;

pub struct App {
    options: Vec<(String, Option<String>, bool, bool)>, // (name, account, is_existing_profile, is_active)
    list_state: ListState,
    should_quit: bool,
    status_message: Option<String>,
}

impl App {
    pub fn new_with_options(options: Vec<(String, Option<String>, bool, bool)>) -> Self {
        let mut list_state = ListState::default();
        if !options.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            options,
            list_state,
            should_quit: false,
            status_message: None,
        }
    }

    pub fn run(&mut self) -> Result<Option<Tailnet>> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn run_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<Option<Tailnet>> {
        let mut selected_tailnet = None;

        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            self.should_quit = true;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            self.next();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            self.previous();
                        }
                        KeyCode::Enter => {
                            if let Some(index) = self.list_state.selected() {
                                if index < self.options.len() {
                                    let (name, _, _, _) = &self.options[index];
                                    selected_tailnet = Some(Tailnet {
                                        name: name.clone(),
                                        login_server: None,
                                        auth_key: None,
                                    });
                                    self.should_quit = true;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(selected_tailnet)
    }

    fn ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(f.area());

        self.render_header(f, chunks[0]);
        self.render_tailnet_list(f, chunks[1]);
        self.render_footer(f, chunks[2]);
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let title = Paragraph::new("TailSwitch - Tailscale Network Switcher")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, area);
    }

    fn render_tailnet_list(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .options
            .iter()
            .map(|(name, account, is_profile, is_active)| {
                let mut lines = vec![];

                if *is_profile {
                    // Existing profile - show with checkmark and star if active
                    let prefix = if *is_active {
                        Span::styled("★ ", Style::default().fg(Color::Yellow))
                    } else {
                        Span::styled("  ", Style::default())
                    };

                    lines.push(Line::from(vec![
                        prefix,
                        Span::styled(name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                        if *is_active {
                            Span::styled(" (active)", Style::default().fg(Color::Green))
                        } else {
                            Span::styled("", Style::default())
                        },
                    ]));

                    if let Some(acc) = account {
                        lines.push(Line::from(vec![
                            Span::styled("    ", Style::default()),
                            Span::styled(acc, Style::default().fg(Color::Gray)),
                        ]));
                    }
                } else {
                    // New profile from config - show with plus
                    lines.push(Line::from(vec![
                        Span::styled("+ ", Style::default().fg(Color::Yellow)),
                        Span::styled(name, Style::default().fg(Color::White)),
                        Span::styled(" (add new)", Style::default().fg(Color::DarkGray)),
                    ]));
                }

                ListItem::new(lines)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Select Tailnet (↑/↓ or j/k to navigate, Enter to select, q to quit)"),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let config_path = Config::get_config_path_string().unwrap_or_else(|_| "Unknown".to_string());
        let footer_text = if let Some(ref msg) = self.status_message {
            msg.clone()
        } else {
            format!("Config: {}", config_path)
        };

        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, area);
    }

    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.options.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.options.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }
}

pub struct UrlDisplayApp {
    url: String,
    tailnet_name: String,
    should_quit: bool,
    open_browser: bool,
}

impl UrlDisplayApp {
    pub fn new(url: String, tailnet_name: String) -> Self {
        Self {
            url,
            tailnet_name,
            should_quit: false,
            open_browser: false,
        }
    }

    pub fn run(&mut self) -> Result<bool> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn run_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<bool> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Enter => {
                            self.open_browser = true;
                            self.should_quit = true;
                        }
                        KeyCode::Char('q') | KeyCode::Esc => {
                            self.should_quit = true;
                        }
                        KeyCode::Char('c') => {
                            // Future: copy to clipboard
                            self.should_quit = true;
                        }
                        _ => {}
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(self.open_browser)
    }

    fn ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(5),
            ])
            .split(f.area());

        self.render_header(f, chunks[0]);
        self.render_url_box(f, chunks[1]);
        self.render_instructions(f, chunks[2]);
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let title = Paragraph::new(format!("Authentication Required - {}", self.tailnet_name))
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, area);
    }

    fn render_url_box(&self, f: &mut Frame, area: Rect) {
        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Authentication URL:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(&self.url, Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("Please authenticate in your browser and select the ", Style::default().fg(Color::Gray)),
                Span::styled(&self.tailnet_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(" tailnet.", Style::default().fg(Color::Gray)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Tailscale is running in the background waiting for authentication...", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
            ]),
        ];

        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("URL"));
        f.render_widget(paragraph, area);
    }

    fn render_instructions(&self, f: &mut Frame, area: Rect) {
        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Press ", Style::default().fg(Color::Gray)),
                Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(" to open browser  |  ", Style::default().fg(Color::Gray)),
                Span::styled("q", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(" to exit without opening", Style::default().fg(Color::Gray)),
            ]),
        ];

        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(paragraph, area);
    }
}
