#![allow(unused)]

use rand::{prelude::*, rngs::SmallRng};

use std::{collections::HashMap, sync::LazyLock};

use crate::game::{BB, FILE_A, FILE_H, RANK_1, RANK_8, bit, pop_lsb};

type RookupTable = HashMap<(u32, BB), BB>;

fn create_rook_lookup_table() -> RookupTable {
    let mut table = HashMap::new();

    for n in 0..64 {
        let rank_part = super::RANK_MASKS[n] & !(FILE_A | FILE_H);
        let file_part = super::FILE_MASKS[n] & !(RANK_1 | RANK_8);
        let rook_mask = rank_part | file_part;

        get_all_blocker_patterns(rook_mask).for_each(|pat| {
            table.insert((n as u32, pat), create_rook_legal_move(n as u32, pat));
        })
    }

    table
}

fn get_all_blocker_patterns(mut mask: BB) -> impl Iterator<Item = BB> {
    let positions = std::iter::repeat_with(|| pop_lsb(&mut mask))
        .take_while(|&n| n < 64)
        .collect::<Vec<_>>();
    (0..2_u64.pow(positions.len() as u32)).map(move |mut n| {
        let mut num = 0u64;
        std::iter::repeat_with(|| pop_lsb(&mut n))
            .take_while(|&n| n < 64)
            .for_each(|b| num |= bit(positions[b as usize]));
        num
    })
}

fn create_rook_legal_move(sq: u32, blockers: BB) -> BB {
    hyperbola_quintessence(sq, blockers, super::RANK_MASKS[sq as usize])
        | hyperbola_quintessence(sq, blockers, super::FILE_MASKS[sq as usize])
}

#[inline]
fn hyperbola_quintessence(sq: u32, all: BB, mask: BB) -> BB {
    let o = all & mask;
    let s = bit(sq);

    let forward = o.wrapping_sub(s.wrapping_mul(2));

    let ro = o.reverse_bits();
    let rs = bit(63 - sq);
    let backward = (ro.wrapping_sub(rs.wrapping_mul(2))).reverse_bits();

    (forward ^ backward) & mask
}

const START_SHIFT_ROOK: u32 = 52;
const START_SHIFT_BISHOP: u32 = 54;

#[derive(Debug, Clone)]
pub struct MagicLookup {
    pub magic: u64,
    pub shift: u32,
    sq: usize,
    lookup: Box<[BB]>,
    is_rook: bool,
}

impl MagicLookup {
    pub fn get(&self, all: BB, color: BB) -> BB {
        let base_mask = if self.is_rook {
            let rank_part = super::RANK_MASKS[self.sq] & !(FILE_A | FILE_H);
            let file_part = super::FILE_MASKS[self.sq] & !(RANK_1 | RANK_8);
            rank_part | file_part
        } else {
            BISHOP_MASKS_NO_ENDS[self.sq]
        };

        let full_blockers = if self.is_rook {
            super::RANK_MASKS[self.sq] | super::FILE_MASKS[self.sq]
        } else {
            super::DIAGONAL_MASKS[self.sq] | super::ANTI_DIAGONAL_MASKS[self.sq]
        };
        let blockers = all & base_mask;

        let idx = (blockers.wrapping_mul(self.magic) >> self.shift) as usize;

        let legalish = self.lookup[idx];

        let us_blockers = full_blockers & color;

        (us_blockers & legalish) ^ legalish
    }
}

fn generate_rook_magics(rng: &mut SmallRng) -> [MagicLookup; 64] {
    let rookup_table = create_rook_lookup_table();
    std::array::from_fn(|n| {
        let rank_part = super::RANK_MASKS[n as usize] & !(FILE_A | FILE_H);
        let file_part = super::FILE_MASKS[n as usize] & !(RANK_1 | RANK_8);
        let rook_mask = rank_part | file_part;
        let (magic, shift) =
            generate_magic_number(n as u32, START_SHIFT_ROOK, &rookup_table, rng, rook_mask);

        let mut lookup = vec![0; 1 << (64 - shift)];

        get_all_blocker_patterns(rook_mask)
            .map(|pat| pat)
            .for_each(|pat| {
                let idx = (pat.wrapping_mul(magic) >> shift) as usize;
                let legal = *rookup_table.get(&(n as u32, pat)).unwrap();

                lookup[idx] = legal;
            });

        MagicLookup {
            magic,
            shift,
            sq: n,
            lookup: lookup.into_boxed_slice(),
            is_rook: true,
        }
    })
}

fn generate_rook_table_from_magic(magics: [(u64, u32); 64]) -> [MagicLookup; 64] {
    let rookup_table = create_rook_lookup_table();

    std::array::from_fn(|n| {
        let (magic, shift) = magics[n];

        let mut lookup = vec![0; 1 << (64 - shift)];

        let rank_part = super::RANK_MASKS[n as usize] & !(FILE_A | FILE_H);
        let file_part = super::FILE_MASKS[n as usize] & !(RANK_1 | RANK_8);
        let rook_mask = rank_part | file_part;

        get_all_blocker_patterns(rook_mask)
            .map(|pat| pat)
            .for_each(|pat| {
                let idx = (pat.wrapping_mul(magic) >> shift) as usize;
                let legal = *rookup_table.get(&(n as u32, pat)).unwrap();

                lookup[idx] = legal;
            });

        MagicLookup {
            magic,
            shift,
            sq: n,
            lookup: lookup.into_boxed_slice(),
            is_rook: true,
        }
    })
}

fn generate_bishop_table_from_magic(magics: [(u64, u32); 64]) -> [MagicLookup; 64] {
    let rookup_table = create_bishop_lookup_table();

    std::array::from_fn(|n| {
        let (magic, shift) = magics[n];

        let mut lookup = vec![0; 1 << (64 - shift)];

        let bishop_mask = BISHOP_MASKS_NO_ENDS[n];

        get_all_blocker_patterns(bishop_mask)
            .map(|pat| pat)
            .for_each(|pat| {
                let idx = (pat.wrapping_mul(magic) >> shift) as usize;
                let legal = *rookup_table.get(&(n as u32, pat)).unwrap();

                lookup[idx] = legal;
            });

        MagicLookup {
            magic,
            shift,
            sq: n,
            lookup: lookup.into_boxed_slice(),
            is_rook: false,
        }
    })
}

fn generate_magic_number(
    n: u32,
    start_shift: u32,
    rt: &RookupTable,
    rng: &mut SmallRng,
    mask: BB,
) -> (u64, u32) {
    let blocker_patterns = get_all_blocker_patterns(mask)
        .map(|pat| (pat, *rt.get(&(n, pat)).unwrap()))
        .collect::<Vec<_>>();

    let capacity = 1usize << (64 - start_shift);
    let mut table = vec![0; capacity];
    let mut gens = vec![0; capacity];
    let mut generation = 0;

    rand_iter(rng, mask)
        .find_map(|num| {
            check_if_works(
                num,
                start_shift,
                &blocker_patterns,
                &mut table,
                &mut gens,
                &mut generation,
            )
            .map(|shift| (num, shift))
        })
        .unwrap()
}

fn rand_iter(rng: &mut SmallRng, _r_mask: u64) -> impl Iterator<Item = u64> {
    rng.random_iter()
        .map(|(a, b, c): (u64, u64, u64)| a & b & c)
    //.filter(move |m| ((r_mask * m) & 0xff00000000000000).count_ones() >= 6)
}

fn check_if_works(
    magic: u64,
    start_shift: u32,
    blocker_patterns: &[(u64, u64)],
    table: &mut [u64],
    gens: &mut [u32],
    generation: &mut u32,
) -> Option<u32> {
    (start_shift..64)
        .take_while(|&shift| {
            *generation = generation.wrapping_add(1);
            let cur_gen = *generation;

            blocker_patterns.iter().all(|&(pat, legal)| {
                let idx = (pat.wrapping_mul(magic) >> shift) as usize;
                let slot_gen = &mut gens[idx];
                let slot_val = &mut table[idx];

                if *slot_gen != cur_gen {
                    *slot_gen = cur_gen;
                    *slot_val = legal;
                    true
                } else {
                    *slot_val == legal
                }
            })
        })
        .last()
}

fn step(s: u32, d: (i32, i32)) -> Option<u32> {
    let rank = s / 8;
    let file = s % 8;

    match d {
        (1, 1) => {
            if rank < 7 && file < 7 {
                Some(s + 9)
            } else {
                None
            }
        }
        (-1, 1) => {
            if rank < 7 && file > 0 {
                Some(s + 7)
            } else {
                None
            }
        }
        (1, -1) => {
            if rank > 0 && file < 7 {
                Some(s - 7)
            } else {
                None
            }
        }
        (-1, -1) => {
            if rank > 0 && file > 0 {
                Some(s - 9)
            } else {
                None
            }
        }
        _ => unreachable!(),
    }
}
fn bishop_mask(square: u32) -> u64 {
    let mut mask = 0;

    for dir in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
        let Some(mut s) = step(square, dir) else {
            continue;
        };

        while let Some(next) = step(s, dir) {
            mask |= 1u64 << s;
            s = next;
        }
    }

    mask
}

static BISHOP_MASKS_NO_ENDS: LazyLock<[BB; 64]> =
    LazyLock::new(|| std::array::from_fn(|sq| bishop_mask(sq as u32)));

fn create_bishop_lookup_table() -> RookupTable {
    let mut table = HashMap::new();

    for n in 0..64 {
        let bishop_mask = BISHOP_MASKS_NO_ENDS[n];

        get_all_blocker_patterns(bishop_mask).for_each(|pat| {
            table.insert((n as u32, pat), create_bishop_legal_move(n as u32, pat));
        })
    }

    table
}

fn create_bishop_legal_move(sq: u32, all: BB) -> BB {
    hyperbola_quintessence(sq, all, super::DIAGONAL_MASKS[sq as usize])
        | hyperbola_quintessence(sq, all, super::ANTI_DIAGONAL_MASKS[sq as usize])
}

fn generate_bishop_magics(rng: &mut SmallRng) -> [MagicLookup; 64] {
    let rookup_table = create_bishop_lookup_table();
    std::array::from_fn(|n| {
        let bishop_mask = BISHOP_MASKS_NO_ENDS[n];
        let (magic, shift) = generate_magic_number(
            n as u32,
            START_SHIFT_BISHOP,
            &rookup_table,
            rng,
            bishop_mask,
        );

        let mut lookup = vec![0; 1 << (64 - shift)];

        get_all_blocker_patterns(bishop_mask)
            .map(|pat| pat)
            .for_each(|pat| {
                let idx = (pat.wrapping_mul(magic) >> shift) as usize;
                let legal = *rookup_table.get(&(n as u32, pat)).unwrap();

                lookup[idx] = legal;
            });

        MagicLookup {
            magic,
            shift,
            sq: n,
            lookup: lookup.into_boxed_slice(),
            is_rook: false,
        }
    })
}

/*
pub static BISHOP_MAGICS: LazyLock<[MagicLookup; 64]> = LazyLock::new(|| {
    generate_bishop_magics(&mut SmallRng::from_seed(
        *b"magicbitboardsaresostupidblahbla",
    ))
});

pub static ROOK_MAGICS: LazyLock<[MagicLookup; 64]> = LazyLock::new(|| {
    generate_rook_magics(&mut SmallRng::from_seed(
        *b"whatifinsteadofmagicitwasnothaha",
    ))
});
*/

pub static BISHOP_MAGICS: LazyLock<[MagicLookup; 64]> =
    LazyLock::new(|| generate_bishop_table_from_magic(BISHOP_MAGICS_PRECOMP));

pub static ROOK_MAGICS: LazyLock<[MagicLookup; 64]> =
    LazyLock::new(|| generate_rook_table_from_magic(ROOK_MAGICS_PRECOMP));

const BISHOP_MAGICS_PRECOMP: [(u64, u32); 64] = [
    (576604788503086224, 54),
    (1126999733043752, 55),
    (9227876255498502181, 58),
    (566248756740352, 57),
    (7063898219252809736, 54),
    (2306124561533502788, 55),
    (6958976359730282496, 54),
    (4399392956576, 55),
    (5506248737792, 54),
    (1299852854454419457, 54),
    (288266453961015301, 54),
    (1266792047575040, 54),
    (18166140781985920, 54),
    (9241386483145910280, 55),
    (1154056269594631170, 54),
    (5704270622491648, 54),
    (9224497937298620448, 54),
    (2071726267145125920, 56),
    (2818610943001090, 54),
    (4683813986175683078, 54),
    (19423438232355856, 56),
    (9255055650634090560, 54),
    (565151128356896, 54),
    (8797196386312, 54),
    (4785216342212872, 54),
    (36027326534659, 55),
    (351914730586145, 55),
    (1173751240354464032, 55),
    (577023736819023936, 54),
    (144256750472665088, 55),
    (4629876507643806224, 55),
    (1729986988602360128, 54),
    (1190498628880826880, 55),
    (77141813147758722, 56),
    (180146493893116120, 54),
    (1153097443718480128, 54),
    (45177043268174336, 54),
    (9227875707349241880, 54),
    (119391020429606946, 57),
    (10522660563794789952, 54),
    (7278098558740333573, 55),
    (4686558448165322768, 54),
    (11568064090479822868, 54),
    (1231171586789739392, 54),
    (303490983596064, 56),
    (27022836896104960, 54),
    (1486188426956931440, 55),
    (2343015319817422080, 54),
    (1161930912015859750, 54),
    (6917811877075943488, 54),
    (11294335916834880, 55),
    (9263060283465925632, 54),
    (577586669458031136, 57),
    (2308099211637362945, 58),
    (9512730795479998853, 56),
    (9009398615115792, 54),
    (9048431747143680, 55),
    (2305847545825333792, 54),
    (70643638894656, 54),
    (4611690966296953184, 55),
    (144185831731581568, 55),
    (564359440900609, 54),
    (9007757910875144, 55),
    (9804336698027425856, 56),
];

const ROOK_MAGICS_PRECOMP: [(u64, u32); 64] = [
    (36028867898507296, 52),
    (216199445285470224, 52),
    (14447552621126967296, 52),
    (144117662047297544, 52),
    (612490683194015778, 52),
    (360290309000659272, 52),
    (4899935120641363972, 52),
    (1801440271863565056, 52),
    (7566328923055718432, 52),
    (578747753937829896, 52),
    (9259418975884806024, 52),
    (145381843727845382, 52),
    (38562140931490818, 53),
    (1153660385019169797, 52),
    (18296974071824896, 52),
    (307652158155690017, 52),
    (9547640280998477874, 52),
    (2314850243936665600, 52),
    (504684942758789136, 52),
    (36346555959611392, 53),
    (18016631959587076, 52),
    (289673004164579465, 52),
    (389636134577147912, 53),
    (5188153367872427033, 52),
    (1162597449597009921, 52),
    (585468020479492161, 52),
    (2341906994912303753, 52),
    (1298204684611241984, 52),
    (1733916720173028385, 52),
    (1152996340184154128, 52),
    (19281041542090760, 52),
    (704245787803684, 52),
    (46214947624976928, 52),
    (2377975378766496001, 52),
    (4611690592634699792, 52),
    (45038195700662321, 52),
    (4559129293095969, 52),
    (4648286570419852160, 52),
    (213305339687044, 53),
    (72093335447143433, 52),
    (2305900458702569472, 52),
    (18015567143276546, 52),
    (12125977252001222656, 52),
    (9299950822774408192, 52),
    (5476517918999119872, 52),
    (4402349871632, 52),
    (576609188557373442, 52),
    (10700573605894422532, 52),
    (288371114715130112, 52),
    (63095509388788224, 52),
    (576465184986570784, 52),
    (90230322255369504, 52),
    (9011597318684736, 52),
    (2338610741568, 52),
    (585470154876404368, 52),
    (1129234152038912, 52),
    (37717784590909698, 52),
    (4791971098566596641, 52),
    (432417067119936546, 52),
    (72094995418450081, 52),
    (1730596127478644753, 52),
    (32096952573493249, 52),
    (2378470184904081796, 52),
    (2377900882978407042, 52),
];
