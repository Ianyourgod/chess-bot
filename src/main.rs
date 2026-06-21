mod display;
mod eval_engine;
mod game;

fn main() {
    flame_on();
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
    let game = game::Game::default();

    display::player_vs_bot(game);
}

#[allow(unused)]
fn flame_on() {
    let mut game = game::Game::default();
    let mut engine = eval_engine::Engine::new();

    let turns = 10;
    for _ in 0..turns {
        if !game.checkmate(game.get_to_move()) {
            let engine_move = engine.best_move(&game, game.get_to_move());
            game.move_piece(engine_move.1);
        }
    }
}
