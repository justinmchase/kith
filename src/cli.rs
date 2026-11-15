use crate::ast::Program;
use crate::codec::encode;
use crate::doc;
use crate::error::Error;
use crate::interpreter::World;
use crate::parse::{artifact_document, load_program, parse_file};
use serde_json::Value as Json;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Command {
    Parse {
        path: PathBuf,
    },
    Check {
        path: PathBuf,
    },
    Compile {
        path: PathBuf,
    },
    Run {
        path: PathBuf,
        message: Option<Json>,
        argv: Vec<String>,
    },
    Version,
}

pub struct RunResult {
    pub text: String,
    pub code: u8,
}

pub fn parse_args(args: &[String]) -> Result<Command, Error> {
    let mut args = args.iter().skip(1);
    let command = args
        .next()
        .ok_or_else(|| Error::new("usage: kith <parse|check|compile|run> <file>"))?;
    match command.as_str() {
        "--version" | "-V" => return Ok(Command::Version),
        "parse" | "check" | "compile" => {
            let path = args
                .next()
                .ok_or_else(|| Error::new(format!("usage: kith {command} <file>")))?;
            let path = PathBuf::from(path);
            Ok(match command.as_str() {
                "parse" => Command::Parse { path },
                "check" => Command::Check { path },
                "compile" => Command::Compile { path },
                _ => unreachable!(),
            })
        }
        "run" => {
            let mut path = None;
            let mut message = None;
            let mut argv = Vec::new();
            while let Some(arg) = args.next() {
                if arg == "--message" {
                    let value = args
                        .next()
                        .ok_or_else(|| Error::new("kith run --message needs JSON"))?;
                    message = Some(
                        serde_json::from_str(value)
                            .map_err(|error| Error::new(error.to_string()))?,
                    );
                } else if arg == "--" {
                    argv.extend(args.cloned());
                    break;
                } else if path.is_none() {
                    path = Some(PathBuf::from(arg));
                } else {
                    argv.push(arg.clone());
                }
            }
            Ok(Command::Run {
                path: path.ok_or_else(|| Error::new("usage: kith run <file> [--message JSON]"))?,
                message,
                argv,
            })
        }
        other => Err(Error::new(format!(
            "unknown command {other}; expected parse, check, compile, or run"
        ))),
    }
}

pub fn run(command: Command) -> Result<RunResult, Error> {
    match command {
        Command::Version => Ok(RunResult {
            text: format!("{}\n", env!("CARGO_PKG_VERSION")),
            code: 0,
        }),
        Command::Parse { path } => {
            let program = parse_file(&path)?;
            let tree =
                serde_json::to_value(&program).map_err(|error| Error::new(error.to_string()))?;
            Ok(RunResult {
                text: encode(&tree)?,
                code: 0,
            })
        }
        Command::Check { path } => {
            let program = load_program(&path)?;
            World::check(&program)?;
            Ok(RunResult {
                text: "ok\n".into(),
                code: 0,
            })
        }
        Command::Compile { path } => {
            let program = load_program(&path)?;
            World::check(&program)?;
            let out = artifact_path(&path);
            let document = artifact_document(&program);
            std::fs::write(&out, encode(&document)?)
                .map_err(|error| Error::new(error.to_string()))?;
            Ok(RunResult {
                text: format!("{}\n", out.display()),
                code: 0,
            })
        }
        Command::Run {
            path,
            message,
            argv,
        } => {
            let program = load_program(&path)?;
            run_program(program, message, argv)
        }
    }
}

pub fn run_program(
    program: Program,
    message: Option<Json>,
    argv: Vec<String>,
) -> Result<RunResult, Error> {
    let mut world = World::start(program)?;
    world.set_argv(argv);
    if let Some(message) = message {
        world.inject(message)?;
    }
    finish(&mut world)?;
    let text = if world.suppress_result() {
        String::new()
    } else {
        doc::encode(&world.result_document())?
    };
    Ok(RunResult {
        text,
        code: world.exit_code().unwrap_or(0),
    })
}

fn finish(world: &mut World) -> Result<(), Error> {
    loop {
        world.run_until_idle()?;
        world.poll_io()?;
        if world.exit_code().is_some() {
            return Ok(());
        }
        let fired = world.fire_next_timer()?;
        world.poll_io()?;
        if world.exit_code().is_some() {
            return Ok(());
        }
        if world.outstanding() == 0 && !world.listening() && !fired {
            return Ok(());
        }
        if world.outstanding() > 0 || world.listening() {
            world.wait_io(std::time::Duration::from_millis(50))?;
        }
    }
}

fn artifact_path(source: &Path) -> PathBuf {
    let mut out = source.to_path_buf();
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("program.kith");
    if name.ends_with(".kith") {
        out.set_file_name(format!("{name}.ast"));
    } else {
        out.set_extension("kith.ast");
    }
    out
}
