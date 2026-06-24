use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::game::{Color, Game, Move, PieceTy, Pos, Square};

const CHECKMATE: i64 = i64::MAX;
const DRAW: i64 = -200;

const BASE_MOVE: Move = ((8, 8), (8, 8));

const PIECE_MULT: i64 = 20;
const PIECE_POS_MULT: i64 = 1;
const MOBILITY_MULT: i64 = 1;
const BISHOP_PAIRS: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheBound {
    Exact,
    Lower,
    Upper,
}

#[derive(Debug, Clone)]
pub struct Engine {
    cache: DashMap<u64, (i64, Move, u32, CacheBound)>,
    start_time: Instant,
    max_time: Duration,
}

impl Engine {
    pub fn new(max_time: Duration) -> Self {
        Self {
            cache: DashMap::new(),
            start_time: Instant::now(),
            max_time,
        }
    }

    pub fn best_move(&mut self, game: &Game) -> (i64, Move, u32) {
        let mut depth = 1;

        let mut best = self.eval_rec(game, 1, -CHECKMATE, CHECKMATE, 0);

        self.start_time = std::time::Instant::now();
        loop {
            /*
            eprintln!(
                "depth {} at {}ms",
                depth,
                self.start_time.elapsed().as_millis()
            );
            */

            if best.0 == CHECKMATE || self.timed_out() {
                break (best.0, best.1, depth);
            }
            depth += 1;

            let result = self.eval_rec(game, depth, -CHECKMATE, CHECKMATE, 0);

            if !self.timed_out() {
                best = result;
            } else {
                //eprintln!("timed out at {}ms", self.start_time.elapsed().as_millis());
                break (best.0, best.1, depth - 1);
            }

            if best.1 == BASE_MOVE {
                panic!("{:?} Best move is base", best);
            }
        }
    }

    fn timed_out(&self) -> bool {
        self.start_time.elapsed() >= self.max_time
    }

    fn eval_rec(
        &self,
        game: &Game,
        depth: u32,
        mut alpha: i64,
        mut beta: i64,
        moves: usize,
    ) -> (i64, Move) {
        if self.timed_out() {
            return (0, BASE_MOVE);
        }

        if !game.has_been_played(game)
            && let Some(entry) = self.cache.get(&game.get_hash())
            && entry.2 >= depth
        {
            match entry.3 {
                CacheBound::Exact => {
                    return (entry.0, entry.1);
                }
                CacheBound::Lower => {
                    alpha = alpha.max(entry.0);
                }
                CacheBound::Upper => {
                    beta = beta.min(entry.0);
                }
            }

            if alpha >= beta {
                return (entry.0, entry.1);
            }
        }

        if game.checkmate(game.get_to_move()) {
            return (-CHECKMATE, BASE_MOVE);
        }
        if game.stalemate(game.get_to_move()) || game.lose_on_repeat() {
            return (DRAW, BASE_MOVE);
        }

        if depth == 0 {
            return (self.quiescence(game, alpha, beta, moves), BASE_MOVE);
        }

        let original_alpha = alpha;

        let mut best: Option<(i64, Move)> = None;
        let p_moves = game.get_all_moves(game.get_to_move()).collect::<Vec<_>>();

        let mut scored: Vec<_> = p_moves
            .into_iter()
            .map(|m| {
                let g = game.clone().move_change(m);
                (-self.eval_base(&g, moves), (m, g))
            })
            .collect();

        scored.sort_by(|(a, _), (b, _)| b.cmp(a));

        for (m, g) in scored.into_iter().map(|(_, m)| m) {
            let score = -self.eval_rec(&g, depth - 1, -beta, -alpha, moves + 1).0;

            if self.timed_out() {
                return (0, BASE_MOVE);
            }

            if score >= beta {
                return (beta, m);
            }

            alpha = alpha.max(score);

            best = Some(match best {
                Some(prev) if prev.0 >= score => prev,
                _ => (score, m),
            });
        }
        let best = best.unwrap(); // cannot be none - not checkmate or stalemate

        self.cache.insert(
            game.get_hash(),
            (
                best.0,
                best.1,
                depth,
                if best.0 <= original_alpha {
                    CacheBound::Upper
                } else if best.0 >= beta {
                    CacheBound::Lower
                } else {
                    CacheBound::Exact
                },
            ),
        );
        best
    }

    pub fn eval_base(&self, game: &Game, moves: usize) -> i64 {
        if !game.has_been_played(game)
            && let Some((score, _, _, _)) = self.cache.get(&game.get_hash()).map(|v| *v.value())
        {
            return score;
        }

        if game.checkmate(game.get_to_move()) {
            return -CHECKMATE + moves as i64;
        }

        let basic_piece_score =
            Self::get_total_piece_score(game) * game.get_to_move().to_int() * PIECE_MULT;

        let piece_pos_values = game
            .get_all_pieces()
            .map(|(p, pos)| self.piece_pos(game, p, pos) * p.color().to_int())
            .sum::<i64>()
            * PIECE_POS_MULT;

        let mobility = (self.mobility(game, game.get_to_move())
            - self.mobility(game, game.get_to_move().other()))
            * MOBILITY_MULT;

        let bishop_counts = game
            .get_all_pieces()
            .filter(|(p, _)| p.ty() == PieceTy::Bishop)
            .fold([0i64; 2], |mut acc, (p, _)| {
                acc[p.color().to_index()] += 1;
                acc
            });

        let bishop_pairs =
            ((bishop_counts[0] >= 2) as i64 - (bishop_counts[1] >= 2) as i64) * BISHOP_PAIRS;

        // TODO: doubled pawns bad, backwards pawns bad
        // perhaps rework this function to just be only get score for our color, then have a super function that subtracts them from us

        basic_piece_score + piece_pos_values + mobility + bishop_pairs
    }

    fn mobility(&self, game: &Game, color: Color) -> i64 {
        game.get_all_moves(color)
            .map(|m| game.get(m.0))
            .map(|p| match p.ty() {
                PieceTy::Knight => 4,
                PieceTy::Bishop => 3,
                PieceTy::Rook => 2,
                PieceTy::Queen => 1,
                _ => 0,
            })
            .sum()
    }

    fn quiescence(&self, game: &Game, mut alpha: i64, beta: i64, moves: usize) -> i64 {
        let stand_pat = self.eval_base(game, moves);
        if stand_pat >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat);

        let mut scored: Vec<_> = game
            .get_all_moves(game.get_to_move())
            .filter(|&m| game.is_capture(m))
            .map(|m| {
                let g = game.clone().move_change(m);
                (-self.eval_base(&g, moves), (m, g))
            })
            .collect();

        scored.sort_by(|(a, _), (b, _)| b.cmp(a));

        for (_, (_, g)) in scored {
            let score = -self.quiescence(&g, -beta, -alpha, moves + 1);
            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            };
        }

        alpha
    }

    /// white is positive
    fn get_total_piece_score(game: &Game) -> i64 {
        game.get_all_pieces()
            .map(|p| Self::piece_value(p.0))
            .fold(0, |a, b| a + b)
    }

    /// white is positive
    fn piece_value(p: Square) -> i64 {
        (p.color().to_int())
            * match p.ty() {
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
            match piece.ty() {
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

    fn piece_pos(&self, game: &Game, piece: Square, pos: Pos) -> i64 {
        let table = match piece.ty() {
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

        let y = if piece.color() == Color::White {
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
