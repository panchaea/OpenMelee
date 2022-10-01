use clap::{Parser, Subcommand};
use figment::{providers::Env, providers::Serialized, Figment};

mod matchmaking;
mod webserver;

use slippi_re::{init_pool, run_migrations, Config};

#[derive(Parser)]
#[clap()]
struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {}

#[tokio::main]
async fn main() {
    let config: Config = Figment::from(Serialized::defaults(Config::default()))
        .merge(Env::prefixed("SLIPPI_RE_"))
        .extract()
        .unwrap();

    let cli = Cli::parse();

    match &cli.command {
        None => {
            let pool = init_pool(config.clone()).await;

            run_migrations(&pool).await;

            let webserver_thread =
                tokio::spawn(webserver::start_server(config.clone(), pool.clone()));

            let enet_server_thread = tokio::task::spawn_blocking(move || {
                matchmaking::start_server(config, pool);
            });

            if webserver_thread.await.is_err() {
                println!("webserver thread exited abnormally")
            }
            if enet_server_thread.await.is_err() {
                println!("ENet server thread exited abnormally")
            }
        }
        Some(_) => (),
    }
}
