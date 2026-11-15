//! Pure slash paths. Nothing here reads the operating system.

use crate::doc::{self, Doc};
use crate::error::Error;

pub fn call(name: &str, args: &[Doc]) -> Result<Option<Doc>, Error> {
    let value = match name {
        "join" => join(args)?,
        "dirname" => dirname(args)?,
        "basename" => basename(args)?,
        "extname" => extname(args)?,
        "resolve" => resolve(args)?,
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn strings(name: &str, args: &[Doc]) -> Result<Vec<String>, Error> {
    if args.is_empty() {
        return Err(Error::new(format!("{name} expected a path")));
    }
    args.iter()
        .map(|arg| doc::as_str(arg).ok_or_else(|| Error::new(format!("{name} expected text"))))
        .collect()
}

fn join(args: &[Doc]) -> Result<Doc, Error> {
    let parts = strings("join", args)?;
    Ok(doc::string(join_parts(&parts)))
}

pub fn join_parts(parts: &[String]) -> String {
    let mut out = String::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if part.starts_with('/') {
            out = part.clone();
            continue;
        }
        if out.is_empty() || out.ends_with('/') {
            out.push_str(part);
        } else {
            out.push('/');
            out.push_str(part);
        }
    }
    out
}

fn dirname(args: &[Doc]) -> Result<Doc, Error> {
    let parts = strings("dirname", args)?;
    if parts.len() != 1 {
        return Err(Error::new("dirname expected one path"));
    }
    Ok(doc::string(dir_of(&parts[0])))
}

fn dir_of(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return "/".into();
    }
    match trimmed.rfind('/') {
        Some(0) => "/".into(),
        Some(index) => trimmed[..index].to_string(),
        None => ".".into(),
    }
}

fn basename(args: &[Doc]) -> Result<Doc, Error> {
    let parts = strings("basename", args)?;
    if parts.len() != 1 {
        return Err(Error::new("basename expected one path"));
    }
    Ok(doc::string(base_of(&parts[0])))
}

fn base_of(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    match trimmed.rfind('/') {
        Some(index) => trimmed[index + 1..].to_string(),
        None => trimmed.to_string(),
    }
}

fn extname(args: &[Doc]) -> Result<Doc, Error> {
    let parts = strings("extname", args)?;
    if parts.len() != 1 {
        return Err(Error::new("extname expected one path"));
    }
    let base = base_of(&parts[0]);
    let Some(index) = base.rfind('.') else {
        return Ok(doc::string(""));
    };
    if index == 0 {
        return Ok(doc::string(""));
    }
    Ok(doc::string(base[index..].to_string()))
}

fn resolve(args: &[Doc]) -> Result<Doc, Error> {
    let parts = strings("resolve", args)?;
    if parts.len() != 2 {
        return Err(Error::new("resolve expected a base and a path"));
    }
    if parts[1].starts_with('/') {
        return Ok(doc::string(parts[1].clone()));
    }
    Ok(doc::string(join_parts(&[
        parts[0].clone(),
        parts[1].clone(),
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // req:edges-007
    fn slash_paths_do_not_read_the_os() {
        let joined = call(
            "join",
            &[doc::string("a"), doc::string(""), doc::string("b")],
        )
        .unwrap()
        .unwrap();
        assert_eq!(doc::as_str(&joined).as_deref(), Some("a/b"));
        let restarted = call(
            "join",
            &[doc::string("a"), doc::string("/b"), doc::string("c")],
        )
        .unwrap()
        .unwrap();
        assert_eq!(doc::as_str(&restarted).as_deref(), Some("/b/c"));
        assert_eq!(
            doc::as_str(&call("dirname", &[doc::string("a")]).unwrap().unwrap()).as_deref(),
            Some(".")
        );
        assert_eq!(
            doc::as_str(&call("dirname", &[doc::string("/")]).unwrap().unwrap()).as_deref(),
            Some("/")
        );
        assert_eq!(
            doc::as_str(&call("basename", &[doc::string("/")]).unwrap().unwrap()).as_deref(),
            Some("")
        );
        assert_eq!(
            doc::as_str(
                &call("extname", &[doc::string(".gitignore")])
                    .unwrap()
                    .unwrap()
            )
            .as_deref(),
            Some("")
        );
        assert_eq!(
            doc::as_str(
                &call("extname", &[doc::string("a.tar.gz")])
                    .unwrap()
                    .unwrap()
            )
            .as_deref(),
            Some(".gz")
        );
        let resolved = call("resolve", &[doc::string("/base"), doc::string("../x")])
            .unwrap()
            .unwrap();
        assert_eq!(doc::as_str(&resolved).as_deref(), Some("/base/../x"));
        let absolute = call("resolve", &[doc::string("/base"), doc::string("/abs")])
            .unwrap()
            .unwrap();
        assert_eq!(doc::as_str(&absolute).as_deref(), Some("/abs"));
    }
}
