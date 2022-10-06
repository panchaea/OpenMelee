use clap::{Parser, Subcommand};

mod matchmaking;
mod webserver;

use openmelee::{init_pool, run_migrations};

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
    let cli = Cli::parse();

    match &cli.command {
        None => {
            let config = openmelee::CONFIG.clone();

            if config.jwt_secret_path.is_none() {
                panic!("JWT secret path not configured, exiting");
            }

            let pool = init_pool(config.clone()).await;

            run_migrations(&pool).await;

            let webserver_thread =
                tokio::spawn(webserver::start_server(config.clone(), pool.clone()));

            let enet_server_thread = tokio::task::spawn_blocking(move || {
                matchmaking::start_server(config.clone(), pool);
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
