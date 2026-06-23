use std::time::Duration;

use crate::{
    eval_engine::Engine,
    game::{Color, Game, Piece, PieceTy},
};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::{self, event::KeyCode},
    style::{Color as RColor, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

#[allow(unused)]
pub fn display(game: Game) {
    let app = App::new(game);
    let run = |t: &mut DefaultTerminal| app.display(t);
    ratatui::run(run).unwrap();
}

pub fn bot_on_bot(game: Game) {
    let mut app = App::new(game);
    let run = |t: &mut DefaultTerminal| app.botbot(t);
    ratatui::run(run).unwrap();
}

pub fn player_vs_bot(game: Game, bot_color: Color) {
    let mut app = App::new(game);
    let run = |t: &mut DefaultTerminal| app.player_bot(t, bot_color);
    ratatui::run(run).unwrap();
}

struct App {
    board: Game,
    engine: Engine,
}

impl App {
    pub fn new(board: Game) -> Self {
        Self {
            board,
            engine: Engine::new(std::time::Duration::from_millis(5000)),
        }
    }

    fn get_place_char(piece: Option<Piece>) -> char {
        let Some(piece) = piece else {
            return ' ';
        };

        match piece.ty {
            PieceTy::Pawn => '♟',
            PieceTy::Rook => '♜',
            PieceTy::Knight => '♞',
            PieceTy::Bishop => '♝',
            PieceTy::Queen => '♛',
            PieceTy::King => '♚',
        }
    }

    fn botbot(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let mut depth = 0;
        loop {
            let render = |frame: &mut Frame| self.render(frame, depth, None);
            terminal.draw(render)?;
            if !self.board.checkmate(self.board.get_to_move())
                && !self.board.stalemate(self.board.get_to_move())
            {
                let engine_move = self.engine.best_move(&self.board);
                self.board.move_piece(engine_move.1);
                depth = engine_move.2;
            }
            if crossterm::event::poll(Duration::from_millis(100))? {
                if let Some(kp) = crossterm::event::read()?.as_key_press_event() {
                    match kp.code {
                        KeyCode::Esc | KeyCode::Char('q') => break Ok(()),
                        /*_ if !self.board.checkmate(self.color) => {
                            let engine_move = self.engine.best_move(&self.board, self.color);
                            self.board.move_piece(engine_move.1);
                        }*/
                        _ => (),
                    }
                }
            }
        }
    }

    fn player_bot(
        &mut self,
        terminal: &mut DefaultTerminal,
        bot_color: Color,
    ) -> std::io::Result<()> {
        let mut depth = 0;
        let mut cursor_pos = (0, 0);
        let mut selected = None;

        loop {
            let render =
                |frame: &mut Frame| self.render(frame, depth, Some((cursor_pos, selected)));
            terminal.draw(render)?;

            if !self.board.checkmate(self.board.get_to_move())
                && !self.board.stalemate(self.board.get_to_move())
                && self.board.get_to_move() == bot_color
            {
                let engine_move = self.engine.best_move(&self.board);
                self.board.move_piece(engine_move.1);
                depth = engine_move.2;
            }

            let correct_pos = |p: (isize, isize)| {
                let s = |n: isize| {
                    if n < 0 {
                        -n
                    } else if n > 7 {
                        n - 8
                    } else {
                        n
                    }
                };
                (s(p.0), s(p.1))
            };

            if crossterm::event::poll(Duration::from_millis(10))? {
                if let Some(kp) = crossterm::event::read()?.as_key_press_event() {
                    match kp.code {
                        KeyCode::Esc | KeyCode::Char('q') => break Ok(()),
                        _ if self.board.checkmate(self.board.get_to_move())
                            || self.board.stalemate(self.board.get_to_move())
                            || self.board.get_to_move() == bot_color =>
                        {
                            ()
                        }
                        KeyCode::Enter => {
                            if selected.is_none() {
                                selected = Some(cursor_pos);
                            } else if let Some(s) = selected {
                                selected = None;

                                let m = (
                                    (s.0 as usize, s.1 as usize),
                                    (cursor_pos.0 as usize, cursor_pos.1 as usize),
                                );
                                if s != cursor_pos && self.board.is_valid(m) {
                                    self.board.move_piece(m);
                                }
                            }
                        }
                        KeyCode::Up => cursor_pos = correct_pos((cursor_pos.0, cursor_pos.1 + 1)),
                        KeyCode::Down => cursor_pos = correct_pos((cursor_pos.0, cursor_pos.1 - 1)),
                        KeyCode::Left => cursor_pos = correct_pos((cursor_pos.0 - 1, cursor_pos.1)),
                        KeyCode::Right => {
                            cursor_pos = correct_pos((cursor_pos.0 + 1, cursor_pos.1))
                        }

                        _ => (),
                    }
                }
            }
        }
    }

    fn display(&self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            let render = |frame: &mut Frame| self.render(frame, 0, None);
            terminal.draw(render)?;
            if crossterm::event::read()?.is_key_press() {
                break Ok(());
            }
        }
    }

    fn render(
        &self,
        frame: &mut Frame,
        d: u32,
        player_info: Option<((isize, isize), Option<(isize, isize)>)>,
    ) {
        let area = frame.area();

        let mut board_spans = Vec::with_capacity(8);
        for y in (0..8).rev() {
            let mut row_spans = Vec::with_capacity(8);

            let rank = self.board.get_rank(y);
            for (x, place) in rank.iter().enumerate() {
                let is_dark = (x ^ y) % 2 == 0;
                let style = if let Some((cursor_pos, _)) = player_info
                    && cursor_pos == (x as isize, y as isize)
                {
                    Style::default().bg(RColor::LightYellow)
                } else if let Some((_, selected)) = player_info
                    && selected == Some((x as isize, y as isize))
                {
                    Style::default().bg(RColor::LightRed)
                } else if is_dark {
                    Style::default().bg(RColor::Rgb(140, 86, 53))
                } else {
                    Style::default().bg(RColor::Rgb(197, 149, 98))
                };

                let fg_white = 1 == place.map(|p| p.color.to_int()).unwrap_or(0);
                let style = if fg_white {
                    style.fg(RColor::Rgb(255, 255, 255))
                } else {
                    style.fg(RColor::Rgb(0, 0, 0))
                };

                row_spans.push(Span::styled(
                    format!(" {} ", Self::get_place_char(*place)),
                    style,
                ))
            }

            board_spans.push(Line::from(row_spans));
        }

        let board_widget = Paragraph::new(board_spans).block(
            Block::default()
                .title(format!("chian (d={d})"))
                .borders(Borders::ALL),
        );

        frame.render_widget(board_widget, area);
    }
}
