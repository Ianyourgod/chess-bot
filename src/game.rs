pub mod magic_bb;
mod zobrists;

use zobrists::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Square(u8);

impl Square {
    pub const EMPTY: Square = Square(0);

    #[inline]
    pub fn piece(ty: PieceTy, color: Color) -> Square {
        Square((ty as u8) | ((color as u8) << 3))
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn color(self) -> Color {
        Color::from_u8(self.0 >> 3)
    }

    #[inline]
    pub fn ty(self) -> PieceTy {
        PieceTy::from_u8(self.0 & 0b111)
    }

    #[inline]
    pub fn to_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PieceTy {
    Pawn = 1,
    Knight = 2,
    Bishop = 3,
    Rook = 4,
    Queen = 5,
    King = 6,
}

const BB_PAWN: usize = 0;
const BB_KNIGHT: usize = 1;
const BB_BISHOP: usize = 2;
const BB_ROOK: usize = 3;
const BB_QUEEN: usize = 4;
const BB_KING: usize = 5;

impl PieceTy {
    #[inline]
    pub fn from_u8(n: u8) -> Self {
        match n {
            1 => Self::Pawn,
            2 => Self::Knight,
            3 => Self::Bishop,
            4 => Self::Rook,
            5 => Self::Queen,
            6 => Self::King,
            n => unreachable!("found {n}"),
        }
    }

    #[inline]
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

impl Color {
    #[inline]
    pub fn from_u8(n: u8) -> Self {
        match n {
            1 => Color::Black,
            0 => Color::White,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    pub fn to_int(self) -> i32 {
        match self {
            Self::White => 1,
            Self::Black => -1,
        }
    }

    #[inline]
    pub const fn to_index(&self) -> usize {
        match self {
            Self::White => 0,
            Self::Black => 1,
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    #[inline]
    pub fn start(self) -> usize {
        match self {
            Self::White => 0,
            Self::Black => 7,
        }
    }
}

// TODO: using u8 would be a lot nicer memory wise (and memory is time)
pub type Pos = (usize, usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move {
    pub from: Pos,
    pub to: Pos,
    pub promotion: Option<PieceTy>,
}

impl Move {
    pub fn from_str(m: &str) -> Self {
        let (from, to_ish) = m.split_at(2);
        let (to, promo) = to_ish.split_at(2);
        let promo = (!promo.is_empty()).then(|| match promo {
            "q" => PieceTy::Queen,
            "r" => PieceTy::Rook,
            "n" => PieceTy::Knight,
            "b" => PieceTy::Bishop,
            _ => panic!("thats not a valid promotion"),
        });
        let parse_pos = |p: &str| {
            let (row, col) = p.split_at(1);
            let x = match row {
                "a" => 0,
                "b" => 1,
                "c" => 2,
                "d" => 3,
                "e" => 4,
                "f" => 5,
                "g" => 6,
                "h" => 7,
                _ => panic!("invalid FEN string"),
            };
            let y = col.parse::<usize>().unwrap() - 1;
            (x, y)
        };
        Move {
            from: parse_pos(from),
            to: parse_pos(to),
            promotion: promo,
        }
    }

    pub fn to_string(self) -> String {
        let stringify_pos = |p: Pos| {
            let row = match p.0 {
                0 => "a",
                1 => "b",
                2 => "c",
                3 => "d",
                4 => "e",
                5 => "f",
                6 => "g",
                7 => "h",
                _ => panic!("invalid FEN move"),
            }
            .to_string();
            let col = (p.1 + 1).to_string();
            row + &col
        };

        stringify_pos(self.from)
            + &stringify_pos(self.to)
            + if let Some(p) = self.promotion {
                match p {
                    PieceTy::Queen => "q",
                    PieceTy::Rook => "r",
                    PieceTy::Bishop => "b",
                    PieceTy::Knight => "n",
                    _ => unreachable!(),
                }
            } else {
                ""
            }
    }
}

type Board = [[Square; 8]; 8];

#[derive(Debug, Clone)]
struct PrevPos {
    previous_positions: [u64; 256],
    prev_pos_idx: usize,
}

impl PartialEq for PrevPos {
    fn eq(&self, other: &Self) -> bool {
        if self.prev_pos_idx != other.prev_pos_idx {
            return false;
        }

        self.previous_positions
            .iter()
            .take(self.prev_pos_idx)
            .zip(other.previous_positions)
            .all(|(&n1, n2)| n1 == n2)
    }
}

impl Default for PrevPos {
    fn default() -> Self {
        Self::new()
    }
}

impl PrevPos {
    pub fn new() -> Self {
        Self {
            previous_positions: [0; 256],
            prev_pos_idx: 0,
        }
    }

    #[inline]
    pub fn undo(&mut self) {
        self.prev_pos_idx -= 1;
    }

    #[inline]
    pub fn push(&mut self, h: u64) {
        self.previous_positions[self.prev_pos_idx] = h;
        self.prev_pos_idx += 1;
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Undo {
    moving: Square,
    capture: Option<(Square, Pos)>,
    castleable: [(bool, bool); 2],
    en_pass: Option<Pos>,
    hash: u64,
    m: Move,
}

type BB = u64;

#[inline]
pub fn sq(pos: Pos) -> u32 {
    (pos.1 * 8 + pos.0) as u32
}
#[inline]
fn sq_to_pos(sq: u32) -> Pos {
    ((sq % 8) as usize, (sq / 8) as usize)
}
#[inline]
const fn bit(sq: u32) -> BB {
    1u64 << sq
}
#[inline]
const fn pop_lsb(bb: &mut BB) -> u32 {
    let sq = bb.trailing_zeros();
    *bb &= bb.wrapping_sub(1);
    sq
}
#[inline]
fn shift(bb: BB, n: i32) -> BB {
    if n > 0 { bb << n } else { bb >> -n }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BitBoards {
    pub pieces: [[BB; 6]; 2],
    pub color: [BB; 2],
    pub all: BB,
}

impl BitBoards {
    pub fn from_board(b: Board) -> Self {
        let mut s = Self {
            pieces: [[0; 6]; 2],
            color: [0; 2],
            all: 0,
        };

        for (idx, sq) in b
            .as_flattened()
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_empty())
        {
            let b = bit(idx as u32);

            s.all |= b;

            let color_idx = sq.color().to_index();
            s.color[color_idx] |= b;

            let piece_idx = (sq.ty().to_u8() - 1) as usize;
            s.pieces[color_idx][piece_idx] |= b;
        }

        s
    }

    pub fn get(&self, p: Pos) -> Square {
        let b = bit(sq(p));

        if self.all & b == 0 {
            return Square::EMPTY;
        }

        let (color, c_idx) = if self.color[0] & b != 0 {
            (Color::White, 0)
        } else {
            (Color::Black, 1)
        };

        let ty =
            PieceTy::from_u8((0..6).find(|&n| self.pieces[c_idx][n] & b != 0).unwrap() as u8 + 1);

        Square::piece(ty, color)
    }

    pub fn occupied(&self, p: Pos) -> bool {
        self.all & bit(sq(p)) != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    board: BitBoards,
    to_move: Color,
    castleable: [(bool, bool); 2],
    enpass: Option<Pos>,
    #[allow(unused)]
    full_move_clock: u32, // TODO: implement
    hash: u64,
    prev_pos: PrevPos,
    moves: Vec<Undo>,
}

impl Game {
    pub fn new(
        board: Board,
        castleable: [(bool, bool); 2],
        enpass: Option<Pos>,
        to_move: Color,
    ) -> Self {
        let hash = Self::gen_full_hash(&board, to_move, castleable, enpass);
        let board = BitBoards::from_board(board);
        Self {
            board,
            castleable,
            enpass,
            to_move,
            full_move_clock: 0,
            hash,
            prev_pos: PrevPos::default(),
            moves: Vec::new(),
        }
    }

    fn gen_full_hash(
        board: &Board,
        to_move: Color,
        castleable: [(bool, bool); 2],
        enpass: Option<Pos>,
    ) -> u64 {
        let mut h = 0u64;
        for (y, row) in board.iter().enumerate() {
            for (x, piece) in row.iter().enumerate() {
                if !piece.is_empty() {
                    h ^= ZOBRIST_TABLE[y][x][piece.to_usize()];
                }
            }
        }
        if to_move == Color::Black {
            h ^= ZOBRIST_SIDE_TO_MOVE;
        }
        for color in 0..2 {
            if castleable[color].0 {
                h ^= ZOBRIST_CASTLING[color][0];
            }
            if castleable[color].1 {
                h ^= ZOBRIST_CASTLING[color][1];
            }
        }
        if let Some((file, _)) = enpass {
            h ^= ZOBRIST_ENPASS[to_move.other().to_index()][file];
        }
        h
    }

    #[allow(unused)]
    pub fn from_fen(f: &str) -> Self {
        let mut parts = f.split(' ');
        let f = parts.next().unwrap();
        let to_move = parts.next();
        let castling = parts.next();
        let enpass = parts.next();

        let to_piece = |c: char| match c.to_ascii_lowercase() {
            'k' => PieceTy::King,
            'p' => PieceTy::Pawn,
            'n' => PieceTy::Knight,
            'b' => PieceTy::Bishop,
            'r' => PieceTy::Rook,
            'q' => PieceTy::Queen,
            _ => unreachable!(),
        };

        let mut x = 0;
        let mut y = 7; // fen string goes reverse to us

        let mut board = [[Square::EMPTY; 8]; 8];

        for c in f.chars() {
            if c == '/' {
                x = 0;
                y -= 1;
                continue;
            }

            if let Some(n) = c.to_digit(10) {
                x += n as usize;
                continue;
            }

            let color = if c.is_uppercase() {
                Color::White
            } else {
                Color::Black
            };
            let ty = to_piece(c);

            board[y][x] = Square::piece(ty, color);

            x += 1;
        }

        let to_move = if to_move == Some("w") {
            Color::White
        } else {
            Color::Black
        };

        let castleable = if let Some(c) = castling
            && c != "-"
        {
            let wq = c.contains('Q');
            let wk = c.contains('K');
            let bq = c.contains('q');
            let bk = c.contains('k');
            [(wq, wk), (bq, bk)]
        } else {
            [(false, false); 2]
        };

        let enpass = if let Some(e) = enpass
            && e != "-"
        {
            let (row, col) = e.split_at(1);
            let x = match row {
                "a" => 0,
                "b" => 1,
                "c" => 2,
                "d" => 3,
                "e" => 4,
                "f" => 5,
                "g" => 6,
                "h" => 7,
                _ => panic!("invalid FEN string"),
            };
            let y = col.parse::<usize>().unwrap() - 1;

            Some((x, y))
        } else {
            None
        };

        Self::new(board, castleable, enpass, to_move)
    }

    pub fn get_hash(&self) -> u64 {
        self.hash
    }

    #[inline]
    pub fn get_to_move(&self) -> Color {
        self.to_move
    }

    #[inline]
    pub fn has_been_played(&self, g: &Game) -> bool {
        self.prev_pos
            .previous_positions
            .iter()
            .take(self.prev_pos.prev_pos_idx)
            .any(|&n| n == g.get_hash())
    }

    pub fn lose_on_repeat(&self) -> bool {
        self.prev_pos
            .previous_positions
            .iter()
            .take(self.prev_pos.prev_pos_idx)
            .filter(|&&n| n == self.get_hash())
            .count()
            >= 2
    }

    pub fn hash_after_move(&self, m: Move, moving: Square) -> u64 {
        let mut hash = self.hash;

        let c = moving.color().to_index();
        let ec = 1 - c;

        let ep_capture = moving.ty() == PieceTy::Pawn && Some(m.to) == self.enpass;
        let capture = if ep_capture {
            let cap_pos = (m.to.0, m.from.1);
            Some((self.get(cap_pos), cap_pos))
        } else {
            let t = self.get(m.to);
            (!t.is_empty()).then_some((t, m.to))
        };

        hash ^= ZOBRIST_TABLE[m.from.1][m.from.0][moving.to_usize()];

        if ep_capture {
            let cap_pos = (m.to.0, m.from.1);
            let cap_sq = self.get(cap_pos);
            hash ^= ZOBRIST_TABLE[cap_pos.1][cap_pos.0][cap_sq.to_usize() - 1];
        }
        if let Some(ep) = self.enpass {
            hash ^= ZOBRIST_ENPASS[ec][ep.0];
        }
        if moving.ty() == PieceTy::Pawn && m.from.1.abs_diff(m.to.1) == 2 {
            hash ^= ZOBRIST_ENPASS[c][m.to.0];
        }

        let old_cas = self.castleable;
        let mut new_cas = self.castleable;
        if moving.ty() == PieceTy::King {
            new_cas[c] = (false, false);
        }
        if moving.ty() == PieceTy::Rook && m.from.1 == moving.color().start() {
            match m.from.0 {
                0 => new_cas[c].0 = false,
                7 => new_cas[c].1 = false,
                _ => {}
            }
        }
        if let Some((cap, cap_pos)) = capture.filter(|_| !ep_capture) {
            if cap.ty() == PieceTy::Rook && cap_pos.1 == cap.color().start() {
                match cap_pos.0 {
                    0 => new_cas[ec].0 = false,
                    7 => new_cas[ec].1 = false,
                    _ => {}
                }
            }
        }
        for color_idx in 0..2 {
            if old_cas[color_idx].0 != new_cas[color_idx].0 {
                hash ^= ZOBRIST_CASTLING[color_idx][0];
            }
            if old_cas[color_idx].1 != new_cas[color_idx].1 {
                hash ^= ZOBRIST_CASTLING[color_idx][1];
            }
        }

        if moving.ty() == PieceTy::King && m.from.0.abs_diff(m.to.0) == 2 {
            let rank = m.from.1;
            let (rx, rfinal) = if m.to.0 < m.from.0 { (0, 3) } else { (7, 5) };
            let rook = self.get((rx, rank));
            hash ^= ZOBRIST_TABLE[rank][rx][rook.to_usize()];
            hash ^= ZOBRIST_TABLE[rank][rfinal][rook.to_usize()];
        }

        if let Some((cap, _)) = capture.filter(|_| !ep_capture) {
            hash ^= ZOBRIST_TABLE[m.to.1][m.to.0][cap.to_usize()];
        }

        let final_sq = match m.promotion {
            Some(p) => Square::piece(p, moving.color()),
            None => moving,
        };
        hash ^= ZOBRIST_TABLE[m.to.1][m.to.0][final_sq.to_usize()];

        hash ^= ZOBRIST_SIDE_TO_MOVE;

        hash
    }

    pub fn move_piece(&mut self, m: Move) {
        let moving = self.get(m.from);

        let c = moving.color().to_index();
        let ec = 1 - c;
        let pt = moving.ty() as usize - 1;
        let from_bit = bit(sq(m.from));
        let to_bit = bit(sq(m.to));

        let ep_capture = moving.ty() == PieceTy::Pawn && Some(m.to) == self.enpass;
        let capture = if ep_capture {
            let cap_pos = (m.to.0, m.from.1);
            Some((self.get(cap_pos), cap_pos))
        } else {
            let t = self.get(m.to);
            (!t.is_empty()).then_some((t, m.to))
        };

        if let Some(cap) = capture
            && cap.0.ty() == PieceTy::King
        {
            panic!("capturing king!");
        }

        self.prev_pos.push(self.hash);
        self.moves.push(Undo {
            moving,
            capture,
            castleable: self.castleable,
            en_pass: self.enpass,
            hash: self.hash,
            m,
        });

        self.hash ^= ZOBRIST_TABLE[m.from.1][m.from.0][moving.to_usize()];

        if ep_capture {
            let cap_pos = (m.to.0, m.from.1);
            let cap_sq = self.get(cap_pos);
            let cap_bit = bit(sq(cap_pos));
            self.hash ^= ZOBRIST_TABLE[cap_pos.1][cap_pos.0][cap_sq.to_usize()];
            self.board.pieces[ec][BB_PAWN] &= !cap_bit;
            self.board.color[ec] &= !cap_bit;
            self.board.all &= !cap_bit;
        }

        if let Some(ep) = self.enpass {
            self.hash ^= ZOBRIST_ENPASS[ec][ep.0];
        }
        self.enpass = None;
        if moving.ty() == PieceTy::Pawn && m.from.1.abs_diff(m.to.1) == 2 {
            let mid_y = (m.from.1 + m.to.1) / 2;
            self.enpass = Some((m.to.0, mid_y));
            self.hash ^= ZOBRIST_ENPASS[c][m.to.0];
        }

        let old_cas = self.castleable;
        if moving.ty() == PieceTy::King {
            self.castleable[c] = (false, false);
        }
        if moving.ty() == PieceTy::Rook && m.from.1 == moving.color().start() {
            match m.from.0 {
                0 => self.castleable[c].0 = false,
                7 => self.castleable[c].1 = false,
                _ => {}
            }
        }
        if let Some((cap, cap_pos)) = capture.filter(|_| !ep_capture) {
            if cap.ty() == PieceTy::Rook && cap_pos.1 == cap.color().start() {
                match cap_pos.0 {
                    0 => self.castleable[ec].0 = false,
                    7 => self.castleable[ec].1 = false,
                    _ => {}
                }
            }
        }
        for color_idx in 0..2 {
            if old_cas[color_idx].0 != self.castleable[color_idx].0 {
                self.hash ^= ZOBRIST_CASTLING[color_idx][0];
            }
            if old_cas[color_idx].1 != self.castleable[color_idx].1 {
                self.hash ^= ZOBRIST_CASTLING[color_idx][1];
            }
        }

        if moving.ty() == PieceTy::King && m.from.0.abs_diff(m.to.0) == 2 {
            let rank = m.from.1;
            let (rx, rfinal) = if m.to.0 < m.from.0 { (0, 3) } else { (7, 5) };
            let rook = self.get((rx, rank));
            let r_from_bit = 1u64 << sq((rx, rank));
            let r_to_bit = 1u64 << sq((rfinal, rank));
            self.hash ^= ZOBRIST_TABLE[rank][rx][rook.to_usize()];
            self.hash ^= ZOBRIST_TABLE[rank][rfinal][rook.to_usize()];
            self.board.pieces[c][BB_ROOK] =
                (self.board.pieces[c][BB_ROOK] & !r_from_bit) | r_to_bit;
            self.board.color[c] = (self.board.color[c] & !r_from_bit) | r_to_bit;
            self.board.all = (self.board.all & !r_from_bit) | r_to_bit;
        }

        if let Some((cap, _)) = capture.filter(|_| !ep_capture) {
            let cpt = cap.ty() as usize - 1;
            self.hash ^= ZOBRIST_TABLE[m.to.1][m.to.0][cap.to_usize()];
            self.board.pieces[ec][cpt] &= !to_bit;
            self.board.color[ec] &= !to_bit;
        }

        self.board.pieces[c][pt] &= !from_bit;
        self.board.color[c] &= !from_bit;
        self.board.all &= !from_bit;

        let (final_pt, final_sq) = match m.promotion {
            Some(p) => (p as usize - 1, Square::piece(p, moving.color())),
            None => (pt, moving),
        };
        self.hash ^= ZOBRIST_TABLE[m.to.1][m.to.0][final_sq.to_usize()];
        self.board.pieces[c][final_pt] |= to_bit;
        self.board.color[c] |= to_bit;
        self.board.all |= to_bit;

        self.hash ^= ZOBRIST_SIDE_TO_MOVE;
        self.to_move = self.to_move.other();
    }

    pub fn undo_move(&mut self) {
        let undo = self.moves.pop().unwrap();

        self.to_move = self.to_move.other();
        self.hash = undo.hash;
        self.castleable = undo.castleable;
        self.enpass = undo.en_pass;
        self.prev_pos.undo();

        if undo.m.from == undo.m.to {
            return; // null move
        }

        let m = undo.m;
        let moving = undo.moving;
        let c = moving.color().to_index();
        let pt = moving.ty() as usize - 1;
        let from_bit = bit(sq(m.from));
        let to_bit = bit(sq(m.to));
        let final_pt = m.promotion.map(|p| p as usize - 1).unwrap_or(pt);

        self.board.pieces[c][final_pt] &= !to_bit;
        self.board.color[c] &= !to_bit;
        self.board.all &= !to_bit;

        self.board.pieces[c][pt] |= from_bit;
        self.board.color[c] |= from_bit;
        self.board.all |= from_bit;

        if let Some((cap, cap_pos)) = undo.capture {
            let cap_bit = bit(sq(cap_pos));
            let cpt = cap.ty() as usize - 1;
            let cec = cap.color().to_index();
            self.board.pieces[cec][cpt] |= cap_bit;
            self.board.color[cec] |= cap_bit;
            self.board.all |= cap_bit;
        }

        if moving.ty() == PieceTy::King && m.from.0.abs_diff(m.to.0) == 2 {
            let rank = m.from.1;
            let (rx, rfinal) = if m.to.0 < m.from.0 { (0, 3) } else { (7, 5) };
            let r_origin_bit = bit(sq((rx, rank)));
            let r_moved_bit = bit(sq((rfinal, rank)));
            self.board.pieces[c][BB_ROOK] =
                (self.board.pieces[c][BB_ROOK] & !r_moved_bit) | r_origin_bit;
            self.board.color[c] = (self.board.color[c] & !r_moved_bit) | r_origin_bit;
            self.board.all = (self.board.all & !r_moved_bit) | r_origin_bit;
        }
    }

    pub fn make_null_move(&mut self) {
        // TODO: technically, this (prev_pos) might cause issues as we might see a repeat
        // thats like not exactly real
        // BUT its just for null move pruning so who cares
        self.prev_pos.push(self.get_hash());
        self.moves.push(Undo {
            moving: Square::EMPTY,
            capture: None,
            castleable: self.castleable,
            hash: self.get_hash(),
            en_pass: self.enpass,
            m: Move {
                from: (8, 8),
                to: (8, 8),
                promotion: None,
            },
        });
        if let Some(ep) = self.enpass {
            self.hash ^= ZOBRIST_ENPASS[self.to_move.to_index()][ep.0];
        }
        self.enpass = None;
        self.to_move = self.to_move.other();
        self.hash ^= ZOBRIST_SIDE_TO_MOVE;
    }

    fn apply_move_board(&self, bb: &mut BitBoards, m: Move, moving: Square) {
        let c = moving.color().to_index();
        let from_bit = 1u64 << sq(m.from);
        let to_bit = 1u64 << sq(m.to);
        let pt = moving.ty() as usize - 1;

        bb.pieces[c][pt] &= !from_bit;
        bb.color[c] &= !from_bit;
        bb.all &= !from_bit;

        let cap = self.get(m.to);
        if !cap.is_empty() {
            let ec = cap.color().to_index();
            let ept = cap.ty() as usize - 1;
            bb.pieces[ec][ept] &= !to_bit;
            bb.color[ec] &= !to_bit;
        }

        if moving.ty() == PieceTy::Pawn && Some(m.to) == self.enpass {
            let ep_bit = 1u64 << sq((m.to.0, m.from.1));
            let ec = moving.color().other().to_index();
            bb.pieces[ec][BB_PAWN] &= !ep_bit;
            bb.color[ec] &= !ep_bit;
            bb.all &= !ep_bit;
        }

        let final_pt = m.promotion.map(|p| p as usize - 1).unwrap_or(pt);
        bb.pieces[c][final_pt] |= to_bit;
        bb.color[c] |= to_bit;
        bb.all |= to_bit;

        if moving.ty() == PieceTy::King && m.from.0.abs_diff(m.to.0) == 2 {
            let rank = m.from.1;
            let (rx, rfinal) = if m.to.0 < m.from.0 { (0, 3) } else { (7, 5) };
            let rf = 1u64 << sq((rx, rank));
            let rt = 1u64 << sq((rfinal, rank));
            bb.pieces[c][BB_ROOK] = (bb.pieces[c][BB_ROOK] & !rf) | rt;
            bb.color[c] = (bb.color[c] & !rf) | rt;
            bb.all = (bb.all & !rf) | rt;
        }
    }

    #[inline]
    pub fn occupied(&self, p: Pos) -> bool {
        self.board.occupied(p)
    }

    pub fn is_valid(&self, m: Move) -> bool {
        if m.from.0 >= 8 || m.from.1 >= 8 || m.to.0 >= 8 || m.to.1 >= 8 {
            return false;
        }
        if m.from == m.to {
            return false;
        }

        let piece = self.get(m.from);
        if piece.is_empty() {
            return false;
        }

        let color = piece.color();
        let c = color.to_index();
        let from_sq = sq(m.from);
        let to_sq = sq(m.to);
        let to_bit = bit(to_sq);
        let all = self.board.all;

        let target = self.get(m.to);
        if !target.is_empty() && (target.color() == color || target.ty() == PieceTy::King) {
            return false;
        }

        let geom_ok = match piece.ty() {
            PieceTy::Pawn => self.is_valid_pawn(m, color),
            PieceTy::Knight => {
                m.promotion.is_none() && KNIGHT_ATTACKS[from_sq as usize] & to_bit != 0
            }
            PieceTy::Bishop => {
                m.promotion.is_none()
                    && magic_bb::BISHOP_MAGICS[from_sq as usize]
                        .get(self.board.all, self.board.color[c])
                        & to_bit
                        != 0
            }
            PieceTy::Rook => {
                m.promotion.is_none()
                    && magic_bb::ROOK_MAGICS[from_sq as usize]
                        .get(self.board.all, self.board.color[c])
                        & to_bit
                        != 0
            }
            PieceTy::Queen => {
                m.promotion.is_none()
                    && (magic_bb::BISHOP_MAGICS[from_sq as usize].get(all, self.board.color[c])
                        | magic_bb::ROOK_MAGICS[from_sq as usize].get(all, self.board.color[c]))
                        & to_bit
                        != 0
            }
            PieceTy::King => self.is_valid_king(m, color, c),
        };

        if !geom_ok {
            return false;
        }

        self.move_is_legal(m)
    }

    pub fn move_is_legal_fast(&self, m: Move, in_check: bool) -> bool {
        let moving = self.get(m.from);
        let c = moving.color().to_index();
        let ec = 1 - c;
        let king_sq = self.board.pieces[c][BB_KING].trailing_zeros();

        let to_bit = bit(sq(m.to));
        if self.board.color[c] & to_bit != 0 || self.board.pieces[ec][BB_KING] & to_bit != 0 {
            return false;
        }

        if moving.ty() != PieceTy::King && !in_check {
            let is_ep = moving.ty() == PieceTy::Pawn && Some(m.to) == self.enpass;

            if !is_ep {
                let on_ray = (RANK_MASKS[king_sq as usize]
                    | FILE_MASKS[king_sq as usize]
                    | DIAGONAL_MASKS[king_sq as usize]
                    | ANTI_DIAGONAL_MASKS[king_sq as usize])
                    & bit(sq(m.from))
                    != 0;

                if !on_ray {
                    return true;
                }

                let diag_pinners =
                    self.board.pieces[ec][BB_BISHOP] | self.board.pieces[ec][BB_QUEEN];
                let straight_pinners =
                    self.board.pieces[ec][BB_ROOK] | self.board.pieces[ec][BB_QUEEN];

                let pinned_diag = magic_bb::BISHOP_MAGICS[king_sq as usize]
                    .get(self.board.all, self.board.color[c])
                    & diag_pinners
                    != 0;
                let pinned_straight = magic_bb::ROOK_MAGICS[king_sq as usize]
                    .get(self.board.all, self.board.color[c])
                    & straight_pinners
                    != 0;

                if !pinned_diag && !pinned_straight {
                    return true;
                }
            }
        }

        let mut bb = self.board.clone();
        self.apply_move_board(&mut bb, m, moving);
        let final_king_sq = if moving.ty() == PieceTy::King {
            bb.pieces[c][BB_KING].trailing_zeros()
        } else {
            king_sq
        };
        !Self::under_threat_pos(final_king_sq, moving.color().other(), &bb)
    }

    fn move_is_legal(&self, m: Move) -> bool {
        let in_check = self.check(self.get(m.from).color());
        self.move_is_legal_fast(m, in_check)
    }

    fn is_valid_pawn(&self, m: Move, color: Color) -> bool {
        let (push_dir, start_y, promo_y) = match color {
            Color::White => (1, 1, 7),
            Color::Black => (-1, 6, 0),
        };

        let dy = m.to.1 as i32 - m.from.1 as i32;
        let dx = m.to.0 as i32 - m.from.0 as i32;

        if (m.to.1 == promo_y) != m.promotion.is_some() {
            return false;
        }
        if let Some(p) = m.promotion {
            if matches!(p, PieceTy::Pawn | PieceTy::King) {
                return false;
            }
        }

        if dy != push_dir && dy != push_dir * 2 {
            return false;
        }

        match dx {
            0 => {
                let mid = (m.from.0, (m.from.1 as i32 + push_dir) as usize);
                if self.board.all & bit(sq(mid)) != 0 {
                    return false;
                }
                if dy == push_dir * 2 {
                    if m.from.1 != start_y {
                        return false;
                    }
                    if self.board.all & bit(sq(m.to)) != 0 {
                        return false;
                    }
                }
            }
            1 | -1 => {
                if dy != push_dir {
                    return false;
                }
                let en_pass = Some(m.to) == self.enpass;
                if !en_pass && !self.occupied(m.to) {
                    return false;
                }
                if en_pass && !self.occupied(m.to) {
                    let victim = self.get((m.to.0, m.from.1));
                    if victim.is_empty() || victim.color() == color {
                        return false;
                    }
                }
            }
            _ => return false,
        }

        true
    }

    fn is_valid_king(&self, m: Move, color: Color, c: usize) -> bool {
        if m.promotion.is_some() {
            return false;
        }

        let dx = m.to.0 as i32 - m.from.0 as i32;
        let dy = m.to.1 as i32 - m.from.1 as i32;

        if dx.abs() == 2 && dy == 0 {
            let queenside = dx < 0;
            let can_castle = if queenside {
                self.castleable[c].0
            } else {
                self.castleable[c].1
            };
            if !can_castle {
                return false;
            }

            if self.check(color) {
                return false;
            }

            let rank = m.from.1;

            let (empty_lo, empty_hi) = if queenside { (1, 3) } else { (5, 6) };
            for x in empty_lo..=empty_hi {
                if self.board.all & (1u64 << sq((x, rank))) != 0 {
                    return false;
                }
            }

            let pass_x = if queenside { 3 } else { 5 };
            if Self::under_threat_pos(sq((pass_x, rank)), color.other(), &self.board) {
                return false;
            }

            true
        } else {
            KING_ATTACKS[sq(m.from) as usize] & (1u64 << sq(m.to)) != 0
        }
    }

    pub fn check(&self, c: Color) -> bool {
        self.under_threat(Square::piece(PieceTy::King, c))
    }

    #[allow(unused)]
    pub fn get_pieces_color(&self, c: Color) -> Vec<(Square, Pos)> {
        self.get_all_pieces()
            .into_iter()
            .filter(|(p, _)| p.color() == c)
            .collect()
    }

    pub fn get_all_pieces(&self) -> impl Iterator<Item = (Square, Pos)> {
        self.get_all_pieces_ty(PieceTy::Pawn)
            .chain(self.get_all_pieces_ty(PieceTy::Bishop))
            .chain(self.get_all_pieces_ty(PieceTy::Knight))
            .chain(self.get_all_pieces_ty(PieceTy::Rook))
            .chain(self.get_all_pieces_ty(PieceTy::Queen))
            .chain(self.get_all_pieces_ty(PieceTy::King))
    }

    pub fn get_all_pieces_color(&self, c: Color) -> impl Iterator<Item = (Square, Pos)> {
        self.get_all_pieces_ty_color(PieceTy::Pawn, c)
            .chain(self.get_all_pieces_ty_color(PieceTy::Bishop, c))
            .chain(self.get_all_pieces_ty_color(PieceTy::Knight, c))
            .chain(self.get_all_pieces_ty_color(PieceTy::Rook, c))
            .chain(self.get_all_pieces_ty_color(PieceTy::Queen, c))
            .chain(self.get_all_pieces_ty_color(PieceTy::King, c))
    }

    pub fn get_all_pieces_ty(&self, ty: PieceTy) -> impl Iterator<Item = (Square, Pos)> {
        self.get_all_pieces_ty_color(ty, Color::White)
            .chain(self.get_all_pieces_ty_color(ty, Color::Black))
    }

    pub fn get_all_pieces_ty_color(
        &self,
        ty: PieceTy,
        c: Color,
    ) -> impl Iterator<Item = (Square, Pos)> {
        let mut pieces = self.board.pieces[c.to_index()][ty.to_u8() as usize - 1];
        std::iter::from_fn(move || {
            (pieces != 0).then(|| {
                let pos = pop_lsb(&mut pieces);
                (Square::piece(ty, c), sq_to_pos(pos))
            })
        })
    }

    pub fn under_threat_pos(sq: u32, by: Color, board: &BitBoards) -> bool {
        let c = by.to_index();

        if PAWN_ATTACKS[1 - c][sq as usize] & board.pieces[c][BB_PAWN] != 0 {
            return true;
        }

        if KNIGHT_ATTACKS[sq as usize] & board.pieces[c][BB_KNIGHT] != 0 {
            return true;
        }

        if KING_ATTACKS[sq as usize] & board.pieces[c][BB_KING] != 0 {
            return true;
        }

        let diag = magic_bb::BISHOP_MAGICS[sq as usize].get(board.all, board.color[1 - c]);
        if diag & (board.pieces[c][BB_BISHOP] | board.pieces[c][BB_QUEEN]) != 0 {
            return true;
        }

        let straight = magic_bb::ROOK_MAGICS[sq as usize].get(board.all, board.color[1 - c]);
        if straight & (board.pieces[c][BB_ROOK] | board.pieces[c][BB_QUEEN]) != 0 {
            return true;
        }

        false
    }

    #[inline]
    pub fn under_threat(&self, p: Square) -> bool {
        let pos = sq(self.get_piece_pos(p));

        Self::under_threat_pos(pos, p.color().other(), &self.board)
    }

    pub fn checkmate(&self, c: Color) -> bool {
        self.check(c) && !self.has_any_legal_move(c) && !self.lose_on_repeat()
    }

    pub fn stalemate(&self, c: Color) -> bool {
        let not_in_check = !self.check(c);
        let repeat = self.lose_on_repeat();
        let no_moves = !self.has_any_legal_move(c);

        (not_in_check && no_moves) || repeat
    }

    pub fn is_capture(&self, m: Move) -> bool {
        if self.occupied(m.to) {
            return true;
        }

        if let p = self.get(m.from)
            && p.ty() == PieceTy::Pawn
            && m.from.0 != m.to.0
        {
            return true;
        }

        false
    }

    /// returns Option<bool> Some if capture, true if en passant
    pub fn is_capture_known_move(&self, m: Move, moving: Square) -> Option<bool> {
        if self.occupied(m.to) {
            return Some(false);
        }

        if moving.ty() == PieceTy::Pawn && m.from.0 != m.to.0 {
            return Some(true);
        }

        None
    }

    pub fn get_all_moves(&self, c: Color) -> Vec<Move> {
        let in_check = self.check(c);
        self.get_all_pseudo_moves(c)
            .into_iter()
            .filter(|m| self.move_is_legal_fast(*m, in_check))
            .collect()
    }

    pub fn get_all_pseudo_moves(&self, color: Color) -> Vec<Move> {
        // technically, max is 218. but this is good enough as vec can resize.
        // we use a vec instead of an array because returning an array is expensive. we COULD use an out parameter
        // which might be a good idea to prevent the expensiveness of heap allocation
        // TODO
        // update: tried it. didn't seem faster.
        let mut moves = Vec::with_capacity(64);
        let c = color.to_index();
        let own = self.board.color[c];
        let all = self.board.all;
        let enemy = self.board.color[1 - c];

        self.gen_pawn_moves(color, c, all, enemy, &mut moves);
        self.gen_knight_moves(c, own, &mut moves);
        self.gen_bishop_moves(c, own, all, &mut moves);
        self.gen_rook_moves(c, own, all, &mut moves);
        self.gen_queen_moves(c, own, all, &mut moves);
        self.gen_king_moves(color, c, own, all, &mut moves);

        moves
    }

    fn gen_pawn_moves(&self, color: Color, c: usize, all: BB, enemy: BB, moves: &mut Vec<Move>) {
        let pawns = self.board.pieces[c][BB_PAWN];
        let free = !all;

        if pawns == 0 {
            return;
        }

        let (push, pre_promo, mid) = if color == Color::White {
            (8i32, RANK_7, RANK_3)
        } else {
            (-8i32, RANK_2, RANK_6)
        };

        let (left_dir, right_dir) = if color == Color::White {
            (7i32, 9i32)
        } else {
            (-9i32, -7i32)
        };

        let non_promo = pawns & !pre_promo;
        let promo_pawns = pawns & pre_promo;

        let single_push = shift(non_promo, push) & free;
        let mut tmp = single_push;
        while tmp != 0 {
            let to = pop_lsb(&mut tmp);
            let from = (to as i32 - push) as u32;
            moves.push(Move {
                from: sq_to_pos(from),
                to: sq_to_pos(to),
                promotion: None,
            });
        }

        let mut double_push = shift(single_push & mid, push) & free;
        while double_push != 0 {
            let to = pop_lsb(&mut double_push);
            let from = (to as i32 - push * 2) as u32;
            moves.push(Move {
                from: sq_to_pos(from),
                to: sq_to_pos(to),
                promotion: None,
            });
        }

        let mut left_cap = shift(non_promo & !FILE_A, left_dir) & enemy;
        while left_cap != 0 {
            let to = pop_lsb(&mut left_cap);
            let from = (to as i32 - left_dir) as u32;
            moves.push(Move {
                from: sq_to_pos(from),
                to: sq_to_pos(to),
                promotion: None,
            });
        }

        let mut right_cap = shift(non_promo & !FILE_H, right_dir) & enemy;
        while right_cap != 0 {
            let to = pop_lsb(&mut right_cap);
            let from = (to as i32 - right_dir) as u32;
            moves.push(Move {
                from: sq_to_pos(from),
                to: sq_to_pos(to),
                promotion: None,
            });
        }

        const PROMOS: [PieceTy; 4] = [
            PieceTy::Queen,
            PieceTy::Rook,
            PieceTy::Bishop,
            PieceTy::Knight,
        ];

        let mut straight_promo = shift(promo_pawns, push) & free;
        while straight_promo != 0 {
            let to = pop_lsb(&mut straight_promo);
            let from = sq_to_pos((to as i32 - push) as u32);
            let to = sq_to_pos(to);
            for p in PROMOS {
                moves.push(Move {
                    from,
                    to,
                    promotion: Some(p),
                });
            }
        }

        let mut left_cap_promo = shift(promo_pawns & !FILE_A, left_dir) & enemy;
        while left_cap_promo != 0 {
            let to = pop_lsb(&mut left_cap_promo);
            let from = sq_to_pos((to as i32 - left_dir) as u32);
            let to = sq_to_pos(to);
            for p in PROMOS {
                moves.push(Move {
                    from,
                    to,
                    promotion: Some(p),
                });
            }
        }

        let mut right_cap_promo = shift(promo_pawns & !FILE_H, right_dir) & enemy;
        while right_cap_promo != 0 {
            let to = pop_lsb(&mut right_cap_promo);
            let from = sq_to_pos((to as i32 - right_dir) as u32);
            let to = sq_to_pos(to);
            for p in PROMOS {
                moves.push(Move {
                    from,
                    to,
                    promotion: Some(p),
                });
            }
        }

        if let Some(ep) = self.enpass {
            let s = sq(ep);
            let mut attackers = PAWN_ATTACKS[1 - c][s as usize] & pawns;
            while attackers != 0 {
                let from = pop_lsb(&mut attackers);
                moves.push(Move {
                    from: sq_to_pos(from),
                    to: ep,
                    promotion: None,
                });
            }
        }
    }

    fn gen_knight_moves(&self, c: usize, own: BB, moves: &mut Vec<Move>) {
        let mut pieces = self.board.pieces[c][BB_KNIGHT];
        while pieces != 0 {
            let from = pop_lsb(&mut pieces);
            let mut targets = KNIGHT_ATTACKS[from as usize] & !own;
            while targets != 0 {
                let to = pop_lsb(&mut targets);
                moves.push(Move {
                    from: sq_to_pos(from),
                    to: sq_to_pos(to),
                    promotion: None,
                });
            }
        }
    }

    fn gen_bishop_moves(&self, c: usize, own: BB, all: BB, moves: &mut Vec<Move>) {
        let mut pieces = self.board.pieces[c][BB_BISHOP];
        while pieces != 0 {
            let from = pop_lsb(&mut pieces);
            let mut targets = magic_bb::BISHOP_MAGICS[from as usize].get(all, own);
            while targets != 0 {
                let to = pop_lsb(&mut targets);
                moves.push(Move {
                    from: sq_to_pos(from),
                    to: sq_to_pos(to),
                    promotion: None,
                });
            }
        }
    }

    fn gen_rook_moves(&self, c: usize, own: BB, all: BB, moves: &mut Vec<Move>) {
        let mut pieces = self.board.pieces[c][BB_ROOK];
        while pieces != 0 {
            let from = pop_lsb(&mut pieces);
            let mut targets = magic_bb::ROOK_MAGICS[from as usize].get(all, own);
            while targets != 0 {
                let to = pop_lsb(&mut targets);
                moves.push(Move {
                    from: sq_to_pos(from),
                    to: sq_to_pos(to),
                    promotion: None,
                });
            }
        }
    }

    fn gen_queen_moves(&self, c: usize, own: BB, all: BB, moves: &mut Vec<Move>) {
        let mut pieces = self.board.pieces[c][BB_QUEEN];
        while pieces != 0 {
            let from = pop_lsb(&mut pieces);
            let mut targets = magic_bb::BISHOP_MAGICS[from as usize].get(all, own)
                | magic_bb::ROOK_MAGICS[from as usize].get(all, own);
            while targets != 0 {
                let to = pop_lsb(&mut targets);
                moves.push(Move {
                    from: sq_to_pos(from),
                    to: sq_to_pos(to),
                    promotion: None,
                });
            }
        }
    }

    fn gen_king_moves(&self, color: Color, c: usize, own: BB, all: BB, moves: &mut Vec<Move>) {
        let king_bb = self.board.pieces[c][BB_KING];
        if king_bb == 0 {
            return;
        }
        let from = king_bb.trailing_zeros();

        let mut targets = KING_ATTACKS[from as usize] & !own;
        while targets != 0 {
            let to = pop_lsb(&mut targets);
            moves.push(Move {
                from: sq_to_pos(from),
                to: sq_to_pos(to),
                promotion: None,
            });
        }

        let back_rank = color.start();
        let (can_queenside, can_kingside) = self.castleable[c];

        if can_kingside {
            let between = (1 << (back_rank * 8 + 5)) | (1 << (back_rank * 8 + 6));
            if all & between == 0 {
                let to = sq_to_pos((back_rank * 8 + 6) as u32);
                moves.push(Move {
                    from: sq_to_pos(from),
                    to,
                    promotion: None,
                });
            }
        }

        if can_queenside {
            let between = (1 << (back_rank * 8 + 1))
                | (1 << (back_rank * 8 + 2))
                | (1 << (back_rank * 8 + 3));
            if all & between == 0 {
                let to = sq_to_pos((back_rank * 8 + 2) as u32);
                moves.push(Move {
                    from: sq_to_pos(from),
                    to,
                    promotion: None,
                });
            }
        }
    }

    pub fn has_any_legal_move(&self, color: Color) -> bool {
        let in_check = self.check(color);
        self.get_all_pseudo_moves(color)
            .into_iter()
            .any(|m| self.move_is_legal_fast(m, in_check))
    }

    #[inline]
    pub fn get_piece_pos(&self, piece: Square) -> Pos {
        let s = self.board.pieces[piece.color().to_index()][piece.ty().to_u8() as usize - 1]
            .trailing_zeros();
        sq_to_pos(s)
    }

    #[inline]
    pub fn get(&self, p: Pos) -> Square {
        self.board.get(p)
    }

    #[inline]
    pub fn piece_count(&self, s: Square) -> u32 {
        self.board.pieces[s.color().to_index()][s.ty() as usize - 1].count_ones()
    }

    pub fn doubled_pawns_check(&self, color: Color) -> i32 {
        let board = self.board.pieces[color.to_index()][BB_PAWN];
        (0..8)
            .map(|r| {
                let rank = FILE_A << r;

                let pawns = (board & rank).count_ones();

                if pawns > 1 { pawns as i32 - 1 } else { 0 }
            })
            .sum()
    }

    pub fn pawn_endgame(&self) -> bool {
        (self.board.pieces[0][BB_PAWN]
            | self.board.pieces[1][BB_PAWN]
            | self.board.pieces[0][BB_KING]
            | self.board.pieces[1][BB_KING])
            == self.board.all
    }

    #[allow(unused)]
    pub fn passed_pawns_check(&self, c: Color) -> i32 {
        let cx = c.to_index();
        let us = self.board.pieces[cx][BB_PAWN];
        let them = self.board.pieces[1 - cx][BB_PAWN];
        let mut count = 0;
        let mut bb = us;
        while bb != 0 {
            let sq = bb.trailing_zeros() as usize;
            bb &= bb - 1;
            if them & PASSED_MASK[cx][sq] == 0 {
                count += 1;
            }
        }
        count
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    }
}

static PASSED_MASK: [[u64; 64]; 2] = compute_passed_masks();

const fn min(n1: usize, n2: usize) -> usize {
    if n1 < n2 { n1 } else { n2 }
}

const fn compute_passed_masks() -> [[u64; 64]; 2] {
    let mut masks = [[0u64; 64]; 2];
    let mut sq = 0;

    while sq < 64 {
        let file = sq % 8;
        let rank = sq / 8;
        let files =
            FILE_MASKS[file] | FILE_MASKS[file.saturating_sub(1)] | FILE_MASKS[min(file + 1, 7)];
        masks[Color::White.to_index()][sq] = files
            & if rank == 7 {
                0
            } else {
                u64::MAX << (8 * (rank + 1))
            };
        masks[Color::Black.to_index()][sq] = files
            & if rank == 0 {
                0
            } else {
                u64::MAX >> (8 * (8 - rank))
            };
        sq += 1;
    }
    masks
}

const fn gen_pawn_attacks() -> [[BB; 64]; 2] {
    let mut attacks = [[0u64; 64]; 2];
    let mut sq = 0;
    while sq < 64 {
        let b = 1u64 << sq;

        attacks[0][sq as usize] = ((b & !FILE_A) << 7) | ((b & !FILE_H) << 9);
        attacks[1][sq as usize] = ((b & !FILE_A) >> 9) | ((b & !FILE_H) >> 7);
        sq += 1;
    }
    attacks
}

static PAWN_ATTACKS: [[BB; 64]; 2] = gen_pawn_attacks();

const fn gen_knight_attacks() -> [BB; 64] {
    let mut masks = [0; 64];
    let mut sq = 0;
    while sq < 64 {
        let b = bit(sq as u32);
        let mut att = 0u64;
        att |= (b << 17) & !FILE_A;
        att |= (b << 15) & !FILE_H;
        att |= (b << 10) & !(FILE_A | FILE_B);
        att |= (b << 6) & !(FILE_G | FILE_H);
        att |= (b >> 17) & !FILE_H;
        att |= (b >> 15) & !FILE_A;
        att |= (b >> 10) & !(FILE_G | FILE_H);
        att |= (b >> 6) & !(FILE_A | FILE_B);
        masks[sq] = att;
        sq += 1;
    }
    masks
}

static KNIGHT_ATTACKS: [BB; 64] = gen_knight_attacks();

const fn gen_king_attacks() -> [BB; 64] {
    let mut masks = [0; 64];
    let mut sq = 0;
    while sq < 64 {
        let b = 1u64 << sq;
        let mut att = 0u64;

        att |= b << 8;
        att |= (b & !FILE_H) << 9;
        att |= (b & !FILE_H) << 1;
        att |= (b & !FILE_H) >> 7;
        att |= b >> 8;
        att |= (b & !FILE_A) >> 9;
        att |= (b & !FILE_A) >> 1;
        att |= (b & !FILE_A) << 7;

        masks[sq] = att;
        sq += 1;
    }
    masks
}

static KING_ATTACKS: [BB; 64] = gen_king_attacks();

const fn gen_rank_masks() -> [BB; 64] {
    let mut masks = [0; 64];
    let mut sq = 0;
    while sq < 64 {
        masks[sq] = 0xFFu64 << (sq & 56);
        sq += 1;
    }
    masks
}

static RANK_MASKS: [BB; 64] = gen_rank_masks();

const fn gen_file_masks() -> [BB; 64] {
    let mut masks = [0; 64];
    let mut sq = 0;
    while sq < 64 {
        masks[sq] = FILE_A << (sq % 8);
        sq += 1;
    }
    masks
}

static FILE_MASKS: [BB; 64] = gen_file_masks();

const fn gen_diagonal_masks(dir: i32) -> [BB; 64] {
    let mut masks = [0; 64];
    let mut sq = 0;
    while sq < 64 {
        let d = (sq / 8) as i32 + ((sq % 8) as i32 * dir);
        let mut mask = 0u64;
        let mut s = 0;
        while s < 64 {
            if (s / 8) as i32 + (s % 8) as i32 * dir == d {
                mask |= 1u64 << s;
            }
            s += 1;
        }
        masks[sq] = mask;
        sq += 1;
    }

    masks
}

// top right -> bottom left
static DIAGONAL_MASKS: [BB; 64] = gen_diagonal_masks(-1);

// top left -> bottom right
static ANTI_DIAGONAL_MASKS: [BB; 64] = gen_diagonal_masks(1);

const FILE_A: u64 = 0x0101010101010101;
const FILE_B: u64 = FILE_A << 1;
#[allow(unused)]
const FILE_C: u64 = FILE_A << 2;
#[allow(unused)]
const FILE_D: u64 = FILE_A << 3;
#[allow(unused)]
const FILE_E: u64 = FILE_A << 4;
#[allow(unused)]
const FILE_F: u64 = FILE_A << 5;
const FILE_G: u64 = FILE_A << 6;
const FILE_H: u64 = FILE_A << 7;

const RANK_1: u64 = 0x00000000000000FF;
const RANK_2: u64 = RANK_1 << (8 * 1);
const RANK_3: u64 = RANK_1 << (8 * 2);
#[allow(unused)]
const RANK_4: u64 = RANK_1 << (8 * 3);
#[allow(unused)]
const RANK_5: u64 = RANK_1 << (8 * 4);
const RANK_6: u64 = RANK_1 << (8 * 5);
const RANK_7: u64 = RANK_1 << (8 * 6);
#[allow(unused)]
const RANK_8: u64 = RANK_1 << (8 * 7);
