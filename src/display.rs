use std::time::Duration;

use crate::{
    eval_engine::Engine,
    game::{Game, Piece, PieceTy},
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

pub fn run(game: Game) {
    let mut app = App::new(game);
    let run = |t: &mut DefaultTerminal| app.run(t);
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
            engine: Engine::new(),
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

    fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let mut depth = 0;
        loop {
            let render = |frame: &mut Frame| self.render(frame, depth);
            terminal.draw(render)?;
            std::thread::sleep(Duration::from_secs(2));
            if !self.board.checkmate(self.board.get_to_move()) {
                let engine_move = self.engine.best_move(&self.board, self.board.get_to_move());
                self.board.move_piece(engine_move.1);
                self.board.swap_to_move();
                depth = engine_move.2;
            }
            if crossterm::event::poll(Duration::from_millis(100))? {
                if let Some(kp) = crossterm::event::read()?.as_key_press_event() {
                    match kp.code {
                        KeyCode::Esc | KeyCode::Char('q') => break Ok(()),
                        /*_ if !self.board.checkmate(self.color) => {
                            let engine_move = self.engine.best_move(&self.board, self.color);
                            self.board.move_piece(engine_move.1);
                            self.color = self.color.other();
                        }*/
                        _ => (),
                    }
                }
            }
        }
    }

    fn display(&self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            let render = |frame: &mut Frame| self.render(frame, 0);
            terminal.draw(render)?;
            if crossterm::event::read()?.is_key_press() {
                break Ok(());
            }
        }
    }

    fn render(&self, frame: &mut Frame, d: u32) {
        let area = frame.area();

        let mut board_spans = Vec::with_capacity(8);
        for y in (0..8).rev() {
            let mut row_spans = Vec::with_capacity(8);

            let rank = self.board.get_rank(y);
            for (x, place) in rank.iter().enumerate() {
                let is_dark = (x ^ y) % 2 == 0;
                let style = if is_dark {
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
