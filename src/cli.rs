use crate::ast::Program;
use crate::codec::encode;
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
    },
}

pub fn parse_args(args: &[String]) -> Result<Command, Error> {
    let mut args = args.iter().skip(1);
    let command = args
        .next()
        .ok_or_else(|| Error::new("usage: kith <parse|check|compile|run> <file>"))?;
    match command.as_str() {
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
            while let Some(arg) = args.next() {
                if arg == "--message" {
                    let value = args
                        .next()
                        .ok_or_else(|| Error::new("kith run --message needs JSON"))?;
                    message = Some(
                        serde_json::from_str(value)
                            .map_err(|error| Error::new(error.to_string()))?,
                    );
                } else if path.is_none() {
                    path = Some(PathBuf::from(arg));
                } else {
                    return Err(Error::new(format!("unexpected argument {arg}")));
                }
            }
            Ok(Command::Run {
                path: path.ok_or_else(|| Error::new("usage: kith run <file> [--message JSON]"))?,
                message,
            })
        }
        other => Err(Error::new(format!(
            "unknown command {other}; expected parse, check, compile, or run"
        ))),
    }
}

pub fn run(command: Command) -> Result<String, Error> {
    match command {
        Command::Parse { path } => {
            let program = parse_file(&path)?;
            let tree =
                serde_json::to_value(&program).map_err(|error| Error::new(error.to_string()))?;
            encode(&tree)
        }
        Command::Check { path } => {
            let program = load_program(&path)?;
            World::check(&program)?;
            Ok("ok\n".into())
        }
        Command::Compile { path } => {
            let program = load_program(&path)?;
            World::check(&program)?;
            let out = artifact_path(&path);
            let document = artifact_document(&program);
            std::fs::write(&out, encode(&document)?)
                .map_err(|error| Error::new(error.to_string()))?;
            Ok(format!("{}\n", out.display()))
        }
        Command::Run { path, message } => {
            let program = load_program(&path)?;
            run_program(program, message)
        }
    }
}

pub fn run_program(program: Program, message: Option<Json>) -> Result<String, Error> {
    let mut world = World::start(program)?;
    if let Some(message) = message {
        world.inject(message)?;
        world.run()?;
    }
    encode(&world.result())
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
