mod display;
mod eval_engine;
mod game;

fn main() {
    let mut args = std::env::args().skip(1);
    let p = args.next();

    let Some(p) = p else {
        player_vs_bot();
        return;
    };

    match p.as_str() {
        "flame" => flame_on(
            args.next().map(|n| n.parse::<u32>().unwrap()).unwrap_or(8),
            args.next()
                .map(|n| n.parse::<u64>().unwrap())
                .unwrap_or(3000),
        ),
        "bot" => bot_on_bot(),
        "otherbot" => otherbot(),
        "speed" => println!(
            "speed: {}ms",
            speed(args.next().map(|n| n.parse::<u32>().unwrap()).unwrap_or(7)).as_millis()
        ),
        c => panic!("unknown arg \"{c}\""),
    }
}

#[allow(unused)]
fn run_fen_bot_on_bot(fen: &str) {
    let game = game::Game::from_fen(fen);

    display::bot_on_bot(game);
}

#[allow(unused)]
fn bot_on_bot() {
    let game = game::Game::default();

    display::bot_on_bot(game);
}

#[allow(unused)]
fn player_vs_bot() {
    let game = game::Game::from_fen("rnbqkbnr/Pppppppp/8/8/8/8/1PPPPPPP/RNBQKBNR w");

    display::player_vs_bot(game, game::Color::Black);
}

#[allow(unused)]
fn otherbot() {
    let mut game = game::Game::default();

    display::player_vs_bot(game, game::Color::White);
}

#[allow(unused)]
fn flame_on(moves: u32, time_ms: u64) {
    let wait_time = eval_engine::CalcConstraint::Time(std::time::Duration::from_millis(time_ms));

    let mut game = game::Game::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq c3");

    let mut engine = eval_engine::Engine::new(wait_time);

    let turns = moves;
    for _ in 0..turns {
        if !game.checkmate(game.get_to_move()) {
            let engine_move = engine.best_move(&mut game);
            game.move_piece(engine_move.1);
        }
    }
}

fn speed(depth: u32) -> std::time::Duration {
    let wait_time = eval_engine::CalcConstraint::Depth(depth);

    let mut game = game::Game::default();

    let mut engine = eval_engine::Engine::new(wait_time);

    engine.best_move(&mut game);

    engine.elapsed()
}
