use rand::{prelude::*, rngs::SmallRng};
use std::sync::LazyLock;

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

impl PieceTy {
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
    pub fn to_int(self) -> i64 {
        match self {
            Self::White => 1,
            Self::Black => -1,
        }
    }

    #[inline]
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

type Board = [[Square; 8]; 8];

static ZOBRIST_TABLE: LazyLock<[[[u64; 16]; 8]; 8]> = LazyLock::new(|| {
    let mut rng = SmallRng::from_seed(*b"andrewrobsonlovespenis1234567890");
    let mut z = [[[0u64; 16]; 8]; 8];
    for y in 0..8 {
        for x in 0..8 {
            for p in 0..16 {
                z[y][x][p] = rng.next_u64();
            }
        }
    }
    z
});
static ZOBRIST_SIDE_TO_MOVE: LazyLock<u64> =
    LazyLock::new(|| SmallRng::from_seed(*b"sideblck00000000000000000000000a").next_u64());

static ZOBRIST_CASTLING: LazyLock<[[u64; 2]; 2]> = LazyLock::new(|| {
    let mut rng = SmallRng::from_seed(*b"castling00000000000000000000000b");
    [
        [rng.next_u64(), rng.next_u64()],
        [rng.next_u64(), rng.next_u64()],
    ]
});

static ZOBRIST_ENPASS: LazyLock<[u64; 8]> = LazyLock::new(|| {
    let mut rng = SmallRng::from_seed(*b"enpassnt00000000000000000000000c");
    std::array::from_fn(|_| rng.next_u64())
});

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
    capture: Option<(Square, Pos)>,
    castleable: [(bool, bool); 2],
    en_pass: Option<Pos>,
    hash: u64,
    m: Move,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    board: Board,
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
            h ^= *ZOBRIST_SIDE_TO_MOVE;
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
            h ^= ZOBRIST_ENPASS[file];
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

        // TODO: use new
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

    #[inline]
    pub fn get_rank(&self, n: usize) -> &[Square; 8] {
        &self.board[n]
    }

    pub fn move_piece(&mut self, m: Move) {
        if m.from == m.to {
            return;
        }

        let mut undo = Undo {
            m,
            hash: self.get_hash(),
            en_pass: self.enpass,
            castleable: self.castleable,
            capture: (!self.board[m.to.1][m.to.0].is_empty())
                .then_some((self.board[m.to.1][m.to.0], m.to)),
        };

        self.prev_pos.push(self.get_hash());

        let moving = self.board[m.from.1][m.from.0];
        let moving_color_index = moving.color().to_index();

        self.hash ^= ZOBRIST_TABLE[m.from.1][m.from.0][moving.to_usize()];

        if moving.ty() == PieceTy::Pawn && Some(m.to) == self.enpass {
            let target = (m.to.0, m.from.1);
            undo.capture = Some((self.get(target), target));
            self.hash ^= ZOBRIST_TABLE[m.from.1][m.to.0][self.board[m.from.1][m.to.0].to_usize()];
            self.board[target.1][target.0] = Square::EMPTY; // y of start, x of end
        }
        if let Some(e) = self.enpass {
            self.hash ^= ZOBRIST_ENPASS[e.0];
        }
        self.enpass = None;
        if moving.ty() == PieceTy::Pawn && (m.from.1.abs_diff(m.to.1) == 2) {
            let mid_y = (m.from.1 + m.to.1) / 2;
            self.enpass = Some((m.to.0, mid_y));
            self.hash ^= ZOBRIST_ENPASS[m.to.0];
        }

        let cas = [
            (self.castleable[moving_color_index], moving_color_index),
            (
                self.castleable[moving.color().other().to_index()],
                moving.color().other().to_index(),
            ),
        ];
        if moving.ty() == PieceTy::King {
            self.castleable[moving_color_index] = (false, false);
        }
        if moving.ty() == PieceTy::Rook && m.from.1 == moving_color_index * 7 {
            if m.from.0 == 0 {
                self.castleable[moving_color_index].0 = false;
            } else if m.from.0 == 7 {
                self.castleable[moving_color_index].1 = false;
            }
        }
        if let rook = self.get(m.to)
            && rook != Square::EMPTY
            && m.to.1 == rook.color().start()
            && rook.ty() == PieceTy::Rook
        {
            if m.to.0 == 0 {
                self.castleable[moving.color().other().to_index()].0 = false;
            } else if m.to.0 == 7 {
                self.castleable[moving.color().other().to_index()].1 = false;
            }
        }

        for (cas, idx) in cas {
            if self.castleable[idx].0 != cas.0 {
                self.hash ^= ZOBRIST_CASTLING[idx][0];
            }
            if self.castleable[idx].1 != cas.1 {
                self.hash ^= ZOBRIST_CASTLING[idx][1];
            }
        }

        if moving.ty() == PieceTy::King && m.from.0.abs_diff(m.to.0) == 2 {
            // castling
            // we move the rook too
            let (rook_x, rook_final) = if m.from.0 > m.to.0 { (0, 3) } else { (7, 5) };
            let rook = self.board[m.from.1][rook_x];
            self.board[m.from.1][rook_final] = self.board[m.from.1][rook_x];
            self.board[m.from.1][rook_x] = Square::EMPTY;
            self.hash ^= ZOBRIST_TABLE[m.from.1][rook_final][rook.to_usize()];
            self.hash ^= ZOBRIST_TABLE[m.from.1][rook_x][rook.to_usize()];
        }

        if let captured = self.board[m.to.1][m.to.0]
            && !captured.is_empty()
        {
            self.hash ^= ZOBRIST_TABLE[m.to.1][m.to.0][captured.to_usize()];
        }

        self.board[m.to.1][m.to.0] = self.board[m.from.1][m.from.0];
        self.board[m.from.1][m.from.0] = Square::EMPTY;

        if let Some(p) = m.promotion {
            let promoted = Square::piece(p, moving.color());
            self.hash ^= ZOBRIST_TABLE[m.to.1][m.to.0][promoted.to_usize()];
            self.board[m.to.1][m.to.0] = promoted;
        } else {
            self.hash ^= ZOBRIST_TABLE[m.to.1][m.to.0][moving.to_usize()];
        }

        self.hash ^= *ZOBRIST_SIDE_TO_MOVE;

        self.to_move = self.to_move.other();

        self.moves.push(undo);
    }

    pub fn undo_move(&mut self) {
        let undo = self.moves.pop().unwrap();

        self.to_move = self.to_move.other();
        self.hash = undo.hash;
        self.castleable = undo.castleable;
        self.enpass = undo.en_pass;

        if self.get(undo.m.to).ty() == PieceTy::King && undo.m.from.0.abs_diff(undo.m.to.0) == 2 {
            let (rook_now, rook_origin) = if undo.m.from.0 > undo.m.to.0 {
                (3, 0)
            } else {
                (5, 7)
            };
            self.board[undo.m.from.1][rook_origin] = self.board[undo.m.from.1][rook_now];
            self.board[undo.m.from.1][rook_now] = Square::EMPTY;
        }

        self.board[undo.m.from.1][undo.m.from.0] = if let Some(p) = undo.m.promotion {
            Square::piece(p, self.get(undo.m.to).color())
        } else {
            self.get(undo.m.to)
        };
        self.board[undo.m.to.1][undo.m.to.0] = Square::EMPTY;
        if let Some(cap) = undo.capture {
            self.board[cap.1.1][cap.1.0] = cap.0;
        }

        self.prev_pos.undo();
    }

    pub fn move_piece_board(&self, m: Move, b: &mut Board) {
        if m.from == m.to {
            return;
        }

        let moving = b[m.from.1][m.from.0];

        if moving.ty() == PieceTy::Pawn && Some(m.to) == self.enpass {
            b[m.from.1][m.to.0] = Square::EMPTY; // y of start, x of end
        }

        if moving.ty() == PieceTy::King && m.from.0.abs_diff(m.to.0) == 2 {
            let (rook_x, rook_final) = if m.from.0 > m.to.0 { (0, 3) } else { (7, 5) };
            b[m.from.1][rook_final] = b[m.from.1][rook_x];
            b[m.from.1][rook_x] = Square::EMPTY;
        }

        if let Some(p) = m.promotion {
            b[m.to.1][m.to.0] = Square::piece(p, moving.color());
        } else {
            b[m.to.1][m.to.0] = b[m.from.1][m.from.0];
        }
        b[m.from.1][m.from.0] = Square::EMPTY;
    }

    #[inline]
    pub fn occupied(&self, p: Pos) -> bool {
        !self.board[p.1][p.0].is_empty()
    }

    pub fn is_valid(&self, m: Move) -> bool {
        let start = m.from;
        let end = m.to;

        if start == end {
            return false;
        }

        let piece = self.board[start.1][start.0];
        if piece.is_empty() {
            return false;
        }

        if let p = self.get(end)
            && p != Square::EMPTY
            && (p.ty() == PieceTy::King || p.color() == piece.color())
        {
            return false;
        }

        match piece.ty() {
            PieceTy::Bishop => {
                let x_change = end.0 as i8 - start.0 as i8;
                let y_change = end.1 as i8 - start.1 as i8;

                if x_change.abs() != y_change.abs() {
                    return false;
                }

                // one less as we don't care whether the target is occupied
                for delta in 1..(x_change.abs()) {
                    let dx = delta * x_change.signum();
                    let dy = delta * y_change.signum();

                    let pos = ((start.0 as i8 + dx) as usize, (start.1 as i8 + dy) as usize);

                    if pos.0 >= 8 || pos.1 >= 8 || self.occupied(pos) {
                        return false;
                    }
                }
            }
            PieceTy::Pawn => {
                let move_dir = piece.color().to_int() as i8;

                let occupied = self.occupied(end);

                let front = match piece.color() {
                    Color::Black => 6,
                    Color::White => 1,
                };
                let can_be_double = start.1 == front;

                if start.1 as i8 + move_dir != end.1 as i8 {
                    if !(can_be_double && start.1 as i8 + move_dir * 2 == end.1 as i8)
                        || self.occupied((start.0, (start.1 as i8 + move_dir) as usize))
                    {
                        return false;
                    }
                }

                let en_pass = !occupied
                    && Some(end) == self.enpass
                    && self.get((end.0, start.1)).color() == piece.color().other();

                let x_dist = start.0.abs_diff(end.0);
                if x_dist == 0 {
                    if occupied {
                        return false; // blocked
                    }
                    // straight push — valid, fall through
                } else if x_dist == 1 {
                    if !en_pass && !occupied {
                        return false; // diagonal must be a capture
                    }
                } else {
                    return false; // can't move sideways more than 1
                }
            }
            PieceTy::Rook => {
                let vertical = start.1 != end.1;
                let horizontal = start.0 != end.0;

                if !(vertical ^ horizontal) {
                    return false;
                }

                let man_dist = (start.1.abs_diff(end.1) + start.0.abs_diff(end.0)) as i8;
                let neg = start.1 > end.1 || start.0 > end.0;

                // one less as we don't care whether the target is occupied
                for delta in 1..man_dist {
                    let delta = if neg { -delta } else { delta };
                    let pos = if vertical {
                        (start.0, (start.1 as i8 + delta) as usize)
                    } else {
                        ((start.0 as i8 + delta) as usize, start.1)
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
                    let color_idx = piece.color().to_index();
                    let (legal, rook_x_ish, rook_final) = if m.from.0 > m.to.0 {
                        (self.castleable[color_idx].0, 1, 3)
                    } else {
                        (self.castleable[color_idx].1, 6, 5)
                    };
                    if !legal {
                        return false;
                    }

                    if self.check(piece.color()) {
                        return false;
                    }

                    let empty_start = rook_final.min(rook_x_ish);
                    let empty_end = rook_final + rook_x_ish - empty_start;
                    for x in empty_start..=empty_end {
                        if self.occupied((x, m.from.1)) {
                            return false;
                        }
                    }

                    let safe_start = m.to.0.min(rook_final);
                    let safe_end = m.to.0.max(rook_final);
                    for x in safe_start..=safe_end {
                        if self.under_threat_pos((x, m.from.1), piece.color().other()) {
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

                    let man_dist = (start.1.abs_diff(end.1) + start.0.abs_diff(end.0)) as i8;
                    let neg = start.1 > end.1 || start.0 > end.0;

                    // one less as we don't care whether the target is occupied
                    for delta in 1..man_dist {
                        let delta = if neg { -delta } else { delta };
                        let pos = if vertical {
                            (start.0, (start.1 as i8 + delta) as usize)
                        } else {
                            ((start.0 as i8 + delta) as usize, start.1)
                        };

                        if pos.0 >= 8 || pos.1 >= 8 || self.occupied(pos) {
                            return false;
                        }
                    }
                } else {
                    // bishop-like movement
                    let x_change = end.0 as i8 - start.0 as i8;
                    let y_change = end.1 as i8 - start.1 as i8;

                    if x_change.abs() != y_change.abs() {
                        return false;
                    }

                    // one less as we don't care whether the target is occupied
                    for delta in 1..(x_change.abs()) {
                        let dx = delta * x_change.signum();
                        let dy = delta * y_change.signum();

                        let pos = ((start.0 as i8 + dx) as usize, (start.1 as i8 + dy) as usize);

                        if pos.0 >= 8 || pos.1 >= 8 || self.occupied(pos) {
                            return false;
                        }
                    }
                }
            }
        }

        let mut temp_board = self.board;
        self.move_piece_board(m, &mut temp_board);

        let king_pos = if piece.ty() == PieceTy::King {
            m.to
        } else {
            self.get_piece_pos(Square::piece(PieceTy::King, piece.color()))
                .unwrap()
        };

        !Self::under_threat_pos_board(&temp_board, king_pos, piece.color().other())
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
        self.board
            .as_flattened()
            .iter()
            .enumerate()
            .filter_map(|(i, p)| (!p.is_empty()).then(|| (*p, (i % 8, i / 8))))
    }

    fn under_threat_pos_board(board: &Board, pos: Pos, by: Color) -> bool {
        let other = by;

        let get = |p: Pos| board[p.1][p.0];

        // pawn
        {
            let y_dir = other.other().to_int();
            let y = pos.1 as i64 + y_dir;
            if y >= 0 && y <= 7 {
                let check = |x: isize| {
                    x >= 0
                        && x <= 7
                        && get((x as usize, y as usize)) == Square::piece(PieceTy::Pawn, other)
                };

                if check(pos.0 as isize + 1) || check(pos.0 as isize - 1) {
                    return true;
                }
            }
        }

        const KNIGHT_DELTAS: [(isize, isize); 8] = [
            (2, 1),
            (2, -1),
            (-2, 1),
            (-2, -1),
            (1, 2),
            (1, -2),
            (-1, 2),
            (-1, -2),
        ];

        for (dx, dy) in KNIGHT_DELTAS {
            let nx = pos.0 as isize + dx;
            let ny = pos.1 as isize + dy;
            if nx < 0 || nx >= 8 || ny < 0 || ny >= 8 {
                continue;
            }
            if let p = get((nx as usize, ny as usize))
                && p != Square::EMPTY
                && p.ty() == PieceTy::Knight
                && p.color() == other
            {
                return true;
            }
        }

        let oob = |a: isize, x: isize| a + x < 0 || a + x >= 8;
        // bishop
        for dir in [(1, 1), (-1, 1), (-1, -1), (1, -1)] {
            let mut pos = pos;
            loop {
                if oob(pos.0 as isize, dir.0) || oob(pos.1 as isize, dir.1) {
                    break;
                }

                pos = (
                    (pos.0 as isize + dir.0) as usize,
                    (pos.1 as isize + dir.1) as usize,
                );

                if let p = get(pos)
                    && p != Square::EMPTY
                {
                    if (p.ty() == PieceTy::Bishop || p.ty() == PieceTy::Queen) && p.color() == other
                    {
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
                if oob(pos.0 as isize, dir.0) || oob(pos.1 as isize, dir.1) {
                    break;
                }

                pos = (
                    (pos.0 as isize + dir.0) as usize,
                    (pos.1 as isize + dir.1) as usize,
                );

                if let p = get(pos)
                    && p != Square::EMPTY
                {
                    if (p.ty() == PieceTy::Rook || p.ty() == PieceTy::Queen) && p.color() == other {
                        return true;
                    }
                    break;
                }
            }
        }

        for delta in [
            (1, 1),
            (1, 0),
            (1, -1),
            (0, -1),
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, 1),
        ] {
            let p = (pos.0 as isize + delta.0, pos.1 as isize + delta.1);
            if p.0 < 0 || p.0 > 7 || p.1 < 0 || p.1 > 7 {
                continue;
            }
            let p = (p.0 as usize, p.1 as usize);
            if let piece = get(p)
                && !piece.is_empty()
                && piece.color() == other
                && piece.ty() == PieceTy::King
            {
                return true;
            }
        }

        false
    }

    #[inline]
    pub fn under_threat_pos(&self, pos: Pos, by: Color) -> bool {
        Self::under_threat_pos_board(&self.board, pos, by)
    }

    #[inline]
    pub fn under_threat(&self, p: Square) -> bool {
        let Some(pos) = self.get_piece_pos(p) else {
            return false;
        };

        self.under_threat_pos(pos, p.color().other())
    }

    pub fn checkmate(&self, c: Color) -> bool {
        self.check(c) && self.get_all_moves(c).find(|_| true).is_none() && !self.lose_on_repeat()
    }

    pub fn stalemate(&self, c: Color) -> bool {
        let not_in_check = !self.check(c);
        let repeat = self.lose_on_repeat();
        let no_moves = self.get_all_moves(c).find(|_| true).is_none();

        (not_in_check && no_moves) || repeat
    }

    #[inline]
    pub fn is_capture(&self, m: Move) -> bool {
        if self.occupied(m.to) {
            return true;
        }

        // check for en passant
        if let p = self.get(m.from)
            && p.ty() == PieceTy::Pawn
            && m.from.0 != m.to.0
        {
            return true;
        }

        false
    }

    pub fn get_all_moves(&self, c: Color) -> impl Iterator<Item = Move> {
        self.get_all_pseudo_moves(c).filter(|m| self.is_valid(*m))
    }

    pub fn get_all_pseudo_moves(&self, c: Color) -> impl Iterator<Item = Move> {
        self.board
            .iter()
            .enumerate()
            .flat_map(|(y, row)| row.iter().enumerate().map(move |(x, p)| ((x, y), p)))
            .filter_map(|(pos, p)| (!p.is_empty()).then(|| (pos, p)))
            .filter(move |(_, p)| p.color() == c)
            .map(|(pos, piece)| ((pos.0 as isize, pos.1 as isize), piece))
            .flat_map(move |(pos, piece)| {
                match piece.ty() {
                    PieceTy::Pawn => {
                        let pawn_move_dir = c.to_int() as isize;
                        let y = pos.1 + pawn_move_dir;

                        [
                            (pos.0, y, None),
                            (pos.0, y, Some(PieceTy::Queen)),
                            (pos.0, y, Some(PieceTy::Rook)),
                            (pos.0, y, Some(PieceTy::Knight)),
                            (pos.0, y, Some(PieceTy::Bishop)),
                            (pos.0, y + pawn_move_dir, None), // move 2 tiles
                            (pos.0 - 1, y, None),
                            (pos.0 + 1, y, None),
                        ]
                        .into_iter()
                        .filter(|(_, y, p)| p.is_some() ^ (*y != 0 && *y != 7))
                        .collect()
                    }
                    PieceTy::King => {
                        vec![
                            (pos.0, pos.1 + 1, None),
                            (pos.0, pos.1 - 1, None),
                            (pos.0 + 1, pos.1, None),
                            (pos.0 - 1, pos.1, None),
                            (pos.0 + 1, pos.1 + 1, None),
                            (pos.0 - 1, pos.1 + 1, None),
                            (pos.0 + 1, pos.1 - 1, None),
                            (pos.0 - 1, pos.1 - 1, None),
                            (pos.0 + 2, pos.1, None),
                            (pos.0 - 2, pos.1, None),
                        ]
                    }
                    PieceTy::Bishop => (0..8)
                        .flat_map(|delta| {
                            [
                                (delta, delta + pos.1 - pos.0, None),
                                (pos.1 + pos.0 - delta, delta, None),
                            ]
                        })
                        .collect(),
                    PieceTy::Rook => (0..8)
                        .flat_map(|delta| [(delta, pos.1, None), (pos.0, delta, None)])
                        .collect(),
                    PieceTy::Queen => (0..8)
                        .flat_map(|delta| {
                            [
                                (delta, delta + pos.1 - pos.0, None),
                                (pos.1 + pos.0 - delta, delta, None),
                                (delta, pos.1, None),
                                (pos.0, delta, None),
                            ]
                        })
                        .collect(),
                    PieceTy::Knight => vec![
                        (pos.0 - 1, pos.1 + 2, None),
                        (pos.0 + 1, pos.1 + 2, None),
                        (pos.0 + 2, pos.1 + 1, None),
                        (pos.0 + 2, pos.1 - 1, None),
                        (pos.0 + 1, pos.1 - 2, None),
                        (pos.0 - 1, pos.1 - 2, None),
                        (pos.0 - 2, pos.1 - 1, None),
                        (pos.0 - 2, pos.1 + 1, None),
                    ],
                }
                .into_iter()
                .map(move |d| (pos, (d.0, d.1), d.2))
                .collect::<Vec<_>>()
            })
            .filter_map(|m| {
                let v = |n| (n >= 0 && n <= 7).then_some(n as usize);

                Some(((v(m.0.0)?, v(m.0.1)?), (v(m.1.0)?, v(m.1.1)?), m.2))
            })
            .map(|m| Move {
                from: m.0,
                to: m.1,
                promotion: m.2,
            })
    }

    pub fn get_piece_pos(&self, piece: Square) -> Option<Pos> {
        self.board.iter().enumerate().find_map(|(y, r)| {
            r.iter()
                .enumerate()
                .find_map(|(x, p)| (*p == piece).then_some((x, y)))
        })
    }

    #[inline]
    pub fn get(&self, p: Pos) -> Square {
        self.board[p.1][p.0]
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    }
}
