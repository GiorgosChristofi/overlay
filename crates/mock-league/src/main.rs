//! Runs the fake League client and game for the real desktop app.
//!
//! ```text
//! mock-league [--scenario FILE] [--out DIR] [--live-port 2999]
//!             [--lcu-port 0] [--control-port 0] [--exit-when-done]
//! ```
//!
//! Writes `DIR/lockfile` and `DIR/mock-root.pem`, then plays the scenario.
//! Start a debug build of the app with `LOL_OVERLAY_LCU_LOCKFILE` and
//! `LOL_OVERLAY_RIOT_PEM` pointing at those files (printed on start).

use mock_league::{MockLeague, Options, Scenario};
use std::path::PathBuf;

struct Args {
    scenario: Option<PathBuf>,
    out: PathBuf,
    live_port: u16,
    lcu_port: u16,
    control_port: u16,
    exit_when_done: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        scenario: None,
        out: PathBuf::from("target/mock-league"),
        live_port: 2999,
        lcu_port: 0,
        control_port: 0,
        exit_when_done: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        let port = |v: String| v.parse::<u16>().map_err(|e| format!("bad port {v}: {e}"));
        match flag.as_str() {
            "--scenario" => args.scenario = Some(value()?.into()),
            "--out" => args.out = value()?.into(),
            "--live-port" => args.live_port = port(value()?)?,
            "--lcu-port" => args.lcu_port = port(value()?)?,
            "--control-port" => args.control_port = port(value()?)?,
            "--exit-when-done" => args.exit_when_done = true,
            "-h" | "--help" => {
                return Err(
                    "usage: mock-league [--scenario FILE] [--out DIR] [--live-port 2999] \
                            [--lcu-port 0] [--control-port 0] [--exit-when-done]"
                        .into(),
                )
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(args)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("mock_league=info")),
        )
        .with_writer(std::io::stderr)
        .init();
    if let Err(e) = run().await {
        eprintln!("mock-league: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args = parse_args()?;
    let scenario = match &args.scenario {
        Some(path) => Scenario::from_json(
            &std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
        )?,
        None => Scenario::default(),
    };
    let mock = MockLeague::start(Options {
        lcu_port: args.lcu_port,
        live_port: args.live_port,
        control_port: Some(args.control_port),
        setup: scenario.setup.clone(),
        ..Options::default()
    })
    .await?;

    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let lockfile = args.out.join("lockfile");
    let pem = args.out.join("mock-root.pem");
    std::fs::write(&lockfile, mock.lockfile()).map_err(|e| e.to_string())?;
    std::fs::write(&pem, mock.root_pem()).map_err(|e| e.to_string())?;
    let abs = |p: &PathBuf| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());

    eprintln!("mock-league running (127.0.0.1 only)");
    eprintln!("  client (LCU):  https://127.0.0.1:{}", mock.lcu_port());
    eprintln!("  game (live):   {}", mock.live_url());
    if let Some(url) = mock.control_url() {
        eprintln!("  control:       {url}  (POST /step, POST /steps, GET /state)");
    }
    eprintln!("Start a debug build of the app with:");
    eprintln!("  LOL_OVERLAY_LCU_LOCKFILE={}", abs(&lockfile).display());
    eprintln!("  LOL_OVERLAY_RIOT_PEM={}", abs(&pem).display());
    if !scenario.name.is_empty() {
        eprintln!("Playing scenario: {}", scenario.name);
    }

    mock.run(&scenario.steps).await;
    eprintln!("Scenario finished.");
    let snapshot = serde_json::to_string_pretty(&mock.snapshot()).map_err(|e| e.to_string())?;
    let state_file = args.out.join("state.json");
    std::fs::write(&state_file, snapshot).map_err(|e| e.to_string())?;
    eprintln!("State written to {}", abs(&state_file).display());
    if args.exit_when_done {
        return Ok(());
    }
    eprintln!("Still serving; Ctrl+C to stop.");
    tokio::signal::ctrl_c().await.map_err(|e| e.to_string())
}
