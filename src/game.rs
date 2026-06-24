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
    pub fn from_u8(n: u8) -> Self {
        match n {
            1 => Color::Black,
            _ => Color::White,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn to_int(self) -> i64 {
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

    pub fn start(self) -> usize {
        match self {
            Self::White => 0,
            Self::Black => 7,
        }
    }
}

pub type Pos = (usize, usize);
pub type Move = (Pos, Pos);

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
pub struct Game {
    board: Board,
    to_move: Color,
    castleable: [(bool, bool); 2],
    enpass: Option<Pos>,
    #[allow(unused)]
    full_move_clock: u32, // TODO: implement

    previous_positions: [u64; 256],
    prev_pos_idx: usize,
    hash: u64,
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
            previous_positions: [0; 256],
            prev_pos_idx: 0,
            full_move_clock: 0,
            hash,
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
        self.previous_positions.contains(&g.get_hash())
    }

    pub fn lose_on_repeat(&self) -> bool {
        self.previous_positions
            .iter()
            .take_while(|&&n| n != 0)
            .filter(|&&n| n == self.get_hash())
            .count()
            >= 2
    }

    #[inline]
    pub fn get_rank(&self, n: usize) -> &[Square; 8] {
        &self.board[n]
    }

    fn move_hash(&self, m: Move) -> u64 {
        if m.0 == m.1 {
            panic!("invalid move - cannot calculate hash");
        }

        let mut hash = self.hash;

        let moving = self.board[m.0.1][m.0.0];
        let moving_color_index = moving.color().to_index();

        // GOOD
        hash ^= ZOBRIST_TABLE[m.0.1][m.0.0][moving.to_usize()];

        if moving.ty() == PieceTy::Pawn && Some(m.1) == self.enpass {
            // GOOD
            hash ^= ZOBRIST_TABLE[m.0.1][m.1.0][self.board[m.0.1][m.1.0].to_usize()];
        }
        if let Some(e) = self.enpass {
            // GOOD
            hash ^= ZOBRIST_ENPASS[e.0];
        }
        if moving.ty() == PieceTy::Pawn && (m.0.1.abs_diff(m.1.1) == 2) {
            // GOOD
            hash ^= ZOBRIST_ENPASS[m.1.0];
        }

        let cas = [
            (self.castleable[moving_color_index], moving_color_index),
            (
                self.castleable[moving.color().other().to_index()],
                moving.color().other().to_index(),
            ),
        ];
        let mut cas_change = self.castleable;
        if moving.ty() == PieceTy::King {
            cas_change[moving_color_index] = (false, false);
        }
        if moving.ty() == PieceTy::Rook && m.0.1 == moving_color_index * 7 {
            if m.0.0 == 0 {
                cas_change[moving_color_index].0 = false;
            } else if m.0.0 == 7 {
                cas_change[moving_color_index].1 = false;
            }
        }
        if let rook = self.get(m.1)
            && rook != Square::EMPTY
            && m.1.1 == rook.color().start()
            && rook.ty() == PieceTy::Rook
        {
            if m.1.0 == 0 {
                cas_change[moving.color().other().to_index()].0 = false;
            } else if m.1.0 == 7 {
                cas_change[moving.color().other().to_index()].1 = false;
            }
        }

        for (cas, idx) in cas {
            if cas_change[idx].0 != cas.0 {
                hash ^= ZOBRIST_CASTLING[idx][0];
            }
            if cas_change[idx].1 != cas.1 {
                hash ^= ZOBRIST_CASTLING[idx][1];
            }
        }

        if moving.ty() == PieceTy::King && m.0.0.abs_diff(m.1.0) == 2 {
            // castling
            // we move the rook too
            let (rook_x, rook_final) = if m.0.0 > m.1.0 { (0, 3) } else { (7, 5) };
            let rook = self.board[m.0.1][rook_x];
            hash ^= ZOBRIST_TABLE[m.0.1][rook_final][rook.to_usize()];
            hash ^= ZOBRIST_TABLE[m.0.1][rook_x][rook.to_usize()];
        }

        if let captured = self.board[m.1.1][m.1.0]
            && !captured.is_empty()
        {
            hash ^= ZOBRIST_TABLE[m.1.1][m.1.0][captured.to_usize()];
        }

        hash ^= ZOBRIST_TABLE[m.1.1][m.1.0][moving.to_usize()];
        hash ^= *ZOBRIST_SIDE_TO_MOVE;

        hash
    }

    pub fn move_piece(&mut self, m: Move) {
        if m.0 == m.1 {
            return;
        }

        self.previous_positions[self.prev_pos_idx] = self.get_hash();
        self.prev_pos_idx += 1;

        let moving = self.board[m.0.1][m.0.0];
        let moving_color_index = moving.color().to_index();

        self.hash ^= ZOBRIST_TABLE[m.0.1][m.0.0][moving.to_usize()];

        if moving.ty() == PieceTy::Pawn && Some(m.1) == self.enpass {
            self.hash ^= ZOBRIST_TABLE[m.0.1][m.1.0][self.board[m.0.1][m.1.0].to_usize()];
            self.board[m.0.1][m.1.0] = Square::EMPTY; // y of start, x of end
        }
        if let Some(e) = self.enpass {
            self.hash ^= ZOBRIST_ENPASS[e.0];
        }
        self.enpass = None;
        if moving.ty() == PieceTy::Pawn && (m.0.1.abs_diff(m.1.1) == 2) {
            let mid_y = (m.0.1 + m.1.1) / 2;
            self.enpass = Some((m.1.0, mid_y));
            self.hash ^= ZOBRIST_ENPASS[m.1.0];
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
        if moving.ty() == PieceTy::Rook && m.0.1 == moving_color_index * 7 {
            if m.0.0 == 0 {
                self.castleable[moving_color_index].0 = false;
            } else if m.0.0 == 7 {
                self.castleable[moving_color_index].1 = false;
            }
        }
        if let rook = self.get(m.1)
            && rook != Square::EMPTY
            && m.1.1 == rook.color().start()
            && rook.ty() == PieceTy::Rook
        {
            if m.1.0 == 0 {
                self.castleable[moving.color().other().to_index()].0 = false;
            } else if m.1.0 == 7 {
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

        if moving.ty() == PieceTy::King && m.0.0.abs_diff(m.1.0) == 2 {
            // castling
            // we move the rook too
            let (rook_x, rook_final) = if m.0.0 > m.1.0 { (0, 3) } else { (7, 5) };
            let rook = self.board[m.0.1][rook_x];
            self.board[m.0.1][rook_final] = self.board[m.0.1][rook_x];
            self.board[m.0.1][rook_x] = Square::EMPTY;
            self.hash ^= ZOBRIST_TABLE[m.0.1][rook_final][rook.to_usize()];
            self.hash ^= ZOBRIST_TABLE[m.0.1][rook_x][rook.to_usize()];
        }

        if let captured = self.board[m.1.1][m.1.0]
            && !captured.is_empty()
        {
            self.hash ^= ZOBRIST_TABLE[m.1.1][m.1.0][captured.to_usize()];
        }

        self.board[m.1.1][m.1.0] = self.board[m.0.1][m.0.0];
        self.board[m.0.1][m.0.0] = Square::EMPTY;

        self.hash ^= ZOBRIST_TABLE[m.1.1][m.1.0][moving.to_usize()];

        self.hash ^= *ZOBRIST_SIDE_TO_MOVE;

        self.to_move = self.to_move.other();
    }

    pub fn move_piece_board(&self, m: Move, b: &mut Board) {
        if m.0 == m.1 {
            return;
        }

        let moving = b[m.0.1][m.0.0];

        if moving.ty() == PieceTy::Pawn && Some(m.1) == self.enpass {
            b[m.0.1][m.1.0] = Square::EMPTY; // y of start, x of end
        }

        if moving.ty() == PieceTy::King && m.0.0.abs_diff(m.1.0) == 2 {
            let (rook_x, rook_final) = if m.0.0 > m.1.0 { (0, 3) } else { (7, 5) };
            b[m.0.1][rook_final] = b[m.0.1][rook_x];
            b[m.0.1][rook_x] = Square::EMPTY;
        }

        b[m.1.1][m.1.0] = b[m.0.1][m.0.0];
        b[m.0.1][m.0.0] = Square::EMPTY;
    }

    #[inline]
    pub fn occupied(&self, p: Pos) -> bool {
        !self.board[p.1][p.0].is_empty()
    }

    pub fn is_valid(&self, m: Move) -> bool {
        let start = m.0;
        let end = m.1;

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
                let x_change = end.0 as isize - start.0 as isize;
                let y_change = end.1 as isize - start.1 as isize;

                if x_change.abs() != y_change.abs() {
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
            PieceTy::Pawn => {
                let move_dir = piece.color().to_int() as isize;

                let occupied = self.occupied(end);

                let front = match piece.color() {
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

                let man_dist = (start.1.abs_diff(end.1) + start.0.abs_diff(end.0)) as isize;
                let neg = start.1 > end.1 || start.0 > end.0;

                // one less as we don't care whether the target is occupied
                for delta in 1..man_dist {
                    let delta = if neg { -delta } else { delta };
                    let pos = if vertical {
                        (start.0, (start.1 as isize + delta) as usize)
                    } else {
                        ((start.0 as isize + delta) as usize, start.1)
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
                    let (legal, rook_x_ish, rook_final) = if m.0.0 > m.1.0 {
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
                        if self.occupied((x, m.0.1)) {
                            return false;
                        }
                    }

                    let safe_start = m.1.0.min(rook_final);
                    let safe_end = m.1.0.max(rook_final);
                    for x in safe_start..=safe_end {
                        if self.under_threat_pos((x, m.0.1), piece.color().other()) {
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

                    let man_dist = (start.1.abs_diff(end.1) + start.0.abs_diff(end.0)) as isize;
                    let neg = start.1 > end.1 || start.0 > end.0;

                    // one less as we don't care whether the target is occupied
                    for delta in 1..man_dist {
                        let delta = if neg { -delta } else { delta };
                        let pos = if vertical {
                            (start.0, (start.1 as isize + delta) as usize)
                        } else {
                            ((start.0 as isize + delta) as usize, start.1)
                        };

                        if pos.0 >= 8 || pos.1 >= 8 || self.occupied(pos) {
                            return false;
                        }
                    }
                } else {
                    // bishop-like movement
                    let x_change = end.0 as isize - start.0 as isize;
                    let y_change = end.1 as isize - start.1 as isize;

                    if x_change.abs() != y_change.abs() {
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

        let m_hash = self.move_hash(m);
        if self
            .previous_positions
            .iter()
            .take_while(|&&n| n != 0)
            .filter(|&&n| n == m_hash)
            .count()
            >= 2
        {
            return false;
        }

        let mut temp_board = self.board;
        self.move_piece_board(m, &mut temp_board);

        let king_pos = if piece.ty() == PieceTy::King {
            m.1
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
        self.board.iter().enumerate().flat_map(|(y, r)| {
            r.iter()
                .enumerate()
                .filter_map(move |(x, p)| (!p.is_empty()).then(|| (*p, (x, y))))
        })
    }

    fn under_threat_pos_board(board: &Board, pos: Pos, by: Color) -> bool {
        let other = by;

        let get = |p: Pos| board[p.1][p.0];

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
                let oob = |a: isize, x: isize| a + x < 0 || a + x >= 8;
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
        self.check(c) && self.get_all_moves(c).find(|_| true).is_none()
    }

    pub fn stalemate(&self, c: Color) -> bool {
        let ch = !self.check(c);
        let mo = self.get_all_moves(c).find(|_| true).is_none();

        ch && mo
    }

    #[inline]
    pub fn is_capture(&self, m: Move) -> bool {
        if self.occupied(m.1) {
            return true;
        }

        // check for en passant
        if let p = self.get(m.0)
            && p != Square::EMPTY
            && p.ty() == PieceTy::Pawn
            && m.0.0 != m.1.0
        {
            return true;
        }

        false
    }

    pub fn get_all_moves(&self, c: Color) -> impl Iterator<Item = Move> {
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

                        vec![
                            (pos.0, y),
                            (pos.0, y + pawn_move_dir), // move 2 tiles
                            (pos.0 - 1, y),
                            (pos.0 + 1, y),
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

    pub fn move_change(mut self, m: Move) -> Self {
        self.move_piece(m);
        self
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    }
}
