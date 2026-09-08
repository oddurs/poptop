//! The poptop website.
//!
//! One binary that either serves the site or writes it out as static files.
//! Both come from the same handlers, so the deployed site is the one that was
//! developed against, and the export is not a second implementation waiting to
//! disagree with the first.

mod app;
mod assets;
mod config;
mod content;
mod cvd;
mod demo;
mod export;
mod fonts;
mod markdown;
mod routes;
mod site;
mod views;

use config::{Command, Config};
use std::process::ExitCode;

fn main() -> ExitCode {
    let config = match Config::from_args(std::env::args().skip(1)) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("poptop-web: {message}\n\n{}", config::HELP);
            return ExitCode::from(2);
        }
    };

    match config.command {
        Command::Help => {
            print!("{}", config::HELP);
            ExitCode::SUCCESS
        }
        Command::Routes => {
            for route in routes::all() {
                println!("{}", route.path);
            }
            ExitCode::SUCCESS
        }
        Command::Build { ref out } => match export::build(out, &config.base_url) {
            Ok(count) => {
                println!("wrote {count} files to {}", out.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("poptop-web: {}: {e}", out.display());
                ExitCode::FAILURE
            }
        },
        Command::Serve { addr, live } => serve(addr, live, config.base_url),
    }
}

#[tokio::main]
async fn serve(addr: std::net::SocketAddr, live: bool, base_url: String) -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "poptop_web=info,tower_http=warn".into()),
        )
        .with_target(false)
        .init();

    let router = app::router(&base_url);

    #[cfg(feature = "live")]
    let router = if live {
        // Reloads the browser when this process restarts, which is what
        // `cargo watch -x dev` does on every save.
        router.layer(tower_livereload::LiveReloadLayer::new())
    } else {
        router
    };
    #[cfg(not(feature = "live"))]
    let _ = live;

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("poptop-web: cannot bind {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };

    tracing::info!("http://{addr} — {} routes", routes::all().len());

    let served = axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;

    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("poptop-web: {e}");
            ExitCode::FAILURE
        }
    }
}
