use dotenv::dotenv;
use futures_util::StreamExt;
use litchee::{
    LichessClient,
    api::gameplay::{
        board::{LichessBoardEvent, LichessGameEventInfo, LichessIncomingEvent},
        games::LichessGameStatusName,
    },
    model::LichessColor,
};

mod display;
mod eval_engine;
mod game;

#[tokio::main]
async fn main() -> litchee::Result<()> {
    dotenv().ok();

    let mut args = std::env::args().skip(1);
    let p = args.next();

    let Some(p) = p else {
        player_vs_bot();
        return Ok(());
    };

    match p.as_str() {
        "flame" => flame_on(
            args.next().map(|n| n.parse::<u16>().unwrap()).unwrap_or(8),
            args.next()
                .map(|n| n.parse::<u64>().unwrap())
                .unwrap_or(3000),
        ),
        "bot" => bot_on_bot(),
        "otherbot" => otherbot(),
        "speed" => println!(
            "speed: {}ms",
            speed(args.next().map(|n| n.parse::<u16>().unwrap()).unwrap_or(7)).as_millis()
        ),
        "test_possible" => {
            test_possible(args.next().map(|n| n.parse::<u16>().unwrap()).unwrap_or(7))
        }
        "write_magic" => {
            std::fs::write(
                "magics_bishop.txt",
                format!(
                    "{:?}",
                    (game::magic_bb::BISHOP_MAGICS)
                        .iter()
                        .map(|m| (m.magic, m.shift))
                        .collect::<Vec<_>>()
                ),
            )
            .unwrap();
            std::fs::write(
                "magics_rook.txt",
                format!(
                    "{:?}",
                    game::magic_bb::ROOK_MAGICS
                        .iter()
                        .map(|m| (m.magic, m.shift))
                        .collect::<Vec<_>>()
                ),
            )
            .unwrap();
        }
        "lichess" => {
            let client = LichessClient::builder()
                .token(std::env::var("LICHESS_TOKEN").unwrap())
                .build()?;

            let me = client.account().profile().await?;
            println!("Logged in as {}", me.user.username);

            let mut events = client.bot().stream_events().await?;

            while let Some(event) = events.next().await {
                match event? {
                    LichessIncomingEvent::Challenge { challenge } => {
                        client.challenges().accept(&challenge.id).await?;
                    }
                    LichessIncomingEvent::GameStart { game } => {
                        let client = client.clone();

                        tokio::spawn(async move {
                            if let Err(e) = lichess_play_game(client, game).await {
                                eprintln!("{e:?}");
                            }
                        });
                    }

                    e => eprintln!("WARNING: ignoring event {:?}", e),
                }
            }
        }
        c => panic!("unknown arg \"{c}\""),
    };

    Ok(())
}

async fn lichess_play_game(
    client: LichessClient,
    game: LichessGameEventInfo,
) -> litchee::Result<()> {
    let id = game.game_id.unwrap();
    let mut stream = client.bot().stream_game(&id).await?;

    let mut current_game = game::Game::default();
    let bot_color = match game.color.unwrap() {
        LichessColor::Black => game::Color::Black,
        LichessColor::White => game::Color::White,
    };

    println!("{:?}", bot_color);

    let mut bot = eval_engine::Engine::new(eval_engine::CalcConstraint::Time(
        std::time::Duration::from_millis(5000),
    ));

    while let Some(event) = stream.next().await {
        match event? {
            LichessBoardEvent::ChatLine(msg) => println!("{}: {}", msg.username, msg.text),
            LichessBoardEvent::GameFull(data) => {
                if let Some(clock) = data.clock {
                    let (inc, init) = (
                        std::time::Duration::from_millis(clock.increment.unwrap() as u64),
                        std::time::Duration::from_millis(clock.initial.unwrap() as u64),
                    );
                    bot.set_think_time(inc, init);
                } else {
                    bot.set_think_time(
                        std::time::Duration::from_secs(10),
                        std::time::Duration::from_secs(0),
                    );
                }

                let fen = data.initial_fen.unwrap();
                if fen == "startpos" {
                    current_game = game::Game::default();
                } else {
                    current_game = game::Game::from_fen(&fen);
                }
                for m in data.state.moves.split(' ').filter(|m| !m.is_empty()) {
                    let m = game::Move::from_str(m);
                    current_game.move_piece(m);
                }
            }
            LichessBoardEvent::GameState(state) => {
                match state.status {
                    LichessGameStatusName::Started => (),
                    _ => return Ok(()),
                }

                let m = state.moves.split(' ').last().unwrap();
                let m = game::Move::from_str(m);
                current_game.move_piece(m);

                if current_game.get_to_move() == bot_color {
                    let best = bot.best_move(&mut current_game);

                    println!("eval: {}, depth: {}", best.0, best.2);

                    let best = best.1;

                    client
                        .bot()
                        .make_move(&id, &best.to_string(), false)
                        .await
                        .unwrap();
                }
            }
            LichessBoardEvent::OpponentGone(_) => (),
            a => println!("WARNING: skipping unknown board event: {:?}", a),
        }
    }

    Ok(())
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

    display::player_vs_bot(game, game::Color::Black);
}

#[allow(unused)]
fn otherbot() {
    let mut game = game::Game::default();

    display::player_vs_bot(game, game::Color::White);
}

#[allow(unused)]
fn flame_on(moves: u16, time_ms: u64) {
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

fn speed(depth: u16) -> std::time::Duration {
    println!("running speed test...");

    let wait_time = eval_engine::CalcConstraint::Depth(depth);

    let mut game = game::Game::default();

    let mut engine = eval_engine::Engine::new(wait_time);

    engine.best_move(&mut game);

    println!(
        "searched {} nodes",
        *eval_engine::NODES_SEARCHED.lock().unwrap()
    );

    engine.elapsed()
}

// TODO: add tests so we can just run cargo test
fn test_possible(depth: u16) {
    let mut game = game::Game::default();

    fn explore(game: &mut game::Game, depth: u16) -> usize {
        if depth == 0 {
            return 1;
        }

        game.get_all_moves(game.get_to_move())
            .into_iter()
            .map(|m| {
                game.move_piece(m);
                let n = explore(game, depth - 1);
                game.undo_move();

                n
            })
            .sum()
    }

    println!(
        "at depth {}, there are {} nodes",
        depth,
        explore(&mut game, depth)
    )
}
