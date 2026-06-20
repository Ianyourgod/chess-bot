use std::time::Instant;

use dashmap::DashMap;
use rayon::prelude::*;

use std::sync::{
    Mutex,
    atomic::{AtomicI64, Ordering},
};

use crate::game::{Color, Game, Move, Piece, PieceTy, Pos};

const CHECKMATE: i64 = i64::MAX;

const MAX_TIME: std::time::Duration = std::time::Duration::from_millis(5000);

const BASE_MOVE: Move = ((8, 8), (8, 8));

#[derive(Debug, Clone)]
pub struct Engine {
    cache: DashMap<u64, (i64, Move, u32)>,
    start_time: Instant,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
            start_time: Instant::now(),
        }
    }

    pub fn best_move(&mut self, game: &Game, color: Color) -> (i64, Move, u32) {
        let mut depth = 1;

        let mut best = self.eval_rec(game, color, 1, -CHECKMATE, CHECKMATE, 0);

        self.start_time = std::time::Instant::now();
        loop {
            if best.0 == CHECKMATE || self.timed_out() {
                break (best.0, best.1, depth);
            }
            depth += 1;

            let result = self.eval_rec(game, color, depth, -CHECKMATE, CHECKMATE, 0);

            if !self.timed_out() {
                best = result;
            } else {
                break (best.0, best.1, depth - 1);
            }

            if best.1 == BASE_MOVE {
                panic!("{:?} Best move is base", best);
            }
        }
    }

    fn timed_out(&self) -> bool {
        self.start_time.elapsed() >= MAX_TIME
    }

    fn eval_rec(
        &self,
        game: &Game,
        color: Color,
        depth: u32,
        alpha: i64,
        beta: i64,
        moves: usize,
    ) -> (i64, Move) {
        if self.timed_out() {
            return (0, Move::default());
        }

        if !game.has_been_played(game)
            && let Some((score, mov, dep)) = self.cache.get(&game.get_hash()).map(|v| *v.value())
            && dep >= depth
        {
            return (score, mov);
        }

        if game.checkmate(color) {
            return (-CHECKMATE, BASE_MOVE);
        }
        if game.stalemate(color) || game.lose_on_repeat() {
            return (0, BASE_MOVE);
        }

        if depth == 0 {
            return (self.quiescence(game, color, alpha, beta, moves), BASE_MOVE);
        }

        let best = Mutex::new(Option::<(i64, Move)>::None);

        /*
        let mut moves = game.get_all_moves(color).collect::<Vec<_>>();
        moves.sort_by(|a, b| {
            let b = -self.eval_base(&game.clone().move_change(*b), color.other());
            let a = -self.eval_base(&game.clone().move_change(*a), color.other());
            b.cmp(&a)
        });
        */

        let alpha = AtomicI64::new(alpha);

        if let Some(r) = game.get_all_moves(color).find_map_any(|m| {
            let score = -self
                .eval_rec(
                    &game.clone().move_change(m),
                    color.other(),
                    depth - 1,
                    -beta,
                    -alpha.load(Ordering::Relaxed),
                    moves + 1,
                )
                .0;

            if self.timed_out() {
                return Some((0, Move::default()));
            }

            if score >= beta {
                return Some((beta, m));
            }

            alpha.fetch_max(score, Ordering::Relaxed);

            let mut b = best.lock().unwrap();
            *b = Some(match *b {
                Some(prev) if prev.0 >= score => prev,
                _ => (score, m),
            });

            None
        }) {
            return r;
        }

        //crate::display::display(game.clone());

        let best = best.into_inner().unwrap().unwrap(); // cannot be none - not checkmate or stalemate
        self.cache.insert(game.get_hash(), (best.0, best.1, depth));
        best
    }

    fn eval_base(&self, game: &Game, color: Color, moves: usize) -> i64 {
        if !game.has_been_played(game)
            && let Some((score, _, _)) = self.cache.get(&game.get_hash()).map(|v| *v.value())
        {
            return score;
        }

        if game.checkmate(color) {
            return -CHECKMATE + moves as i64;
        }

        const PIECE_MULT: i64 = 3;
        const PIECE_POS_MULT: i64 = 1;

        let basic_piece_score =
            Self::get_total_piece_score(game) * color.to_int() as i64 * PIECE_MULT;

        let piece_pos_values = game
            .get_all_pieces()
            .map(|(p, pos)| self.piece_pos(game, p, pos) * p.color.to_int() as i64)
            .sum::<i64>()
            * PIECE_POS_MULT;

        basic_piece_score + piece_pos_values
    }

    fn quiescence(
        &self,
        game: &Game,
        color: Color,
        mut alpha: i64,
        beta: i64,
        moves: usize,
    ) -> i64 {
        let stand_pat = self.eval_base(game, color, moves);
        if stand_pat >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat);

        let alpha = AtomicI64::new(alpha);

        if let Some(s) = game
            .get_all_moves(color)
            .filter(|&m| game.is_capture(m))
            .find_map_any(|m| {
                let score = -self.quiescence(
                    &game.clone().move_change(m),
                    color.other(),
                    -beta,
                    -alpha.load(Ordering::Relaxed),
                    moves + 1,
                );
                if score >= beta {
                    return Some(beta);
                }
                alpha.fetch_max(score, Ordering::Relaxed);

                None
            })
        {
            return s;
        }

        alpha.into_inner()
    }

    /// white is positive
    fn get_total_piece_score(game: &Game) -> i64 {
        game.get_board()
            .iter()
            .flat_map(|r| r.iter().filter_map(|p| p.map(|p| Self::piece_value(p))))
            .fold(0, |a, b| a + b)
    }

    /// white is positive
    fn piece_value(p: Piece) -> i64 {
        (p.color.to_int() as i64)
            * match p.ty {
                PieceTy::Pawn => 1,
                PieceTy::Bishop | PieceTy::Knight => 3,
                PieceTy::Rook => 5,
                PieceTy::Queen => 9,
                PieceTy::King => 0,
            }
    }

    fn get_game_stage(&self, game: &Game) -> GameStage {
        let mut queens: u8 = 0;
        let mut major_minor: u8 = 0;

        for (piece, _) in game.get_all_pieces() {
            match piece.ty {
                PieceTy::Queen => {
                    queens += 1;
                    major_minor += 1;
                }
                PieceTy::Rook | PieceTy::Bishop | PieceTy::Knight => {
                    major_minor += 1;
                }
                _ => {}
            }
        }

        match (major_minor, queens) {
            (22.., _) => GameStage::Early,
            (_, 2..) => GameStage::Mid,
            _ => GameStage::End,
        }
    }

    fn piece_pos(&self, game: &Game, piece: Piece, pos: Pos) -> i64 {
        let table = match piece.ty {
            PieceTy::Pawn => [
                [0, 0, 0, 0, 0, 0, 0, 0],
                [50, 50, 50, 50, 50, 50, 50, 50],
                [10, 10, 20, 30, 30, 20, 10, 10],
                [5, 5, 10, 25, 25, 10, 5, 5],
                [0, 0, 0, 20, 20, 0, 0, 0],
                [5, -5, -10, 0, 0, -10, -5, 5],
                [5, 10, 10, -20, -20, 10, 10, 5],
                [0, 0, 0, 0, 0, 0, 0, 0],
            ],
            PieceTy::Knight => [
                [-50, -40, -30, -30, -30, -30, -40, -50],
                [-40, -20, 0, 0, 0, 0, -20, -40],
                [-30, 0, 10, 15, 15, 10, 0, -30],
                [-30, 5, 15, 20, 20, 15, 5, -30],
                [-30, 0, 15, 20, 20, 15, 0, -30],
                [-30, 5, 10, 15, 15, 10, 5, -30],
                [-40, -20, 0, 5, 5, 0, -20, -40],
                [-50, -40, -30, -30, -30, -30, -40, -50],
            ],
            PieceTy::Bishop => [
                [-20, -10, -10, -10, -10, -10, -10, -20],
                [-10, 0, 0, 0, 0, 0, 0, -10],
                [-10, 0, 5, 10, 10, 5, 0, -10],
                [-10, 5, 5, 10, 10, 5, 5, -10],
                [-10, 0, 10, 10, 10, 10, 0, -10],
                [-10, 10, 10, 10, 10, 10, 10, -10],
                [-10, 5, 0, 0, 0, 0, 5, -10],
                [-20, -10, -10, -10, -10, -10, -10, -20],
            ],
            PieceTy::Rook => [
                [0, 0, 0, 0, 0, 0, 0, 0],
                [5, 10, 10, 10, 10, 10, 10, 5],
                [-5, 0, 0, 0, 0, 0, 0, -5],
                [-5, 0, 0, 0, 0, 0, 0, -5],
                [-5, 0, 0, 0, 0, 0, 0, -5],
                [-5, 0, 0, 0, 0, 0, 0, -5],
                [-5, 0, 0, 0, 0, 0, 0, -5],
                [0, 0, 0, 5, 5, 0, 0, 0],
            ],
            PieceTy::Queen => [
                [-20, -10, -10, -5, -5, -10, -10, -20],
                [-10, 0, 0, 0, 0, 0, 0, -10],
                [-10, 0, 5, 5, 5, 5, 0, -10],
                [-5, 0, 5, 5, 5, 5, 0, -5],
                [0, 0, 5, 5, 5, 5, 0, -5],
                [-10, 5, 5, 5, 5, 5, 0, -10],
                [-10, 0, 5, 0, 0, 0, 0, -10],
                [-20, -10, -10, -5, -5, -10, -10, -20],
            ],
            PieceTy::King => match self.get_game_stage(game) {
                GameStage::Early | GameStage::Mid => [
                    [-30, -40, -40, -50, -50, -40, -40, -30],
                    [-30, -40, -40, -50, -50, -40, -40, -30],
                    [-30, -40, -40, -50, -50, -40, -40, -30],
                    [-30, -40, -40, -50, -50, -40, -40, -30],
                    [-20, -30, -30, -40, -40, -30, -30, -20],
                    [-10, -20, -20, -20, -20, -20, -20, -10],
                    [20, 20, 0, 0, 0, 0, 20, 20],
                    [20, 30, 10, 0, 0, 10, 30, 20],
                ],
                GameStage::End => [
                    [-50, -40, -30, -20, -20, -30, -40, -50],
                    [-30, -20, -10, 0, 0, -10, -20, -30],
                    [-30, -10, 20, 30, 30, 20, -10, -30],
                    [-30, -10, 30, 40, 40, 30, -10, -30],
                    [-30, -10, 30, 40, 40, 30, -10, -30],
                    [-30, -10, 20, 30, 30, 20, -10, -30],
                    [-30, -30, 0, 0, 0, 0, -30, -30],
                    [-50, -30, -30, -30, -30, -30, -30, -50],
                ],
            },
        };

        let y = if piece.color == Color::White {
            7 - pos.1
        } else {
            pos.1
        };

        table[y][pos.0] / 5
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GameStage {
    Early,
    Mid,
    End,
}
