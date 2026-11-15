//! Host drivers for the edge modules. Messages and replies are data-codec
//! documents. Operating-system handles live beside the driver and are not
//! part of its snapshot.

use crate::doc::{self, Doc};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CHUNK: usize = 4096;
const IO_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Socket,
    Listener,
    Http,
    Server,
    Stdin,
    Stdout,
    Stderr,
    File,
    Crypto,
    Process,
}

#[derive(Clone, Debug, PartialEq)]
enum FileMode {
    Read { path: String, offset: u64 },
    Write { path: String },
    Names { names: Vec<String>, index: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    status: u64,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    kind: Kind,
    busy: bool,
    busy_ids: Vec<String>,
    script: Option<Vec<Vec<Doc>>>,
    seq: u64,
    open: bool,
    posting: bool,
    pending: Vec<u8>,
    body_open: bool,
    file: Option<FileMode>,
    gets: Vec<String>,
    responses: Vec<(String, Response)>,
    post: Option<Post>,
    listening: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Post {
    https: bool,
    host: String,
    port: u16,
    path: String,
    headers: Vec<(String, String)>,
}

pub struct Host<'a> {
    pub argv: &'a [String],
    pub env: &'a HashMap<String, String>,
    pub rng: &'a mut u64,
}

pub struct Outcome {
    pub edge: Edge,
    pub replies: Vec<Doc>,
    pub job: Option<Job>,
    pub exit: Option<u8>,
    pub stdout: Option<Vec<u8>>,
    pub stderr: Option<Vec<u8>>,
    pub listen: Option<ListenOn>,
    pub stop_listen: bool,
}

pub struct ListenOn {
    pub host: String,
    pub port: u16,
    pub server: bool,
}

pub enum Job {
    Connect { host: String, port: u16 },
    ReadSock,
    WriteSock { bytes: Vec<u8> },
    CloseSock,
    ReadConn { id: String },
    WriteConn { id: String, bytes: Vec<u8> },
    CloseConn { id: String },
    Http(HttpRequest),
    ReadStdin,
    WriteResponse { id: String },
}

pub struct HttpRequest {
    pub https: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

pub struct Incoming {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub stream: TcpStream,
}

pub struct IoDone {
    pub actor: u64,
    pub replies: Vec<Doc>,
    pub clear_busy: bool,
    pub busy_id: Option<String>,
    pub sock: Option<TcpStream>,
    pub body: Option<Vec<u8>>,
    pub accepted: Option<TcpStream>,
    pub request: Option<Incoming>,
    pub track: bool,
}

pub struct IoShared {
    socks: Mutex<HashMap<u64, TcpStream>>,
    conns: Mutex<HashMap<(u64, String), TcpStream>>,
    bodies: Mutex<HashMap<u64, Vec<u8>>>,
    stops: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl IoShared {
    pub fn new() -> Self {
        Self {
            socks: Mutex::new(HashMap::new()),
            conns: Mutex::new(HashMap::new()),
            bodies: Mutex::new(HashMap::new()),
            stops: Mutex::new(HashMap::new()),
        }
    }

    pub fn stop_all(&self) {
        for stop in self.stops.lock().expect("stops").values() {
            stop.store(true, Ordering::Relaxed);
        }
    }

    pub fn insert_sock(&self, actor: u64, stream: TcpStream) {
        self.socks.lock().expect("socks").insert(actor, stream);
    }

    pub fn insert_conn(&self, actor: u64, id: String, stream: TcpStream) {
        self.conns
            .lock()
            .expect("conns")
            .insert((actor, id), stream);
    }

    pub fn insert_body(&self, actor: u64, body: Vec<u8>) {
        self.bodies.lock().expect("bodies").insert(actor, body);
    }
}

impl Edge {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            busy: false,
            busy_ids: Vec::new(),
            script: None,
            seq: 0,
            open: true,
            posting: false,
            pending: Vec::new(),
            body_open: false,
            file: None,
            gets: Vec::new(),
            responses: Vec::new(),
            post: None,
            listening: false,
        }
    }

    pub fn install_script(&mut self, batches: Vec<Vec<Doc>>) {
        self.script = Some(batches);
    }

    pub fn clear_busy(&mut self) {
        self.busy = false;
        self.busy_ids.clear();
    }

    pub fn clear_one(&mut self, id: Option<&str>) {
        match id {
            Some(id) => self.busy_ids.retain(|item| item != id),
            None => self.busy = false,
        }
    }

    pub fn set_listening(&mut self, listening: bool) {
        self.listening = listening;
    }

    pub fn note_get(&mut self, id: String) {
        self.gets.push(id);
    }

    pub fn open_body(&mut self) {
        self.body_open = true;
    }

    pub fn next_id(&mut self, prefix: &str) -> String {
        self.seq += 1;
        format!("{prefix}{}", self.seq)
    }

    pub fn to_document(&self) -> Doc {
        let script = match &self.script {
            Some(batches) => doc::array(batches.iter().cloned().map(doc::array)),
            None => doc::null(),
        };
        let file = match &self.file {
            None => doc::null(),
            Some(FileMode::Read { path, offset }) => doc::object([
                ("mode", doc::string("read")),
                ("path", doc::string(path)),
                ("offset", doc::number(*offset as f64)),
            ]),
            Some(FileMode::Write { path }) => {
                doc::object([("mode", doc::string("write")), ("path", doc::string(path))])
            }
            Some(FileMode::Names { names, index }) => doc::object([
                ("mode", doc::string("list")),
                ("names", doc::array(names.iter().cloned().map(doc::string))),
                ("index", doc::number(*index as f64)),
            ]),
        };
        doc::object([
            ("kind", doc::string(kind_name(&self.kind))),
            ("busy", doc::boolean(self.busy)),
            (
                "busyIds",
                doc::array(self.busy_ids.iter().cloned().map(doc::string)),
            ),
            ("script", script),
            ("seq", doc::number(self.seq as f64)),
            ("open", doc::boolean(self.open)),
            ("posting", doc::boolean(self.posting)),
            ("pending", doc::bytes(self.pending.clone())),
            ("bodyOpen", doc::boolean(self.body_open)),
            ("file", file),
            (
                "gets",
                doc::array(self.gets.iter().cloned().map(doc::string)),
            ),
            ("listening", doc::boolean(self.listening)),
        ])
    }

    pub fn from_document(value: &Doc) -> Option<Self> {
        let name = doc::as_str(&doc::get(value, "kind")?)?;
        let kind = kind_from(&name)?;
        let mut edge = Self::new(kind);
        edge.busy =
            doc::as_bool(&doc::get(value, "busy").unwrap_or_else(doc::null)).unwrap_or(false);
        edge.busy_ids = doc::items(&doc::get(value, "busyIds").unwrap_or_else(doc::null))
            .unwrap_or_default()
            .iter()
            .filter_map(|item| doc::as_str(item))
            .collect();
        edge.script = match doc::get(value, "script") {
            Some(script) if !doc::is_null(&script) => Some(
                doc::items(&script)
                    .unwrap_or_default()
                    .iter()
                    .map(|batch| doc::items(batch).unwrap_or_default())
                    .collect(),
            ),
            _ => None,
        };
        edge.seq = doc::as_u64(&doc::get(value, "seq").unwrap_or_else(doc::null)).unwrap_or(0);
        edge.open =
            doc::as_bool(&doc::get(value, "open").unwrap_or_else(doc::null)).unwrap_or(true);
        edge.posting =
            doc::as_bool(&doc::get(value, "posting").unwrap_or_else(doc::null)).unwrap_or(false);
        edge.pending = doc::as_bytes(&doc::get(value, "pending").unwrap_or_else(doc::null))
            .unwrap_or_default();
        edge.body_open =
            doc::as_bool(&doc::get(value, "bodyOpen").unwrap_or_else(doc::null)).unwrap_or(false);
        edge.gets = doc::items(&doc::get(value, "gets").unwrap_or_else(doc::null))
            .unwrap_or_default()
            .iter()
            .filter_map(|item| doc::as_str(item))
            .collect();
        edge.listening =
            doc::as_bool(&doc::get(value, "listening").unwrap_or_else(doc::null)).unwrap_or(false);
        edge.file = doc::get(value, "file").and_then(|file| {
            if doc::is_null(&file) {
                return None;
            }
            match doc::as_str(&doc::get(&file, "mode")?).as_deref() {
                Some("read") => Some(FileMode::Read {
                    path: doc::as_str(&doc::get(&file, "path")?)?,
                    offset: doc::as_u64(&doc::get(&file, "offset").unwrap_or_else(doc::null))
                        .unwrap_or(0),
                }),
                Some("write") => Some(FileMode::Write {
                    path: doc::as_str(&doc::get(&file, "path")?)?,
                }),
                Some("list") => Some(FileMode::Names {
                    names: doc::items(&doc::get(&file, "names").unwrap_or_else(doc::null))
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|item| doc::as_str(item))
                        .collect(),
                    index: doc::as_u64(&doc::get(&file, "index").unwrap_or_else(doc::null))
                        .unwrap_or(0) as usize,
                }),
                _ => None,
            }
        });
        Some(edge)
    }
}

pub fn kind_for(name: &str) -> Option<Kind> {
    Some(match name {
        "Socket" => Kind::Socket,
        "Listener" => Kind::Listener,
        "Http" => Kind::Http,
        "Server" => Kind::Server,
        "Stdin" => Kind::Stdin,
        "Stdout" => Kind::Stdout,
        "Stderr" => Kind::Stderr,
        "File" => Kind::File,
        "Crypto" => Kind::Crypto,
        "Process" => Kind::Process,
        _ => return None,
    })
}

fn kind_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Socket => "socket",
        Kind::Listener => "listener",
        Kind::Http => "http",
        Kind::Server => "server",
        Kind::Stdin => "stdin",
        Kind::Stdout => "stdout",
        Kind::Stderr => "stderr",
        Kind::File => "file",
        Kind::Crypto => "crypto",
        Kind::Process => "process",
    }
}

fn kind_from(name: &str) -> Option<Kind> {
    Some(match name {
        "socket" => Kind::Socket,
        "listener" => Kind::Listener,
        "http" => Kind::Http,
        "server" => Kind::Server,
        "stdin" => Kind::Stdin,
        "stdout" => Kind::Stdout,
        "stderr" => Kind::Stderr,
        "file" => Kind::File,
        "crypto" => Kind::Crypto,
        "process" => Kind::Process,
        _ => return None,
    })
}

struct Command {
    name: String,
    args: Vec<Doc>,
}

fn command(message: &Doc) -> Command {
    if let Some(text) = doc::as_str(message) {
        return Command {
            name: text,
            args: Vec::new(),
        };
    }
    if let Some(items) = doc::items(message) {
        if let Some(name) = items.first().and_then(|item| doc::as_str(item)) {
            return Command {
                name,
                args: items.into_iter().skip(1).collect(),
            };
        }
    }
    Command {
        name: String::new(),
        args: Vec::new(),
    }
}

fn reply(name: &str, rest: impl IntoIterator<Item = Doc>) -> Doc {
    let mut items = vec![doc::string(name)];
    items.extend(rest);
    doc::array(items)
}

fn failure(id: Option<&str>, reason: &str) -> Doc {
    match id {
        Some(id) => reply("Error", [doc::string(id), doc::string(reason)]),
        None => reply("Error", [doc::string(reason)]),
    }
}

fn text_arg(args: &[Doc], index: usize) -> Option<String> {
    doc::as_str(args.get(index)?)
}

fn bytes_arg(args: &[Doc], index: usize) -> Option<Vec<u8>> {
    doc::as_bytes(args.get(index)?)
}

/// A server push body is raw bytes, or text encoded as UTF-8.
fn push_body(args: &[Doc], index: usize) -> Option<Vec<u8>> {
    if let Some(bytes) = bytes_arg(args, index) {
        return Some(bytes);
    }
    text_arg(args, index).map(String::into_bytes)
}

fn headers_arg(value: &Doc) -> Result<Vec<(String, String)>, ()> {
    if doc::is_null(value) {
        return Ok(Vec::new());
    }
    let entries = doc::entries(value).ok_or(())?;
    let mut headers = Vec::new();
    for (name, value) in entries {
        headers.push((name, doc::as_str(&value).ok_or(())?));
    }
    Ok(headers)
}

fn header_doc(headers: &[(String, String)]) -> Doc {
    doc::object(
        headers
            .iter()
            .map(|(name, value)| (name.clone(), doc::string(value))),
    )
}

pub fn step(edge: Edge, actor: u64, message: &Doc, host: &mut Host, io: &IoShared) -> Outcome {
    let command = command(message);
    let multiplexed = matches!(edge.kind, Kind::Listener | Kind::Server);
    let id = if multiplexed {
        text_arg(&command.args, 0)
    } else {
        None
    };
    if edge.script.is_some() {
        return scripted(edge, &command, id);
    }
    if command.name.is_empty() || !known(&edge.kind, &command.name) {
        let id = id.as_deref().filter(|_| multiplexed);
        return done(edge, vec![failure(id, "unknown")]);
    }
    if busy_for(&edge, &command, id.as_deref()) {
        return done(edge, vec![failure(id.as_deref(), "busy")]);
    }
    match edge.kind {
        Kind::Socket => socket(edge, command),
        Kind::Listener => listener(edge, command),
        Kind::Http => http(edge, command),
        Kind::Server => server(edge, command),
        Kind::Stdin => stdin(edge, command),
        Kind::Stdout => writer(edge, command, true),
        Kind::Stderr => writer(edge, command, false),
        Kind::File => file(edge, command),
        Kind::Crypto => crypto(edge, command, host),
        Kind::Process => process(edge, command, host),
    }
    .body(actor, io)
}

fn known(kind: &Kind, name: &str) -> bool {
    match kind {
        Kind::Socket => matches!(name, "Connect" | "Pull" | "Push" | "Close"),
        Kind::Listener => matches!(name, "Listen" | "Pull" | "Push" | "Close" | "Stop"),
        Kind::Http => matches!(name, "GET" | "POST" | "Pull" | "Push" | "BodyEnd"),
        Kind::Server => matches!(
            name,
            "Listen" | "Pull" | "Push" | "Status" | "BodyEnd" | "Stop"
        ),
        Kind::Stdin => name == "Pull",
        Kind::Stdout | Kind::Stderr => matches!(name, "Push" | "Close"),
        Kind::File => matches!(name, "Open" | "Pull" | "Push" | "Close" | "Remove" | "List"),
        Kind::Crypto => name == "Random",
        Kind::Process => matches!(name, "Argv" | "Env" | "Exit"),
    }
}

fn busy_for(edge: &Edge, command: &Command, id: Option<&str>) -> bool {
    let tracked = match edge.kind {
        Kind::Listener | Kind::Server => matches!(command.name.as_str(), "Pull" | "Push" | "Close"),
        Kind::Socket | Kind::Stdin | Kind::File | Kind::Http => {
            matches!(
                command.name.as_str(),
                "Pull" | "Push" | "Connect" | "GET" | "POST" | "BodyEnd"
            )
        }
        _ => false,
    };
    tracked
        && match id {
            Some(id) => edge.busy_ids.iter().any(|item| item == id),
            None => edge.busy,
        }
}

fn scripted(mut edge: Edge, command: &Command, id: Option<String>) -> Outcome {
    let multiplexed = matches!(edge.kind, Kind::Listener | Kind::Server);
    if command.name.is_empty() || !known(&edge.kind, &command.name) {
        let shown = id.as_deref().filter(|_| multiplexed);
        return done(edge, vec![failure(shown, "unknown")]);
    }
    if busy_for(&edge, command, id.as_deref()) {
        return done(edge, vec![failure(id.as_deref(), "busy")]);
    }
    let Some(script) = edge.script.as_mut() else {
        return done(edge, vec![]);
    };
    if script.is_empty() {
        edge.script = None;
        return done(edge, vec![failure(id.as_deref(), "closed")]);
    }
    let batch = script.remove(0);
    if batch.is_empty() {
        mark_busy(&mut edge, id);
        return done(edge, vec![]);
    }
    done(edge, batch)
}

fn mark_busy(edge: &mut Edge, id: Option<String>) {
    if let Some(id) = id {
        edge.busy_ids.push(id);
    } else {
        edge.busy = true;
    }
}

fn done(edge: Edge, replies: Vec<Doc>) -> Outcome {
    Outcome {
        edge,
        replies,
        job: None,
        exit: None,
        stdout: None,
        stderr: None,
        listen: None,
        stop_listen: false,
    }
}

fn socket(mut edge: Edge, command: Command) -> Outcome {
    match command.name.as_str() {
        "Connect" => {
            let Some(host) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "host")]);
            };
            let Some(port) = doc::as_u64(command.args.get(1).unwrap_or(&doc::null())) else {
                return done(edge, vec![failure(None, "port")]);
            };
            if port > u16::MAX as u64 {
                return done(edge, vec![failure(None, "port")]);
            }
            mark_busy(&mut edge, None);
            let mut outcome = done(edge, vec![]);
            outcome.job = Some(Job::Connect {
                host,
                port: port as u16,
            });
            outcome
        }
        "Pull" => {
            mark_busy(&mut edge, None);
            let mut outcome = done(edge, vec![]);
            outcome.job = Some(Job::ReadSock);
            outcome
        }
        "Push" => match bytes_arg(&command.args, 0) {
            Some(bytes) => {
                mark_busy(&mut edge, None);
                let mut outcome = done(edge, vec![]);
                outcome.job = Some(Job::WriteSock { bytes });
                outcome
            }
            None => done(edge, vec![failure(None, "bytes")]),
        },
        "Close" => {
            mark_busy(&mut edge, None);
            let mut outcome = done(edge, vec![]);
            outcome.job = Some(Job::CloseSock);
            outcome
        }
        _ => done(edge, vec![failure(None, "unknown")]),
    }
}

fn listener(mut edge: Edge, command: Command) -> Outcome {
    match command.name.as_str() {
        "Listen" => {
            let Some(host) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "host")]);
            };
            let Some(port) = doc::as_u64(command.args.get(1).unwrap_or(&doc::null())) else {
                return done(edge, vec![failure(None, "port")]);
            };
            if port > u16::MAX as u64 {
                return done(edge, vec![failure(None, "port")]);
            }
            edge.listening = true;
            let mut outcome = done(edge, vec![]);
            outcome.listen = Some(ListenOn {
                host,
                port: port as u16,
                server: false,
            });
            outcome
        }
        "Pull" => conn_job(&mut edge, &command, JobKind::Read),
        "Push" => conn_job(&mut edge, &command, JobKind::Write),
        "Close" => conn_job(&mut edge, &command, JobKind::Close),
        "Stop" => {
            edge.listening = false;
            let mut outcome = done(edge, vec![doc::string("Stopped")]);
            outcome.stop_listen = true;
            outcome
        }
        _ => done(edge, vec![failure(None, "unknown")]),
    }
}

enum JobKind {
    Read,
    Write,
    Close,
}

fn conn_job(edge: &mut Edge, command: &Command, kind: JobKind) -> Outcome {
    let Some(id) = text_arg(&command.args, 0) else {
        return done(edge.clone(), vec![failure(None, "id")]);
    };
    let job = match kind {
        JobKind::Read => Job::ReadConn { id: id.clone() },
        JobKind::Write => match bytes_arg(&command.args, 1) {
            Some(bytes) => Job::WriteConn {
                id: id.clone(),
                bytes,
            },
            None => return done(edge.clone(), vec![failure(Some(&id), "bytes")]),
        },
        JobKind::Close => Job::CloseConn { id: id.clone() },
    };
    mark_busy(edge, Some(id));
    let mut outcome = done(edge.clone(), vec![]);
    outcome.job = Some(job);
    outcome
}

fn http(mut edge: Edge, command: Command) -> Outcome {
    match command.name.as_str() {
        "GET" | "POST" => {
            let Some(url) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "url")]);
            };
            let headers = match command.args.get(1) {
                None => Vec::new(),
                Some(value) => match headers_arg(value) {
                    Ok(headers) => headers,
                    Err(()) => return done(edge, vec![failure(None, "headers")]),
                },
            };
            let Ok(target) = parse_url(&url) else {
                return done(edge, vec![failure(None, "url")]);
            };
            if command.name == "POST" {
                edge.posting = true;
                edge.pending.clear();
                edge.post = Some(Post {
                    https: target.https,
                    host: target.host,
                    port: target.port,
                    path: target.path,
                    headers,
                });
                return done(edge, vec![doc::string("Ready")]);
            }
            mark_busy(&mut edge, None);
            let mut outcome = done(edge, vec![]);
            outcome.job = Some(Job::Http(HttpRequest {
                https: target.https,
                host: target.host,
                port: target.port,
                path: target.path,
                method: "GET".into(),
                headers,
                body: None,
            }));
            outcome
        }
        "Push" => {
            if !edge.posting {
                return done(edge, vec![failure(None, "closed")]);
            }
            let Some(bytes) = bytes_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "bytes")]);
            };
            edge.pending.extend(bytes);
            done(edge, vec![doc::string("Pushed")])
        }
        "BodyEnd" => {
            if !edge.posting {
                return done(edge, vec![failure(None, "closed")]);
            }
            let Some(post) = edge.post.clone() else {
                return done(edge, vec![failure(None, "closed")]);
            };
            let body = std::mem::take(&mut edge.pending);
            edge.posting = false;
            mark_busy(&mut edge, None);
            let mut outcome = done(edge, vec![]);
            outcome.job = Some(Job::Http(HttpRequest {
                https: post.https,
                host: post.host,
                port: post.port,
                path: post.path,
                method: "POST".into(),
                headers: post.headers,
                body: Some(body),
            }));
            outcome
        }
        "Pull" => done(edge, vec![]),
        _ => done(edge, vec![failure(None, "unknown")]),
    }
}

struct Target {
    https: bool,
    host: String,
    port: u16,
    path: String,
}

fn parse_url(url: &str) -> Result<Target, ()> {
    let (https, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        return Err(());
    };
    if rest.is_empty() {
        return Err(());
    }
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".into()),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port))
            if !host.is_empty() && port.chars().all(|char| char.is_ascii_digit()) =>
        {
            (host.to_string(), port.parse::<u16>().map_err(|_| ())?)
        }
        _ => (authority.to_string(), if https { 443 } else { 80 }),
    };
    if host.is_empty() {
        return Err(());
    }
    Ok(Target {
        https,
        host,
        port,
        path,
    })
}

fn server(mut edge: Edge, command: Command) -> Outcome {
    match command.name.as_str() {
        "Listen" => {
            let (host, port_doc) = if command.args.len() >= 2 {
                let Some(host) = text_arg(&command.args, 0) else {
                    return done(edge, vec![failure(None, "host")]);
                };
                if host.is_empty() {
                    return done(edge, vec![failure(None, "host")]);
                }
                (host, command.args.get(1).cloned().unwrap_or_else(doc::null))
            } else {
                (
                    "127.0.0.1".into(),
                    command.args.first().cloned().unwrap_or_else(doc::null),
                )
            };
            let Some(port) = doc::as_u64(&port_doc) else {
                return done(edge, vec![failure(None, "port")]);
            };
            if port > u16::MAX as u64 {
                return done(edge, vec![failure(None, "port")]);
            }
            edge.listening = true;
            let mut outcome = done(edge, vec![]);
            outcome.listen = Some(ListenOn {
                host,
                port: port as u16,
                server: true,
            });
            outcome
        }
        "Pull" => {
            let Some(id) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "id")]);
            };
            if let Some(index) = edge.gets.iter().position(|item| item == &id) {
                edge.gets.remove(index);
                return done(edge, vec![reply("End", [doc::string(id)])]);
            }
            conn_job(&mut edge, &command, JobKind::Read)
        }
        "Status" => {
            let Some(id) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "id")]);
            };
            let Some(status) = doc::as_u64(command.args.get(1).unwrap_or(&doc::null())) else {
                return done(edge, vec![failure(Some(&id), "status")]);
            };
            let headers = match command.args.get(2) {
                None => Vec::new(),
                Some(value) => match headers_arg(value) {
                    Ok(headers) => headers,
                    Err(()) => return done(edge, vec![failure(Some(&id), "headers")]),
                },
            };
            edge.responses.retain(|(name, _)| name != &id);
            edge.responses.push((
                id,
                Response {
                    status,
                    headers,
                    body: Vec::new(),
                },
            ));
            done(edge, vec![])
        }
        "Push" => {
            let Some(id) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "id")]);
            };
            let Some(bytes) = push_body(&command.args, 1) else {
                return done(edge, vec![failure(Some(&id), "bytes")]);
            };
            let Some((_, response)) = edge.responses.iter_mut().find(|(name, _)| name == &id)
            else {
                return done(edge, vec![failure(Some(&id), "status")]);
            };
            response.body.extend(bytes);
            done(edge, vec![reply("Pushed", [doc::string(id)])])
        }
        "BodyEnd" => {
            let Some(id) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "id")]);
            };
            if !edge.responses.iter().any(|(name, _)| name == &id) {
                return done(edge, vec![failure(Some(&id), "status")]);
            }
            mark_busy(&mut edge, Some(id.clone()));
            let mut outcome = done(edge, vec![]);
            outcome.job = Some(Job::WriteResponse { id });
            outcome
        }
        "Stop" => {
            edge.listening = false;
            let mut outcome = done(edge, vec![doc::string("Stopped")]);
            outcome.stop_listen = true;
            outcome
        }
        _ => done(edge, vec![failure(None, "unknown")]),
    }
}

fn stdin(mut edge: Edge, command: Command) -> Outcome {
    if command.name != "Pull" {
        return done(edge, vec![failure(None, "unknown")]);
    }
    mark_busy(&mut edge, None);
    let mut outcome = done(edge, vec![]);
    outcome.job = Some(Job::ReadStdin);
    outcome
}

fn writer(mut edge: Edge, command: Command, stdout: bool) -> Outcome {
    match command.name.as_str() {
        "Push" => {
            if !edge.open {
                return done(edge, vec![failure(None, "closed")]);
            }
            let Some(bytes) = bytes_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "bytes")]);
            };
            let mut outcome = done(edge, vec![doc::string("Pushed")]);
            if stdout {
                outcome.stdout = Some(bytes);
            } else {
                outcome.stderr = Some(bytes);
            }
            outcome
        }
        "Close" => {
            if !edge.open {
                return done(edge, vec![failure(None, "closed")]);
            }
            edge.open = false;
            done(edge, vec![doc::string("Closed")])
        }
        _ => done(edge, vec![failure(None, "unknown")]),
    }
}

fn file(mut edge: Edge, command: Command) -> Outcome {
    match command.name.as_str() {
        "Open" => {
            let Some(path) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "path")]);
            };
            match text_arg(&command.args, 1).as_deref() {
                Some("read") => match std::fs::File::open(&path) {
                    Ok(_) => {
                        edge.file = Some(FileMode::Read { path, offset: 0 });
                        done(edge, vec![doc::string("Opened")])
                    }
                    Err(error) => done(edge, vec![failure(None, &file_reason(&path, &error))]),
                },
                Some("write") => {
                    if missing_parent(&path) {
                        return done(edge, vec![failure(None, "missing parent")]);
                    }
                    match std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(&path)
                    {
                        Ok(_) => {
                            edge.file = Some(FileMode::Write { path });
                            done(edge, vec![doc::string("Opened")])
                        }
                        Err(error) => done(edge, vec![failure(None, &file_reason(&path, &error))]),
                    }
                }
                _ => done(edge, vec![failure(None, "mode")]),
            }
        }
        "Pull" => match edge.file.clone() {
            Some(FileMode::Read { path, offset }) => match read_file(&path, offset) {
                Ok(None) => {
                    edge.file = None;
                    done(edge, vec![doc::string("End")])
                }
                Ok(Some(bytes)) => {
                    let next = offset + bytes.len() as u64;
                    edge.file = Some(FileMode::Read { path, offset: next });
                    done(edge, vec![reply("Chunk", [doc::bytes(bytes)])])
                }
                Err(reason) => done(edge, vec![failure(None, &reason)]),
            },
            Some(FileMode::Names { names, index }) => {
                if index >= names.len() {
                    edge.file = None;
                    done(edge, vec![doc::string("End")])
                } else {
                    let name = names[index].clone();
                    edge.file = Some(FileMode::Names {
                        names,
                        index: index + 1,
                    });
                    done(edge, vec![reply("Chunk", [doc::string(name)])])
                }
            }
            _ => done(edge, vec![failure(None, "closed")]),
        },
        "Push" => {
            let Some(FileMode::Write { path }) = edge.file.clone() else {
                return done(edge, vec![failure(None, "closed")]);
            };
            let Some(bytes) = bytes_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "bytes")]);
            };
            match std::fs::OpenOptions::new().append(true).open(&path) {
                Ok(mut file) => match file.write_all(&bytes) {
                    Ok(()) => done(edge, vec![doc::string("Pushed")]),
                    Err(error) => done(edge, vec![failure(None, &error.to_string())]),
                },
                Err(error) => done(edge, vec![failure(None, &error.to_string())]),
            }
        }
        "Close" => {
            edge.file = None;
            done(edge, vec![doc::string("Closed")])
        }
        "Remove" => {
            let Some(path) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "path")]);
            };
            let result = if std::path::Path::new(&path).is_dir() {
                std::fs::remove_dir(&path)
            } else {
                std::fs::remove_file(&path)
            };
            match result {
                Ok(()) => done(edge, vec![doc::string("Removed")]),
                Err(error) => done(edge, vec![failure(None, &error.to_string())]),
            }
        }
        "List" => {
            let Some(path) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "path")]);
            };
            match std::fs::read_dir(&path) {
                Ok(entries) => {
                    let mut names = Vec::new();
                    for entry in entries {
                        let Ok(entry) = entry else { continue };
                        if let Some(name) = entry.file_name().to_str() {
                            names.push(name.to_string());
                        }
                    }
                    names.sort();
                    edge.file = Some(FileMode::Names { names, index: 0 });
                    done(edge, vec![doc::string("Opened")])
                }
                Err(error) => done(edge, vec![failure(None, &error.to_string())]),
            }
        }
        _ => done(edge, vec![failure(None, "unknown")]),
    }
}

fn missing_parent(path: &str) -> bool {
    match std::path::Path::new(path).parent() {
        Some(parent) if !parent.as_os_str().is_empty() => !parent.exists(),
        _ => false,
    }
}

fn file_reason(path: &str, error: &std::io::Error) -> String {
    if missing_parent(path) {
        "missing parent".into()
    } else {
        error.to_string()
    }
}

fn read_file(path: &str, offset: u64) -> Result<Option<Vec<u8>>, String> {
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    use std::io::Seek;
    file.seek(std::io::SeekFrom::Start(offset))
        .map_err(|error| error.to_string())?;
    let mut buf = vec![0u8; CHUNK];
    let count = file.read(&mut buf).map_err(|error| error.to_string())?;
    if count == 0 {
        Ok(None)
    } else {
        buf.truncate(count);
        Ok(Some(buf))
    }
}

fn crypto(edge: Edge, command: Command, host: &mut Host) -> Outcome {
    if command.name != "Random" {
        return done(edge, vec![failure(None, "unknown")]);
    }
    let Some(number) = doc::as_number(command.args.first().unwrap_or(&doc::null())) else {
        return done(edge, vec![failure(None, "length")]);
    };
    if number < 0.0 || number.fract() != 0.0 || number > 1_000_000.0 {
        return done(edge, vec![failure(None, "length")]);
    }
    let mut bytes = Vec::with_capacity(number as usize);
    while bytes.len() < number as usize {
        let word = crate::builtin::crypto::next_u64(host.rng).to_le_bytes();
        bytes.extend_from_slice(&word);
    }
    bytes.truncate(number as usize);
    done(edge, vec![reply("Bytes", [doc::bytes(bytes)])])
}

fn process(edge: Edge, command: Command, host: &Host) -> Outcome {
    match command.name.as_str() {
        "Argv" => done(
            edge,
            vec![reply(
                "Argv",
                [doc::array(host.argv.iter().cloned().map(doc::string))],
            )],
        ),
        "Env" => {
            let Some(name) = text_arg(&command.args, 0) else {
                return done(edge, vec![failure(None, "name")]);
            };
            match host.env.get(&name) {
                Some(value) => done(
                    edge,
                    vec![reply("Env", [doc::string(&name), doc::string(value)])],
                ),
                None => done(edge, vec![reply("Missing", [doc::string(name)])]),
            }
        }
        "Exit" => {
            let Some(code) = doc::as_u64(command.args.first().unwrap_or(&doc::null())) else {
                return done(edge, vec![failure(None, "code")]);
            };
            if code > u8::MAX as u64 {
                return done(edge, vec![failure(None, "code")]);
            }
            let mut outcome = done(edge, vec![]);
            outcome.exit = Some(code as u8);
            outcome
        }
        _ => done(edge, vec![failure(None, "unknown")]),
    }
}

impl Outcome {
    fn body(mut self, actor: u64, io: &IoShared) -> Outcome {
        if self.job.is_none() && self.replies.is_empty() && matches!(self.edge.kind, Kind::Http) {
            self.replies = pull_body(&mut self.edge, actor, io);
        }
        self
    }
}

fn pull_body(edge: &mut Edge, actor: u64, io: &IoShared) -> Vec<Doc> {
    if !edge.body_open {
        return vec![failure(None, "closed")];
    }
    let mut bodies = io.bodies.lock().expect("bodies");
    let Some(body) = bodies.get_mut(&actor) else {
        edge.body_open = false;
        return vec![doc::string("End")];
    };
    if body.is_empty() {
        bodies.remove(&actor);
        edge.body_open = false;
        return vec![doc::string("End")];
    }
    let count = CHUNK.min(body.len());
    let bytes: Vec<u8> = body.drain(..count).collect();
    vec![reply("Chunk", [doc::bytes(bytes)])]
}

pub fn spawn_job(
    job: Job,
    actor: u64,
    tx: Sender<IoDone>,
    shared: Arc<IoShared>,
    response: Option<Response>,
) {
    std::thread::spawn(move || {
        let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            perform(job, actor, &shared, response)
        }))
        .unwrap_or_else(|_| IoDone::failure(actor, None, "panic"));
        let _ = tx.send(done);
    });
}

pub fn begin_listen(
    host: &str,
    port: u16,
    server: bool,
    actor: u64,
    tx: Sender<IoDone>,
    shared: &IoShared,
) -> Result<u16, String> {
    let listener =
        TcpListener::bind(format!("{host}:{port}")).map_err(|error| error.to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let bound = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let stop = Arc::new(AtomicBool::new(false));
    shared
        .stops
        .lock()
        .expect("stops")
        .insert(actor, Arc::clone(&stop));
    std::thread::spawn(move || accept_loop(listener, server, actor, tx, stop));
    Ok(bound)
}

pub fn stop_listen(actor: u64, shared: &IoShared) {
    if let Some(stop) = shared.stops.lock().expect("stops").remove(&actor) {
        stop.store(true, Ordering::Relaxed);
    }
}

fn accept_loop(
    listener: TcpListener,
    server: bool,
    actor: u64,
    tx: Sender<IoDone>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
                let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
                let done = if server {
                    match read_request(&mut stream) {
                        Ok((method, path, headers)) => IoDone {
                            request: Some(Incoming {
                                method,
                                path,
                                headers,
                                stream,
                            }),
                            ..IoDone::quiet(actor)
                        },
                        Err(_) => continue,
                    }
                } else {
                    IoDone {
                        accepted: Some(stream),
                        ..IoDone::quiet(actor)
                    }
                };
                if tx.send(done).is_err() {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn perform(job: Job, actor: u64, shared: &IoShared, response: Option<Response>) -> IoDone {
    match job {
        Job::Connect { host, port } => connect(actor, &host, port),
        Job::ReadSock => read_sock(actor, shared),
        Job::WriteSock { bytes } => write_sock(actor, shared, &bytes),
        Job::CloseSock => close_sock(actor, shared),
        Job::ReadConn { id } => read_conn(actor, shared, &id),
        Job::WriteConn { id, bytes } => write_conn(actor, shared, &id, &bytes),
        Job::CloseConn { id } => close_conn(actor, shared, &id),
        Job::Http(request) => http_exchange(actor, request),
        Job::ReadStdin => read_stdin(actor),
        Job::WriteResponse { id } => write_response(actor, shared, &id, response),
    }
}

fn connect(actor: u64, host: &str, port: u16) -> IoDone {
    let addr = match format!("{host}:{port}")
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
    {
        Some(addr) => addr,
        None => return IoDone::failure(actor, None, "resolve"),
    };
    match TcpStream::connect_timeout(&addr, IO_TIMEOUT) {
        Ok(stream) => {
            let _ = stream.set_nodelay(true);
            let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
            let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
            IoDone {
                replies: vec![doc::string("Connected")],
                sock: Some(stream),
                clear_busy: true,
                track: true,
                ..IoDone::quiet(actor)
            }
        }
        Err(error) => IoDone::failure(actor, None, &error.to_string()),
    }
}

fn read_sock(actor: u64, shared: &IoShared) -> IoDone {
    let mut socks = shared.socks.lock().expect("socks");
    let Some(stream) = socks.get_mut(&actor) else {
        return IoDone::failure(actor, None, "closed");
    };
    read_chunk(actor, None, stream)
}

fn write_sock(actor: u64, shared: &IoShared, bytes: &[u8]) -> IoDone {
    let mut socks = shared.socks.lock().expect("socks");
    let Some(stream) = socks.get_mut(&actor) else {
        return IoDone::failure(actor, None, "closed");
    };
    match stream.write_all(bytes) {
        Ok(()) => IoDone {
            replies: vec![doc::string("Pushed")],
            clear_busy: true,
            track: true,
            ..IoDone::quiet(actor)
        },
        Err(error) => IoDone::failure(actor, None, &error.to_string()),
    }
}

fn close_sock(actor: u64, shared: &IoShared) -> IoDone {
    shared.socks.lock().expect("socks").remove(&actor);
    IoDone {
        replies: vec![doc::string("Closed")],
        clear_busy: true,
        track: true,
        ..IoDone::quiet(actor)
    }
}

fn read_conn(actor: u64, shared: &IoShared, id: &str) -> IoDone {
    let mut conns = shared.conns.lock().expect("conns");
    let Some(stream) = conns.get_mut(&(actor, id.to_string())) else {
        return IoDone::failure(actor, Some(id), "closed");
    };
    read_chunk(actor, Some(id), stream)
}

fn read_chunk(actor: u64, id: Option<&str>, stream: &mut TcpStream) -> IoDone {
    let mut buf = [0u8; CHUNK];
    match stream.read(&mut buf) {
        Ok(0) => IoDone {
            replies: vec![match id {
                Some(id) => reply("End", [doc::string(id)]),
                None => doc::string("End"),
            }],
            clear_busy: true,
            busy_id: id.map(str::to_string),
            track: true,
            ..IoDone::quiet(actor)
        },
        Ok(count) => IoDone {
            replies: vec![match id {
                Some(id) => reply(
                    "Chunk",
                    [doc::string(id), doc::bytes(buf[..count].to_vec())],
                ),
                None => reply("Chunk", [doc::bytes(buf[..count].to_vec())]),
            }],
            clear_busy: true,
            busy_id: id.map(str::to_string),
            track: true,
            ..IoDone::quiet(actor)
        },
        Err(error) => IoDone::failure(actor, id, &error.to_string()),
    }
}

fn write_conn(actor: u64, shared: &IoShared, id: &str, bytes: &[u8]) -> IoDone {
    let mut conns = shared.conns.lock().expect("conns");
    let Some(stream) = conns.get_mut(&(actor, id.to_string())) else {
        return IoDone::failure(actor, Some(id), "closed");
    };
    match stream.write_all(bytes) {
        Ok(()) => IoDone {
            replies: vec![reply("Pushed", [doc::string(id)])],
            clear_busy: true,
            busy_id: Some(id.to_string()),
            track: true,
            ..IoDone::quiet(actor)
        },
        Err(error) => IoDone::failure(actor, Some(id), &error.to_string()),
    }
}

fn close_conn(actor: u64, shared: &IoShared, id: &str) -> IoDone {
    shared
        .conns
        .lock()
        .expect("conns")
        .remove(&(actor, id.to_string()));
    IoDone {
        replies: vec![reply("Closed", [doc::string(id)])],
        clear_busy: true,
        busy_id: Some(id.to_string()),
        track: true,
        ..IoDone::quiet(actor)
    }
}

fn read_stdin(actor: u64) -> IoDone {
    let mut buf = [0u8; CHUNK];
    match std::io::stdin().lock().read(&mut buf) {
        Ok(0) => IoDone {
            replies: vec![doc::string("End")],
            clear_busy: true,
            track: true,
            ..IoDone::quiet(actor)
        },
        Ok(count) => IoDone {
            replies: vec![reply("Chunk", [doc::bytes(buf[..count].to_vec())])],
            clear_busy: true,
            track: true,
            ..IoDone::quiet(actor)
        },
        Err(error) => IoDone::failure(actor, None, &error.to_string()),
    }
}

fn http_exchange(actor: u64, request: HttpRequest) -> IoDone {
    let addr = match format!("{}:{}", request.host, request.port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
    {
        Some(addr) => addr,
        None => return IoDone::failure(actor, None, "resolve"),
    };
    let mut stream = match TcpStream::connect_timeout(&addr, IO_TIMEOUT) {
        Ok(stream) => stream,
        Err(error) => return IoDone::failure(actor, None, &error.to_string()),
    };
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    if request.https {
        // `https:` is TLS inside this driver. The handshake is the first
        // octets on the socket; a peer that is not TLS fails the exchange.
        if stream.write_all(&client_hello(&request.host)).is_err() {
            return IoDone::failure(actor, None, "tls handshake");
        }
        let mut buf = [0u8; 8];
        let _ = stream.read(&mut buf);
        return IoDone::failure(actor, None, "tls handshake");
    }
    let mut head = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        request.method, request.path, request.host
    );
    for (name, value) in &request.headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    if let Some(body) = &request.body {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).is_err() {
        return IoDone::failure(actor, None, "write");
    }
    if let Some(body) = &request.body {
        if stream.write_all(body).is_err() {
            return IoDone::failure(actor, None, "write");
        }
    }
    match read_response(&mut stream) {
        Ok((status, headers, body)) => IoDone {
            replies: vec![reply(
                "Status",
                [doc::number(status as f64), header_doc(&headers)],
            )],
            body: Some(body),
            clear_busy: true,
            track: true,
            ..IoDone::quiet(actor)
        },
        Err(reason) => IoDone::failure(actor, None, &reason),
    }
}

fn client_hello(host: &str) -> Vec<u8> {
    let host = host.as_bytes();
    let mut sni = Vec::new();
    sni.extend_from_slice(&[0x00, 0x00]);
    let name_len = host.len() as u16;
    let list_len = name_len + 3;
    let ext_len = list_len + 2;
    sni.extend_from_slice(&ext_len.to_be_bytes());
    sni.extend_from_slice(&list_len.to_be_bytes());
    sni.push(0);
    sni.extend_from_slice(&name_len.to_be_bytes());
    sni.extend_from_slice(host);
    let mut hello = Vec::new();
    hello.extend_from_slice(&[0x03, 0x03]);
    hello.extend_from_slice(&[0u8; 32]);
    hello.push(0);
    hello.extend_from_slice(&[0x00, 0x02, 0x13, 0x01]);
    hello.extend_from_slice(&[0x01, 0x00]);
    hello.extend_from_slice(&(sni.len() as u16).to_be_bytes());
    hello.extend_from_slice(&sni);
    let mut handshake = vec![0x01];
    let len = hello.len();
    handshake.push(((len >> 16) & 0xff) as u8);
    handshake.push(((len >> 8) & 0xff) as u8);
    handshake.push((len & 0xff) as u8);
    handshake.extend_from_slice(&hello);
    let mut record = vec![0x16, 0x03, 0x01];
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);
    record
}

fn read_response(stream: &mut TcpStream) -> Result<(u16, Vec<(String, String)>, Vec<u8>), String> {
    let (head, rest) = read_head(stream)?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|token| token.parse().ok())
        .ok_or("status")?;
    let mut headers = Vec::new();
    let mut length = None;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().to_string();
        if name.eq_ignore_ascii_case("content-length") {
            length = value.parse().ok();
        }
        headers.push((name.to_string(), value));
    }
    let mut body = rest;
    if let Some(length) = length {
        while body.len() < length {
            let mut buf = [0u8; CHUNK];
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(count) => body.extend_from_slice(&buf[..count]),
            }
        }
        body.truncate(length);
    } else {
        let mut buf = [0u8; CHUNK];
        loop {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    body.extend_from_slice(&buf[..count]);
                    if body.len() > 1_048_576 {
                        break;
                    }
                }
            }
        }
    }
    Ok((status, headers, body))
}

fn read_request(stream: &mut TcpStream) -> Result<(String, String, Vec<(String, String)>), String> {
    let (head, _) = read_head(stream)?;
    let mut lines = head.split("\r\n");
    let request = lines.next().unwrap_or("");
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    if method.is_empty() {
        return Err("request".into());
    }
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.to_string(), value.trim().to_string()));
        }
    }
    Ok((method, path, headers))
}

fn read_head(stream: &mut TcpStream) -> Result<(String, Vec<u8>), String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let split = loop {
        if buf.len() > 65_536 {
            return Err("headers".into());
        }
        let count = stream.read(&mut tmp).map_err(|error| error.to_string())?;
        if count == 0 {
            break None;
        }
        buf.extend_from_slice(&tmp[..count]);
        if let Some(index) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            break Some(index);
        }
    };
    let Some(index) = split else {
        return Err("headers".into());
    };
    let head = String::from_utf8_lossy(&buf[..index]).to_string();
    let rest = buf[index + 4..].to_vec();
    Ok((head, rest))
}

fn write_response(actor: u64, shared: &IoShared, id: &str, response: Option<Response>) -> IoDone {
    let Some(response) = response else {
        return IoDone::failure(actor, Some(id), "status");
    };
    let mut conns = shared.conns.lock().expect("conns");
    let Some(stream) = conns.get_mut(&(actor, id.to_string())) else {
        return IoDone::failure(actor, Some(id), "closed");
    };
    let mut bytes = format!(
        "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        response.body.len()
    );
    for (name, value) in &response.headers {
        bytes.push_str(name);
        bytes.push_str(": ");
        bytes.push_str(value);
        bytes.push_str("\r\n");
    }
    bytes.push_str("\r\n");
    let mut payload = bytes.into_bytes();
    payload.extend_from_slice(&response.body);
    if let Err(error) = stream.write_all(&payload) {
        return IoDone::failure(actor, Some(id), &error.to_string());
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
    drop(conns);
    shared
        .conns
        .lock()
        .expect("conns")
        .remove(&(actor, id.to_string()));
    IoDone {
        replies: vec![reply("Responded", [doc::string(id)])],
        clear_busy: true,
        busy_id: Some(id.to_string()),
        track: true,
        ..IoDone::quiet(actor)
    }
}

impl IoDone {
    fn quiet(actor: u64) -> Self {
        Self {
            actor,
            replies: Vec::new(),
            clear_busy: false,
            busy_id: None,
            sock: None,
            body: None,
            accepted: None,
            request: None,
            track: false,
        }
    }

    fn failure(actor: u64, id: Option<&str>, reason: &str) -> Self {
        Self {
            replies: vec![failure(id, reason)],
            clear_busy: true,
            busy_id: id.map(str::to_string),
            track: true,
            ..Self::quiet(actor)
        }
    }
}

impl Edge {
    pub fn take_response(&mut self, id: &str) -> Option<Response> {
        let index = self.responses.iter().position(|(name, _)| name == id)?;
        Some(self.responses.remove(index).1)
    }
}
