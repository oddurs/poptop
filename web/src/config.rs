//! Command-line configuration.
//!
//! Hand-parsed, like poptop's own. A website with two verbs does not need an
//! argument-parsing dependency, and the help text below is the whole surface.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

pub const HELP: &str = "\
poptop-web — the poptop website

usage:
  poptop-web serve [--port N] [--host ADDR] [--live]
  poptop-web build [--out DIR] [--base-url URL]
  poptop-web routes

  serve    run the site (default)
  build    render every route to static files
  routes   print every URL the site serves, one per line

options:
  --port N          port to listen on (default 3000, or $PORT)
  --host ADDR       address to bind (default 127.0.0.1)
  --live            reload the browser when the server restarts
  --out DIR         output directory for build (default dist)
  --base-url URL    absolute origin for canonical URLs and the sitemap
  -h, --help        this
";

pub enum Command {
    Serve { addr: SocketAddr, live: bool },
    Build { out: PathBuf },
    Routes,
    Help,
}

pub struct Config {
    pub command: Command,
    pub base_url: String,
}

impl Config {
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter().peekable();
        let verb = match args.peek().map(String::as_str) {
            Some("serve") | Some("build") | Some("routes") | Some("help") => {
                args.next().unwrap_or_default()
            }
            _ => "serve".to_string(),
        };

        let mut port: u16 = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);
        let mut host = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let mut live = false;
        let mut out = PathBuf::from("dist");
        let mut base_url =
            std::env::var("BASE_URL").unwrap_or_else(|_| crate::site::DEFAULT_BASE_URL.to_string());
        let mut help = verb == "help";

        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
            match arg.as_str() {
                "-h" | "--help" => help = true,
                "--live" => live = true,
                "--port" => {
                    port = value("--port")?
                        .parse()
                        .map_err(|_| "--port wants a number")?
                }
                "--host" => {
                    host = value("--host")?
                        .parse()
                        .map_err(|_| "--host wants an address")?
                }
                "--out" => out = PathBuf::from(value("--out")?),
                "--base-url" => base_url = value("--base-url")?,
                other => return Err(format!("unknown option {other}")),
            }
        }

        // Trailing slashes are the classic way to get `https://x//page` into a
        // canonical tag.
        let base_url = base_url.trim_end_matches('/').to_string();

        let command = if help {
            Command::Help
        } else {
            match verb.as_str() {
                "build" => Command::Build { out },
                "routes" => Command::Routes,
                _ => Command::Serve {
                    addr: SocketAddr::new(host, port),
                    live,
                },
            }
        };

        Ok(Config { command, base_url })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Config {
        Config::from_args(args.iter().map(|s| (*s).to_string())).expect("parses")
    }

    #[test]
    fn serving_is_the_default_verb() {
        assert!(matches!(parse(&[]).command, Command::Serve { .. }));
        assert!(matches!(
            parse(&["--port", "8080"]).command,
            Command::Serve { .. }
        ));
    }

    #[test]
    fn a_port_can_be_given_without_the_verb() {
        let Command::Serve { addr, .. } = parse(&["--port", "8080"]).command else {
            panic!("expected serve");
        };
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn a_trailing_slash_on_the_base_url_is_dropped() {
        assert_eq!(
            parse(&["--base-url", "https://x.dev/"]).base_url,
            "https://x.dev"
        );
    }

    #[test]
    fn an_unknown_option_is_an_error_rather_than_a_default() {
        assert!(Config::from_args(["--colour".to_string()]).is_err());
    }
}
