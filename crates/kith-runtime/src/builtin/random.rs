//! Numeric draws. `(rand Lo Hi)` and the standard methods share one step.
//! `Random` keeps a separate generator seeded by its parent.

use crate::doc::{self, Doc};
use crate::error::Error;

/// Numerical Recipes LCG. One step of World RNG and of a seeded `Random`.
pub fn step(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    *state
}

/// `(rand Lo Hi)` after the caller has rejected `lo > hi`.
pub fn range(state: &mut u64, lo: f64, hi: f64) -> f64 {
    let draw = step(state);
    if lo.fract() == 0.0 && hi.fract() == 0.0 {
        let width = (hi as i64 - lo as i64 + 1) as u64;
        let pick = lo as i64 + (draw % width) as i64;
        return pick as f64;
    }
    let t = (draw as f64) / (u64::MAX as f64);
    lo + (hi - lo) * t
}

pub fn unit(state: &mut u64) -> f64 {
    let draw = step(state);
    (draw >> 11) as f64 / ((1u64 << 53) as f64)
}

fn unit_open(state: &mut u64) -> f64 {
    let draw = step(state);
    ((draw >> 11) as f64 + 1.0) / ((1u64 << 53) as f64)
}

pub fn uniform(state: &mut u64, lo: f64, hi: f64) -> Result<f64, Error> {
    if !lo.is_finite() || !hi.is_finite() || !(lo < hi) {
        return Err(Error::new("uniform low bound must be less than high bound"));
    }
    let draw = unit(state);
    Ok(lo + (hi - lo) * draw)
}

pub fn bernoulli(state: &mut u64, p: f64) -> Result<f64, Error> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(Error::new("bernoulli probability must be from 0 to 1"));
    }
    let draw = unit(state);
    Ok(if draw < p { 1.0 } else { 0.0 })
}

pub fn normal(state: &mut u64, mean: f64, deviation: f64) -> Result<f64, Error> {
    if !mean.is_finite() || !deviation.is_finite() || deviation <= 0.0 {
        return Err(Error::new("normal deviation must be positive"));
    }
    let u1 = unit_open(state);
    let u2 = unit_open(state);
    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    Ok(mean + deviation * z)
}

pub fn random_seed(value: &Doc) -> Result<u64, Error> {
    doc::as_u64(value).ok_or_else(|| Error::new("Random seed must be a non-negative whole number"))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Generator {
    pub state: u64,
}

impl Generator {
    pub fn new(state: u64) -> Self {
        Self { state }
    }
}

pub fn drive(mut generator: Generator, message: &Doc) -> (Generator, Vec<Doc>) {
    let reply = match command(message) {
        Command::Unit => number(unit(&mut generator.state)),
        Command::Uniform(lo, hi) => match uniform(&mut generator.state, lo, hi) {
            Ok(value) => number(value),
            Err(_) => error("bounds"),
        },
        Command::Range(lo, hi) => {
            if lo > hi {
                error("bounds")
            } else {
                number(range(&mut generator.state, lo, hi))
            }
        }
        Command::Bernoulli(p) => match bernoulli(&mut generator.state, p) {
            Ok(value) => number(value),
            Err(_) => error("probability"),
        },
        Command::Normal(mean, deviation) => match normal(&mut generator.state, mean, deviation) {
            Ok(value) => number(value),
            Err(_) => error("deviation"),
        },
        Command::Bad(reason) => error(reason),
        Command::Unknown => error("unknown"),
    };
    (generator, vec![reply])
}

enum Command {
    Unit,
    Uniform(f64, f64),
    Range(f64, f64),
    Bernoulli(f64),
    Normal(f64, f64),
    Bad(&'static str),
    Unknown,
}

fn command(message: &Doc) -> Command {
    let (name, args) = match head(message) {
        Some(parts) => parts,
        None => return Command::Unknown,
    };
    match name.as_str() {
        "Unit" => {
            if args.is_empty() {
                Command::Unit
            } else {
                Command::Unknown
            }
        }
        "Uniform" => {
            pair(&args).map_or(Command::Bad("bounds"), |(lo, hi)| Command::Uniform(lo, hi))
        }
        "Range" => pair(&args).map_or(Command::Bad("bounds"), |(lo, hi)| Command::Range(lo, hi)),
        "Bernoulli" => one(&args).map_or(Command::Bad("probability"), Command::Bernoulli),
        "Normal" => pair(&args).map_or(Command::Bad("deviation"), |(mean, deviation)| {
            Command::Normal(mean, deviation)
        }),
        _ => Command::Unknown,
    }
}

fn head(message: &Doc) -> Option<(String, Vec<Doc>)> {
    if let Some(name) = doc::as_str(message) {
        return Some((name, Vec::new()));
    }
    let items = doc::items(message)?;
    let name = doc::as_str(items.first()?)?;
    Some((name, items.into_iter().skip(1).collect()))
}

fn one(args: &[Doc]) -> Option<f64> {
    if args.len() == 1 {
        doc::as_number(&args[0])
    } else {
        None
    }
}

fn pair(args: &[Doc]) -> Option<(f64, f64)> {
    if args.len() == 2 {
        Some((doc::as_number(&args[0])?, doc::as_number(&args[1])?))
    } else {
        None
    }
}

fn number(value: f64) -> Doc {
    doc::array([doc::string("Number"), doc::number(value)])
}

fn error(reason: &str) -> Doc {
    doc::array([doc::string("Error"), doc::string(reason)])
}

pub fn to_document(generator: &Generator) -> Doc {
    doc::object([
        ("kind", doc::string("random")),
        ("state", doc::string(generator.state.to_string())),
    ])
}

pub fn from_document(value: &Doc) -> Option<Generator> {
    if doc::as_str(&doc::get(value, "kind")?).as_deref() != Some("random") {
        return None;
    }
    let state = doc::as_str(&doc::get(value, "state")?)?.parse().ok()?;
    Some(Generator { state })
}
