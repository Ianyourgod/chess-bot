use std::time::{Duration, Instant};

use crate::game::{Color, Game, Move, PieceTy, Pos, Square};

mod cache;

#[allow(unused)]
use cache::{Cache, CacheBound, CacheTrait, DashCache};

const CHECKMATE: i32 = i32::MAX;
const DRAW: i32 = -200;

const BASE_MOVE: Move = Move {
    from: (8, 8),
    to: (8, 8),
    promotion: None,
};

const PIECE_MULT: i32 = 32;
const PIECE_POS_MULT: i32 = 2;
#[allow(unused)]
const MOBILITY_MULT: i32 = 2;
const BISHOP_PAIRS: i32 = 8;
const DOUBLED_PAWNS: i32 = -4;
const TO_MOVE_BONUS: i32 = 4;

const MAX_EXTENSIONS: u16 = 16;
const NULL_MOVE_REDUX: u16 = 3;

#[derive(Debug, Clone, PartialEq)]
pub enum CalcConstraint {
    Time(Duration),
    Depth(u16),
}

type CurrentCache = DashCache;

#[derive(Debug, Clone)]
pub struct Engine {
    cache: CurrentCache,
    start_time: Instant,
    constraint: CalcConstraint,
}

impl Engine {
    pub fn new(constraint: CalcConstraint) -> Self {
        Self {
            cache: CurrentCache::cache_new(),
            start_time: Instant::now(),
            constraint,
        }
    }

    pub fn set_think_time(&mut self, inc: Duration, init: Duration) {
        const MAYBE_MOVES: u32 = 60;
        let time_per = (init / MAYBE_MOVES) + inc - Duration::from_millis(300);
        self.constraint = CalcConstraint::Time(time_per)
    }

    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn best_move(&mut self, game: &mut Game) -> (i32, Move, u16) {
        let mut depth = 1;

        let mut best = self.eval_rec_base(game, depth);

        self.start_time = std::time::Instant::now();

        loop {
            // TODO: figure out a way to only log when we're not doing ratatui stuff
            /*
            println!(
                "depth {} at {}ms",
                depth,
                self.start_time.elapsed().as_millis()
            );
            */

            // -50 so that our move thing (prioritize late/early checkmates) still works with this
            if best.0 >= (CHECKMATE - 50) || self.timed_out(depth) {
                break (best.0, best.1, depth);
            }
            depth += 1;

            let s = self.eval_rec_base(game, depth);

            if self.timed_out(depth) {
                break (best.0, best.1, depth - 1);
            }

            best = s;

            assert_ne!(best.1, BASE_MOVE)
        }
    }

    fn timed_out(&self, depth: u16) -> bool {
        match self.constraint {
            CalcConstraint::Time(max) => self.start_time.elapsed() >= max,
            CalcConstraint::Depth(d) => depth > d,
        }
    }

    fn extension_depth(game: &Game, depth: u16, m: Move, extensions_left: u16) -> u16 {
        if depth <= 3
            && extensions_left > 0
            && (game.check(game.get_to_move()) || m.promotion.is_some())
        {
            1
        } else {
            0
        }
    }

    fn eval_rec_base(&mut self, game: &mut Game, depth: u16) -> (i32, Move) {
        let mut best = (i32::MIN, BASE_MOVE);
        let p_moves = game.get_all_moves(game.get_to_move());

        if self.timed_out(depth) {
            return (0, BASE_MOVE);
        }

        let mut scored: Vec<(i32, Move)> = p_moves
            .into_iter()
            .map(|m| (self.move_order_score(game, m), m))
            .collect();
        scored.sort_unstable_by(|(a, _), (b, _)| b.cmp(a));

        for (_, m) in scored {
            game.move_piece(m);
            let score = -self.eval_rec(game, depth - 1, -CHECKMATE, CHECKMATE, 0, MAX_EXTENSIONS);
            game.undo_move();

            if self.timed_out(depth) {
                return best;
            }

            best = if best.0 >= score { best } else { (score, m) };
        }

        best
    }

    fn eval_rec(
        &mut self,
        game: &mut Game,
        depth: u16,
        mut alpha: i32,
        mut beta: i32,
        moves: usize,
        extensions_left: u16,
    ) -> i32 {
        if self.timed_out(depth) {
            return 0;
        }

        if !game.has_been_played(game)
            && let Some(entry) = self.cache.cache_get(game.get_hash())
            && entry.1 >= depth
        {
            match entry.2 {
                CacheBound::Exact => {
                    return entry.0;
                }
                CacheBound::Lower => {
                    alpha = alpha.max(entry.0);
                }
                CacheBound::Upper => {
                    beta = beta.min(entry.0);
                }
            }

            if alpha >= beta {
                return entry.0;
            }
        }

        if game.checkmate(game.get_to_move()) {
            return -CHECKMATE;
        }
        if game.stalemate(game.get_to_move()) || game.lose_on_repeat() {
            return DRAW;
        }

        if depth == 0 {
            return self.quiescence(game, alpha, beta, moves);
        }

        let nm_redux = NULL_MOVE_REDUX;
        if depth >= nm_redux && !game.pawn_endgame() && !game.check(game.get_to_move()) {
            game.make_null_move();
            let score = -self.eval_rec(game, depth - nm_redux, -beta, -(beta - 1), moves, 0);
            game.undo_move();
            if score >= beta {
                return score;
            }
        }

        let original_alpha = alpha;

        let mut best = -i32::MAX;
        let mut p_moves = game.get_all_moves(game.get_to_move());

        p_moves.sort_unstable_by(|a, b| {
            self.move_order_score(game, *b)
                .cmp(&self.move_order_score(game, *a))
        });

        for (i, m) in p_moves.into_iter().enumerate() {
            game.move_piece(m);

            let extension = Self::extension_depth(game, depth, m, extensions_left);
            let reduction = if i >= 3
                && depth >= 3
                && !game.is_capture(m)
                && m.promotion.is_none()
                && !game.check(game.get_to_move())
            {
                1
            } else {
                0
            };

            let score = -self.eval_rec(
                game,
                depth - 1 + extension - reduction,
                -beta,
                -alpha,
                moves + 1,
                extensions_left - extension,
            );
            game.undo_move();

            if self.timed_out(depth) {
                return 0;
            }

            if score >= beta {
                return beta;
            }

            alpha = alpha.max(score);

            best = best.max(score);
        }

        self.cache.insert(
            game.get_hash(),
            (
                best,
                depth,
                if best <= original_alpha {
                    CacheBound::Upper
                } else if best >= beta {
                    CacheBound::Lower
                } else {
                    CacheBound::Exact
                },
            ),
        );
        best
    }

    fn move_order_score(&self, game: &mut Game, m: Move) -> i32 {
        let moving = game.get(m.from);

        if let Some(en_pass) = game.is_capture_known_move(m, moving) {
            let captured = game.get(m.to);
            let victim = if en_pass {
                Self::piece_value_raw(PieceTy::Pawn)
            } else {
                Self::piece_value_raw(captured.ty())
            };
            let attacker = Self::piece_value_raw(moving.ty());
            return 10_000 + victim * 10 - attacker;
        }

        if m.promotion.is_some() {
            return 9_000;
        }

        let hash = game.hash_after_move(m, moving);
        if let Some(entry) = self.cache.cache_get(hash) {
            return 1_000 - entry.0.clamp(-999, 999);
        }

        0
    }

    fn eval_base(&mut self, game: &Game, moves: usize) -> i32 {
        if !game.has_been_played(game)
            && let Some((score, _, CacheBound::Exact)) = self.cache.cache_get(game.get_hash())
        {
            return score;
        }

        if game.checkmate(game.get_to_move()) {
            return -CHECKMATE + moves as i32;
        }

        let player = self.eval_base_color(game, game.get_to_move()) + TO_MOVE_BONUS;
        let enemy = self.eval_base_color(game, game.get_to_move().other());

        player - enemy
    }

    fn eval_base_color(&self, game: &Game, c: Color) -> i32 {
        let basic_piece_score = Self::get_piece_score_color(game, c) * PIECE_MULT;

        let stage = Self::get_game_stage(game);
        let piece_pos_values = game
            .get_all_pieces_color(c)
            .map(|(p, pos)| self.piece_pos(stage, p, pos))
            .sum::<i32>()
            * PIECE_POS_MULT;

        // TODO: reenable this once we make mobility better (cheaper)
        let mobility = 0; /*self.mobility(game, game.get_to_move())
         * MOBILITY_MULT;*/

        let bishop_count = game.get_all_pieces_ty_color(PieceTy::Bishop, c).count();

        let bishop_pairs = (bishop_count >= 2) as i32 * BISHOP_PAIRS;

        let doubled_pawns = game.doubled_pawns_check(c) * DOUBLED_PAWNS;

        // TODO: passed pawns

        basic_piece_score + piece_pos_values + mobility + bishop_pairs + doubled_pawns
    }

    #[allow(unused)]
    fn mobility(&self, game: &Game, color: Color) -> i32 {
        // TODO: use get_all_pseudo moves somehow

        game.get_all_moves(color)
            .into_iter()
            .map(|m| game.get(m.from))
            .map(|p| match p.ty() {
                PieceTy::Knight => 4,
                PieceTy::Bishop => 3,
                PieceTy::Rook => 2,
                PieceTy::Queen => 1,
                _ => 0,
            })
            .sum()
    }

    /// only use when you know the move is a capture. this is to account for en pass. if you know its not, just directly get the .to
    #[inline]
    fn captured_square(game: &Game, m: Move) -> Square {
        let target = game.get(m.to);
        if !target.is_empty() {
            return target;
        }
        // en pass
        game.get((m.to.0, m.from.1))
    }

    fn quiescence(&mut self, game: &mut Game, mut alpha: i32, beta: i32, moves: usize) -> i32 {
        let stand_pat = self.eval_base(game, moves);
        if stand_pat >= beta {
            return beta;
        }

        const DELTA_MARGIN: i32 = Engine::piece_value_raw(PieceTy::Queen) * PIECE_MULT;
        if stand_pat + DELTA_MARGIN < alpha {
            return alpha;
        }

        alpha = alpha.max(stand_pat);

        // TODO: maybe create get_all_captures
        let mut scored: Vec<_> = game
            .get_all_moves(game.get_to_move())
            .into_iter()
            .filter(|&m| game.is_capture(m))
            // TODO: this is probably too restrictive
            .filter(|&m| {
                let victim = Self::piece_value_raw(Self::captured_square(game, m).ty());
                let attacker = Self::piece_value_raw(game.get(m.from).ty());
                victim >= attacker - 1
            })
            .map(|m| {
                let victim = Self::piece_value_raw(Self::captured_square(game, m).ty());
                let attacker = Self::piece_value_raw(game.get(m.from).ty());
                (victim * 10 - attacker, m)
            })
            .collect();

        scored.sort_unstable_by(|(a, _), (b, _)| b.cmp(a));

        for (_, m) in scored {
            let gain = Self::piece_value_raw(Self::captured_square(game, m).ty()) * PIECE_MULT;
            if stand_pat + gain + DELTA_MARGIN < alpha {
                continue;
            }

            game.move_piece(m);
            let score = -self.quiescence(game, -beta, -alpha, moves + 1);
            game.undo_move();

            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }

        alpha
    }

    fn get_piece_score_color(game: &Game, c: Color) -> i32 {
        game.get_all_pieces_color(c)
            .map(|p| Self::piece_value_raw(p.0.ty()))
            .fold(0, |a, b| a + b)
    }

    /*
    // this is somehow slower than the top one
    fn get_total_piece_score(game: &Game) -> i32 {
        let p = |t| {
            (game.piece_count(Square::piece(t, Color::White))
                - game.piece_count(Square::piece(t, Color::Black))) as i32
        };

        [
            PieceTy::Pawn,
            PieceTy::Bishop,
            PieceTy::Knight,
            PieceTy::Rook,
            PieceTy::Queen,
            PieceTy::King,
        ]
        .into_iter()
        .map(|ty| Self::piece_value_raw(ty) * p(ty))
        .sum()
    }
    */

    #[inline]
    const fn piece_value_raw(t: PieceTy) -> i32 {
        match t {
            PieceTy::Pawn => 1,
            PieceTy::Bishop | PieceTy::Knight => 3,
            PieceTy::Rook => 5,
            PieceTy::Queen => 9,
            PieceTy::King => 0,
        }
    }

    fn get_game_stage(game: &Game) -> GameStage {
        let p = |t| {
            game.piece_count(Square::piece(t, Color::White))
                + game.piece_count(Square::piece(t, Color::Black))
        };

        let queens = p(PieceTy::Queen);
        let major_minor = p(PieceTy::Rook) + p(PieceTy::Bishop) + p(PieceTy::Knight);

        match (major_minor, queens) {
            (22.., _) => GameStage::Early,
            (_, 2..) => GameStage::Mid,
            _ => GameStage::End,
        }
    }

    #[inline]
    fn piece_pos(&self, stage: GameStage, piece: Square, pos: Pos) -> i32 {
        let table = match piece.ty() {
            PieceTy::Pawn => &PAWN_TABLE,
            PieceTy::Knight => &KNIGHT_TABLE,
            PieceTy::Bishop => &BISHOP_TABLE,
            PieceTy::Rook => &ROOK_TABLE,
            PieceTy::Queen => &QUEEN_TABLE,
            PieceTy::King => match stage {
                GameStage::Early | GameStage::Mid => &KING_EARLY_TABLE,
                GameStage::End => &KING_END_TABLE,
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

// TODO: add more end game tables

const PAWN_TABLE: [[i32; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [50, 50, 50, 50, 50, 50, 50, 50],
    [10, 10, 20, 30, 30, 20, 10, 10],
    [5, 5, 10, 25, 25, 10, 5, 5],
    [0, 0, 0, 20, 20, 0, 0, 0],
    [5, -5, -10, 0, 0, -10, -5, 5],
    [5, 10, 10, -20, -20, 10, 10, 5],
    [0, 0, 0, 0, 0, 0, 0, 0],
];

const KNIGHT_TABLE: [[i32; 8]; 8] = [
    [-50, -40, -30, -30, -30, -30, -40, -50],
    [-40, -20, 0, 0, 0, 0, -20, -40],
    [-30, 0, 10, 15, 15, 10, 0, -30],
    [-30, 5, 15, 20, 20, 15, 5, -30],
    [-30, 0, 15, 20, 20, 15, 0, -30],
    [-30, 5, 10, 15, 15, 10, 5, -30],
    [-40, -20, 0, 5, 5, 0, -20, -40],
    [-50, -40, -30, -30, -30, -30, -40, -50],
];

const BISHOP_TABLE: [[i32; 8]; 8] = [
    [-20, -10, -10, -10, -10, -10, -10, -20],
    [-10, 0, 0, 0, 0, 0, 0, -10],
    [-10, 0, 5, 10, 10, 5, 0, -10],
    [-10, 5, 5, 10, 10, 5, 5, -10],
    [-10, 0, 10, 10, 10, 10, 0, -10],
    [-10, 10, 10, 10, 10, 10, 10, -10],
    [-10, 5, 0, 0, 0, 0, 5, -10],
    [-20, -10, -10, -10, -10, -10, -10, -20],
];

const ROOK_TABLE: [[i32; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [5, 10, 10, 10, 10, 10, 10, 5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [0, 0, 0, 5, 5, 0, 0, 0],
];

const QUEEN_TABLE: [[i32; 8]; 8] = [
    [-20, -10, -10, -5, -5, -10, -10, -20],
    [-10, 0, 0, 0, 0, 0, 0, -10],
    [-10, 0, 5, 5, 5, 5, 0, -10],
    [-5, 0, 5, 5, 5, 5, 0, -5],
    [0, 0, 5, 5, 5, 5, 0, -5],
    [-10, 5, 5, 5, 5, 5, 0, -10],
    [-10, 0, 5, 0, 0, 0, 0, -10],
    [-20, -10, -10, -5, -5, -10, -10, -20],
];

const KING_EARLY_TABLE: [[i32; 8]; 8] = [
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-20, -30, -30, -40, -40, -30, -30, -20],
    [-10, -20, -20, -20, -20, -20, -20, -10],
    [20, 20, 0, 0, 0, 0, 20, 20],
    [20, 30, 10, 0, 0, 10, 30, 20],
];

const KING_END_TABLE: [[i32; 8]; 8] = [
    [-50, -40, -30, -20, -20, -30, -40, -50],
    [-30, -20, -10, 0, 0, -10, -20, -30],
    [-30, -10, 20, 30, 30, 20, -10, -30],
    [-30, -10, 30, 40, 40, 30, -10, -30],
    [-30, -10, 30, 40, 40, 30, -10, -30],
    [-30, -10, 20, 30, 30, 20, -10, -30],
    [-30, -30, 0, 0, 0, 0, -30, -30],
    [-50, -30, -30, -30, -30, -30, -30, -50],
];
