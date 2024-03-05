#![allow(clippy::uninlined_format_args)]

use std::str::FromStr;

use anyhow::Result;
use engine::logger::{self, Logger};
use engine::main_loop::MainLoop;
use engine::window::Resolution;
use log::{debug, LevelFilter};

fn main() -> Result<()> {
    engine::panic::set_hook();
    let args = parse_args();
    init_logger(&args);

    let res = Resolution::Windowed(1024, 768);
    let mut main_loop = MainLoop::new(res, "game")?;

    if let Some(frames) = args.benchmark {
        main_loop.benchmark(frames);
        return Ok(());
    }

    main_loop.run();

    Ok(())
}

struct Args {
    log_level: LevelFilter,
    verbose: bool,
    benchmark: Option<usize>,
}

fn parse_args() -> Args {
    let mut args = Args {
        log_level: LevelFilter::Info,
        verbose: false,
        benchmark: None,
    };

    let passed_args = std::env::args().collect::<Vec<String>>();
    let passed_args = passed_args.iter().map(String::as_str).collect::<Vec<&str>>();
    let [_progname, tail @ ..] = passed_args.as_slice() else {
        return args;
    };

    let mut it = tail;

    loop {
        match it {
            ["-l" | "--log", level, rest @ ..] => {
                let Ok(level) = LevelFilter::from_str(level) else {
                    panic!("malformed log level \"{}\"", level)
                };
                args.log_level = level;
                it = rest;
            }
            ["-b" | "--benchmark", frames, rest @ ..] => {
                let Ok(frames) = frames.parse::<usize>() else {
                    panic!("failed to parse number of frames to benchmark: got \"{}\"", frames);
                };
                args.benchmark = Some(frames);
                it = rest;
            }
            ["-v" | "--verbose", rest @ ..] => {
                args.verbose = true;
                it = rest;
            }
            [unknown, rest @ ..] => {
                eprintln!("Unknown argument \"{}\"", unknown);
                it = rest;
            }
            [] => break,
        }
    }

    args
}

fn init_logger(args: &Args) {
    Logger::new(args.log_level).init();
    logger::set_verbosity(args.verbose);

    debug!("Game v{}", env!("CARGO_PKG_VERSION"));
}
