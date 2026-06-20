use rand::{prelude::*, rngs::SmallRng};
use rayon::{iter::ParallelIterator, prelude::*};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Piece {
    pub ty: PieceTy,
    pub color: Color,
}

impl Piece {
    pub fn new(ty: PieceTy, color: Color) -> Self {
        Self { ty, color }
    }

    pub fn to_int(&self) -> usize {
        (match self.ty {
            PieceTy::Pawn => 0,
            PieceTy::Knight => 1,
            PieceTy::Bishop => 2,
            PieceTy::Rook => 3,
            PieceTy::Queen => 4,
            PieceTy::King => 5,
        } * 2)
            + match self.color {
                Color::Black => 0,
                Color::White => 1,
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PieceTy {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn to_int(self) -> isize {
        match self {
            Self::White => 1,
            Self::Black => -1,
        }
    }

    pub fn to_index(&self) -> usize {
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
}

pub type Pos = (usize, usize);
pub type Move = (Pos, Pos);

type Board = [[Option<Piece>; 8]; 8];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    board: Board,
    to_move: Color,
    castleable: [(bool, bool); 2],
    enpass: Option<Pos>,
    full_move_clock: u32,

    previous_positions: HashMap<u64, usize>, // hash -> number of times
    zobrist_table: [[[u64; 12]; 8]; 8],
}

impl Hash for Game {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let mut z_hash = 0;
        for (y, row) in self.board.iter().enumerate() {
            for (x, piece) in row.iter().enumerate() {
                if let Some(piece) = piece {
                    z_hash ^= self.zobrist_table[y][x][piece.to_int()];
                }
            }
        }

        z_hash.hash(state);
        self.castleable.hash(state);
        self.enpass.hash(state);
        self.full_move_clock.hash(state);
        self.to_move.hash(state);
    }
}

impl Game {
    pub fn new(board: Board, to_move: Color) -> Self {
        Self {
            board,
            castleable: [(true, true); 2],
            enpass: None,
            to_move,
            previous_positions: HashMap::new(),
            full_move_clock: 0,
            zobrist_table: Self::gen_zobrist_table(),
        }
    }

    fn gen_zobrist_table() -> [[[u64; 12]; 8]; 8] {
        const ZOBRIST_SEED: [u8; 32] = [
            150, 77, 57, 107, 45, 235, 181, 109, 28, 241, 146, 156, 218, 138, 213, 40, 216, 39,
            196, 149, 150, 119, 232, 178, 175, 106, 25, 225, 48, 187, 117, 162,
        ];
        let mut rng = SmallRng::from_seed(ZOBRIST_SEED);

        let mut z = [[[0; 12]; 8]; 8];
        for y in 0..8 {
            for x in 0..8 {
                for p in 0..12 {
                    z[y][x][p] = rng.next_u64();
                }
            }
        }
        z
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

        let mut board = [[None; 8]; 8];

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

            board[y][x] = Some(Piece::new(ty, color));

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
            [(true, true); 2]
        } else {
            [(true, true); 2]
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

        Self {
            board,
            to_move,
            castleable,
            enpass,
            previous_positions: HashMap::new(),
            full_move_clock: 0,
            zobrist_table: Self::gen_zobrist_table(),
        }
    }

    pub fn get_hash(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.hash(&mut s);
        s.finish()
    }

    #[inline]
    pub fn get_to_move(&self) -> Color {
        self.to_move
    }

    #[inline]
    pub fn swap_to_move(&mut self) {
        self.to_move = self.to_move.other();
    }

    #[inline]
    pub fn has_been_played(&self, g: &Game) -> bool {
        self.previous_positions.contains_key(&g.get_hash())
    }

    pub fn lose_on_repeat(&self) -> bool {
        self.previous_positions
            .get(&self.get_hash())
            .is_some_and(|n| *n >= 2)
    }

    #[inline]
    pub fn get_board(&self) -> &Board {
        &self.board
    }

    #[inline]
    pub fn get_rank(&self, n: usize) -> &[Option<Piece>; 8] {
        &self.board[n]
    }

    pub fn move_piece(&mut self, m: Move) {
        if m.0 == m.1 {
            return;
        }

        *self.previous_positions.entry(self.get_hash()).or_insert(0) += 1;

        let moving = self.board[m.0.1][m.0.0].unwrap();
        if moving.ty == PieceTy::Pawn && Some(m.1) == self.enpass {
            self.board[m.0.1][m.1.0] = None; // y of start, x of end
        }
        self.enpass = None;
        if moving.ty == PieceTy::Pawn && (m.0.1.abs_diff(m.1.1) == 2) {
            let mid_y = (m.0.1 + m.1.1) / 2;
            self.enpass = Some((m.1.1, mid_y));
        }
        if moving.ty == PieceTy::King {
            self.castleable[moving.color.to_index()] = (false, false);
        }
        if moving.ty == PieceTy::Rook && m.0.1 == moving.color.to_index() * 7 {
            if m.0.0 == 0 {
                self.castleable[moving.color.to_index()].0 = false;
            } else if m.0.0 == 7 {
                self.castleable[moving.color.to_index()].1 = false;
            }
        }

        if moving.ty == PieceTy::King && m.0.0.abs_diff(m.1.0) == 2 {
            // castling
            // we move the rook too
            let (rook_x, rook_final) = if m.0.0 > m.1.0 { (0, 3) } else { (7, 5) };
            self.board[m.0.1][rook_final] = self.board[m.0.1][rook_x];
            self.board[m.0.1][rook_x] = None
        }

        self.board[m.1.1][m.1.0] = self.board[m.0.1][m.0.0];
        self.board[m.0.1][m.0.0] = None;
    }

    #[inline]
    pub fn occupied(&self, p: Pos) -> bool {
        self.board[p.1][p.0].is_some()
    }

    pub fn is_valid(&self, m: Move) -> bool {
        let start = m.0;
        let end = m.1;

        if start == end {
            return false;
        }

        let Some(piece) = self.board[start.1][start.0] else {
            return false;
        };

        if self
            .get(end)
            .is_some_and(|p| p.ty == PieceTy::King || p.color == piece.color)
        {
            return false;
        }

        match piece.ty {
            PieceTy::Bishop => {
                let x_change = start.0 as isize - end.0 as isize;
                let y_change = start.1 as isize - end.1 as isize;

                if x_change != y_change {
                    return false;
                }

                // one less as we don't care whether the target is occupied
                for delta in 1..(x_change.abs()) {
                    let delta = delta * x_change.signum();

                    let pos = (
                        (start.0 as isize + delta) as usize,
                        (start.1 as isize + delta) as usize,
                    );

                    if pos.0 >= 8 || pos.1 >= 8 || self.occupied(pos) {
                        return false;
                    }
                }
            }
            PieceTy::Pawn => {
                let move_dir = piece.color.to_int();

                let occupied = self.occupied(end);

                let front = match piece.color {
                    Color::Black => 6,
                    Color::White => 1,
                };
                let can_be_double = start.1 == front;

                if start.1 as isize + move_dir != end.1 as isize {
                    if !(can_be_double && start.1 as isize + move_dir * 2 == end.1 as isize)
                        || self.occupied((start.0, (start.1 as isize + move_dir) as usize))
                    {
                        return false;
                    }
                }

                let x_dist = start.0.abs_diff(end.0);
                if x_dist == 0 && occupied {
                    return false;
                } else if x_dist > 1 {
                    return false;
                } else if x_dist == 1 && !occupied && Some(end) != self.enpass {
                    return false;
                }
            }
            PieceTy::Rook => {
                let vertical = start.1 != end.1;
                let horizontal = start.0 != end.0;

                if !(vertical ^ horizontal) {
                    return false;
                }

                let man_dist = start.1.abs_diff(end.1) + start.0.abs_diff(end.0);

                // one less as we don't care whether the target is occupied
                for delta in 1..man_dist {
                    let pos = if vertical {
                        (start.0, start.1 + delta)
                    } else {
                        (start.0 + delta, start.1)
                    };

                    if pos.0 >= 8 || pos.1 >= 8 || self.occupied(pos) {
                        return false;
                    }
                }
            }
            PieceTy::King => {
                let x_change = start.0.abs_diff(end.0);
                let y_change = start.1.abs_diff(end.1);

                if x_change == 2 && y_change == 0 {
                    // check for castling
                    let color_idx = piece.color.to_index();
                    let (legal, rook_x_ish, rook_final) = if m.0.0 > m.1.0 {
                        (self.castleable[color_idx].0, 1, 3)
                    } else {
                        (self.castleable[color_idx].1, 6, 5)
                    };
                    if !legal {
                        return false;
                    }

                    if self.check(piece.color) {
                        return false;
                    }

                    let start = rook_final.min(rook_x_ish);
                    let end = rook_final + rook_x_ish - start;
                    for x in start..=end {
                        if self.occupied((x, m.0.1))
                            || self.under_threat_pos((x, m.0.1), piece.color.other())
                        {
                            return false;
                        }
                    }
                } else if x_change > 1 || y_change > 1 {
                    return false;
                }
            }
            PieceTy::Knight => {
                let x_change = start.0.abs_diff(end.0);
                let y_change = start.1.abs_diff(end.1);

                if !((x_change == 2) ^ (y_change == 2)) || !((x_change == 1) ^ (y_change == 1)) {
                    return false;
                }
            }
            PieceTy::Queen => {
                let x_change = start.0.abs_diff(end.0);
                let y_change = start.1.abs_diff(end.1);

                if x_change == 0 || y_change == 0 {
                    // rook-like movement
                    let vertical = start.1 != end.1;
                    let horizontal = start.0 != end.0;

                    if !(vertical ^ horizontal) {
                        return false;
                    }

                    let man_dist = start.1.abs_diff(end.1) + start.0.abs_diff(end.0);

                    // one less as we don't care whether the target is occupied
                    for delta in 1..man_dist {
                        let pos = if vertical {
                            (start.0, start.1 + delta)
                        } else {
                            (start.0 + delta, start.1)
                        };

                        if pos.0 >= 8 || pos.1 >= 8 || self.occupied(pos) {
                            return false;
                        }
                    }
                } else {
                    // bishop-like movement
                    let x_change = start.0 as isize - end.0 as isize;
                    let y_change = start.1 as isize - end.1 as isize;

                    if x_change != y_change {
                        return false;
                    }

                    // one less as we don't care whether the target is occupied
                    for delta in 1..(x_change.abs()) {
                        let dx = delta * x_change.signum();
                        let dy = delta * y_change.signum();

                        let pos = (
                            (start.0 as isize + dx) as usize,
                            (start.1 as isize + dy) as usize,
                        );

                        if pos.0 >= 8 || pos.1 >= 8 || self.occupied(pos) {
                            return false;
                        }
                    }
                }
            }
        }

        let move_made = self.clone().move_change(m);

        if self
            .previous_positions
            .get(&move_made.get_hash())
            .is_some_and(|n| *n >= 2)
        {
            return false;
        }

        !move_made.check(piece.color)
    }

    pub fn check(&self, c: Color) -> bool {
        self.under_threat(Piece::new(PieceTy::King, c))
    }

    #[allow(unused)]
    pub fn get_pieces_color(&self, c: Color) -> Vec<(Piece, Pos)> {
        self.get_all_pieces()
            .into_iter()
            .filter(|(p, _)| p.color == c)
            .collect()
    }

    pub fn get_all_pieces(&self) -> impl Iterator<Item = (Piece, Pos)> {
        self.board.iter().enumerate().flat_map(|(y, r)| {
            r.iter()
                .enumerate()
                .filter_map(move |(x, p)| p.map(|p| (p, (x, y))))
        })
    }

    pub fn under_threat_pos(&self, pos: Pos, by: Color) -> bool {
        let other = by;

        for x in (-2_isize..=-1).chain(1..=2) {
            for y in (-2_isize..=-1).chain(1..=2) {
                if !((x.abs() == 2) ^ (y.abs() == 2)) || !((x.abs() == 1) ^ (y.abs() == 1)) {
                    continue;
                }
                let oob = |a: isize, x: isize| a + x < 0 || a + x >= 8;
                if oob(pos.0 as isize, x) || oob(pos.1 as isize, y) {
                    continue;
                }

                let pos = ((pos.0 as isize + x) as usize, (pos.1 as isize + y) as usize);

                if self
                    .get(pos)
                    .is_some_and(|p| p.ty == PieceTy::Knight && p.color == other)
                {
                    return true;
                }
            }
        }

        // bishop
        for dir in [(1, 1), (-1, 1), (-1, -1), (1, -1)] {
            let mut pos = pos;
            loop {
                let oob = |a: isize, x: isize| a + x < 0 || a + x >= 8;
                if oob(pos.0 as isize, dir.0) || oob(pos.1 as isize, dir.1) {
                    break;
                }

                pos = (
                    (pos.0 as isize + dir.0) as usize,
                    (pos.1 as isize + dir.1) as usize,
                );

                if let Some(p) = self.get(pos) {
                    if (p.ty == PieceTy::Bishop || p.ty == PieceTy::Queen) && p.color == other {
                        return true;
                    }
                    break;
                }
            }
        }

        // rook
        for dir in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let mut pos = pos;
            loop {
                let oob = |a: isize, x: isize| a + x < 0 || a + x >= 8;
                if oob(pos.0 as isize, dir.0) || oob(pos.1 as isize, dir.1) {
                    break;
                }

                pos = (
                    (pos.0 as isize + dir.0) as usize,
                    (pos.1 as isize + dir.1) as usize,
                );

                if let Some(p) = self.get(pos) {
                    if (p.ty == PieceTy::Rook || p.ty == PieceTy::Queen) && p.color == other {
                        return true;
                    }
                    break;
                }
            }
        }

        // pawn
        {
            let y_dir = other.to_int();
            let y = pos.1 as isize + y_dir;
            if y >= 0 && y <= 7 {
                let check = |x: isize| {
                    x >= 0
                        && x <= 7
                        && self
                            .get((x as usize, y as usize))
                            .is_some_and(|p| p == Piece::new(PieceTy::Pawn, other))
                };

                if check(pos.0 as isize + 1) || check(pos.1 as isize - 1) {
                    return true;
                }
            }
        }

        false
    }

    pub fn under_threat(&self, p: Piece) -> bool {
        let Some(pos) = self.get_piece_pos(p) else {
            return false;
        };

        self.under_threat_pos(pos, p.color.other())
    }

    pub fn checkmate(&self, c: Color) -> bool {
        self.check(c) && self.get_all_moves(c).find_any(|_| true).is_none()
    }

    pub fn stalemate(&self, c: Color) -> bool {
        !self.check(c) && self.get_all_moves(c).find_any(|_| true).is_none()
    }

    #[inline]
    pub fn is_capture(&self, m: Move) -> bool {
        if self.occupied(m.1) {
            return true;
        }

        // check for en passant
        if let Some(p) = self.get(m.0)
            && p.ty == PieceTy::Pawn
            && m.0.0 != m.1.0
        {
            return true;
        }

        false
    }

    pub fn get_all_moves(&self, c: Color) -> impl ParallelIterator<Item = Move> {
        self.board
            .par_iter()
            .enumerate()
            .flat_map(|(y, row)| row.par_iter().enumerate().map(move |(x, p)| ((x, y), p)))
            .filter_map(|(pos, p)| p.map(|p| (pos, p)))
            .filter(move |(_, p)| p.color == c)
            .map(|(pos, piece)| ((pos.0 as isize, pos.1 as isize), piece))
            .flat_map(move |(pos, piece)| {
                match piece.ty {
                    PieceTy::Pawn => {
                        let pawn_move_dir = c.to_int();
                        let y = pos.1 + pawn_move_dir;

                        vec![
                            (pos.0, y),
                            (pos.0, y + pawn_move_dir), // move 2 tiles
                            (pos.0 - 1, y),
                            (pos.1 + 1, y),
                        ]
                    }
                    PieceTy::King => {
                        vec![
                            (pos.0, pos.1 + 1),
                            (pos.0, pos.1 - 1),
                            (pos.0 + 1, pos.1),
                            (pos.0 - 1, pos.1),
                            (pos.0 + 1, pos.1 + 1),
                            (pos.0 - 1, pos.1 + 1),
                            (pos.0 + 1, pos.1 - 1),
                            (pos.0 - 1, pos.1 - 1),
                            (pos.0 + 2, pos.1),
                            (pos.0 - 2, pos.1),
                        ]
                    }
                    PieceTy::Bishop => (0..8)
                        .flat_map(|delta| {
                            [
                                (delta, delta + pos.1 - pos.0),
                                (pos.1 + pos.0 - delta, delta),
                            ]
                        })
                        .collect(),
                    PieceTy::Rook => (0..8)
                        .flat_map(|delta| [(delta, pos.1), (pos.0, delta)])
                        .collect(),
                    PieceTy::Queen => (0..8)
                        .flat_map(|delta| {
                            [
                                (delta, delta + pos.1 - pos.0),
                                (pos.1 + pos.0 - delta, delta),
                                (delta, pos.1),
                                (pos.0, delta),
                            ]
                        })
                        .collect(),
                    PieceTy::Knight => vec![
                        (pos.0 - 1, pos.1 + 2),
                        (pos.0 + 1, pos.1 + 2),
                        (pos.0 + 2, pos.1 + 1),
                        (pos.0 + 2, pos.1 - 1),
                        (pos.0 + 1, pos.1 - 2),
                        (pos.0 - 1, pos.1 - 2),
                        (pos.0 - 2, pos.1 - 1),
                        (pos.0 - 2, pos.1 + 1),
                    ],
                }
                .into_iter()
                .map(move |d| (pos, d))
                .collect::<Vec<_>>()
            })
            .filter_map(|m| {
                let v = |n| (n >= 0 && n <= 7).then_some(n as usize);

                Some(((v(m.0.0)?, v(m.0.1)?), (v(m.1.0)?, v(m.1.1)?)))
            })
            .filter(|m| self.is_valid(*m))
    }

    pub fn get_piece_pos(&self, piece: Piece) -> Option<Pos> {
        self.board.iter().enumerate().find_map(|(y, r)| {
            r.iter()
                .enumerate()
                .find_map(|(x, p)| p.and_then(|p| (p == piece).then_some((x, y))))
        })
    }

    #[inline]
    pub fn get(&self, p: Pos) -> Option<Piece> {
        self.board[p.1][p.0]
    }

    pub fn move_change(mut self, m: Move) -> Self {
        self.move_piece(m);
        self
    }
}

impl Default for Game {
    fn default() -> Self {
        let white_pawn = Piece::new(PieceTy::Pawn, Color::White);
        let white_front = [Some(white_pawn); 8];

        let white_rook = Piece::new(PieceTy::Rook, Color::White);
        let white_knight = Piece::new(PieceTy::Knight, Color::White);
        let white_bishop = Piece::new(PieceTy::Bishop, Color::White);
        let white_back = [
            Some(white_rook),
            Some(white_knight),
            Some(white_bishop),
            Some(Piece::new(PieceTy::Queen, Color::White)),
            Some(Piece::new(PieceTy::King, Color::White)),
            Some(white_bishop),
            Some(white_knight),
            Some(white_rook),
        ];

        let middle = [None; 8];

        let black_pawn = Piece::new(PieceTy::Pawn, Color::Black);
        let black_front = [Some(black_pawn); 8];

        let black_rook = Piece::new(PieceTy::Rook, Color::Black);
        let black_knight = Piece::new(PieceTy::Knight, Color::Black);
        let black_bishop = Piece::new(PieceTy::Bishop, Color::Black);
        let black_back = [
            Some(black_rook),
            Some(black_knight),
            Some(black_bishop),
            Some(Piece::new(PieceTy::Queen, Color::Black)),
            Some(Piece::new(PieceTy::King, Color::Black)),
            Some(black_bishop),
            Some(black_knight),
            Some(black_rook),
        ];

        Self::new(
            [
                white_back,
                white_front,
                middle,
                middle,
                middle,
                middle,
                black_front,
                black_back,
            ],
            Color::White,
        )
    }
}
