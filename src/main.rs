mod display;
mod eval_engine;
mod game;

fn main() {
    bot_on_bot();
}

#[allow(unused)]
fn run_fen(fen: &str) {
    let game = game::Game::from_fen(fen);

    display::run(game);
}

#[allow(unused)]
fn bot_on_bot() {
    let game = game::Game::default();

    display::run(game);
}

#[allow(unused)]
fn flame_on() {
    let mut game = game::Game::default();
    let mut engine = eval_engine::Engine::new();

    let turns = 8;
    for _ in 0..turns {
        if !game.checkmate(game.get_to_move()) {
            let engine_move = engine.best_move(&game, game.get_to_move());
            game.move_piece(engine_move.1);
            game.swap_to_move();
        }
    }
}
