use kith::doc::{self, Doc};
use kith::parse::resolve_program;
use kith::World;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

fn scratch(files: &[(&str, &str)]) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "kith-edges-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, source) in files {
        std::fs::write(dir.join(name), source).unwrap();
    }
    dir
}

fn load(source: &str) -> kith::ast::Program {
    let dir = scratch(&[("main.kith", source)]);
    resolve_program(&dir.join("main.kith")).expect("resolve")
}

fn start(source: &str) -> World {
    World::start(load(source)).expect("start")
}

fn child(world: &World, kind: &str) -> kith::ActorId {
    world
        .children_of_kind(world.root_id(), kind)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("missing {kind}"))
}

fn send(world: &mut World, kind: &str, message: Doc) {
    let id = child(world, kind);
    world.send_document(id, message).unwrap();
    world.run_until_idle().unwrap();
}

fn got(world: &World) -> Doc {
    world
        .field_document(world.root_id(), "got")
        .unwrap_or_else(doc::null)
}

fn settle(world: &mut World) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        world.run_until_idle().unwrap();
        world.poll_io().unwrap();
        if world.outstanding() == 0 {
            world.run_until_idle().unwrap();
            if world.outstanding() == 0 {
                return;
            }
        }
        world.wait_io(Duration::from_millis(50)).unwrap();
    }
    panic!("io did not settle");
}

fn until_got(world: &mut World, expected: &Doc) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        world.run_until_idle().unwrap();
        world.poll_io().unwrap();
        if &got(world) == expected {
            return;
        }
        world.wait_io(Duration::from_millis(50)).unwrap();
    }
    panic!("timed out waiting, last message did not match");
}

fn down(name: &str, args: impl IntoIterator<Item = Doc>) -> Doc {
    let mut items = vec![doc::string(name)];
    items.extend(args);
    doc::array(items)
}

const NET: &str = r#"
import "kith:net" Socket Listener;
export Probe;
export Main;
component Probe {
  field got: any;
  socket: Socket;
  listener: Listener;
  start { on up message:any { got -> message; } }
}
export scene Main {
  Probe() { socket = Socket(); listener = Listener(); }
}
"#;

#[test]
// req:edges-001
fn net_streams_tcp_bytes_and_keeps_a_scripted_pull_in_flight() {
    let program = load(NET);
    let mut world = World::start(program.clone()).unwrap();
    let socket = child(&world, "Socket");
    world.script_driver(socket, vec![vec![]]);
    let started = Instant::now();
    send(&mut world, "Socket", doc::string("Pull"));
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(doc::is_null(&got(&world)));
    send(&mut world, "Socket", doc::string("Pull"));
    assert_eq!(got(&world), down("Error", [doc::string("busy")]));
    send(&mut world, "Socket", doc::string("Nope"));
    assert_eq!(got(&world), down("Error", [doc::string("unknown")]));
    let snap = world.snapshot().unwrap();
    let mut restored = World::restore(program, &snap).unwrap();
    send(&mut restored, "Socket", doc::string("Pull"));
    assert_eq!(got(&restored), down("Error", [doc::string("busy")]));
    let socket = child(&restored, "Socket");
    restored
        .answer(socket, vec![down("Chunk", [doc::bytes(b"ab".to_vec())])])
        .unwrap();
    restored.run_until_idle().unwrap();
    assert_eq!(got(&restored), down("Chunk", [doc::bytes(b"ab".to_vec())]));

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4];
        sock.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ping");
        sock.write_all(b"pong").unwrap();
        let _ = sock.shutdown(std::net::Shutdown::Write);
    });
    let mut world = start(NET);
    send(
        &mut world,
        "Socket",
        down(
            "Connect",
            [doc::string("127.0.0.1"), doc::number(port as f64)],
        ),
    );
    settle(&mut world);
    assert_eq!(got(&world), doc::string("Connected"));
    send(
        &mut world,
        "Socket",
        down("Push", [doc::bytes(b"ping".to_vec())]),
    );
    settle(&mut world);
    assert_eq!(got(&world), doc::string("Pushed"));
    send(&mut world, "Socket", doc::string("Pull"));
    settle(&mut world);
    assert_eq!(got(&world), down("Chunk", [doc::bytes(b"pong".to_vec())]));
    send(&mut world, "Socket", doc::string("Pull"));
    settle(&mut world);
    assert_eq!(got(&world), doc::string("End"));
    send(&mut world, "Socket", doc::string("Close"));
    settle(&mut world);
    assert_eq!(got(&world), doc::string("Closed"));

    let mut world = start(NET);
    send(
        &mut world,
        "Listener",
        down("Listen", [doc::string("127.0.0.1"), doc::number(0.0)]),
    );
    let listening = got(&world);
    let items = doc::items(&listening).unwrap();
    assert_eq!(doc::as_str(&items[0]).as_deref(), Some("Listening"));
    let port = doc::as_u64(&items[1]).unwrap();
    let (release, hold) = std::sync::mpsc::channel();
    let client = std::thread::spawn(move || {
        let sock = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        let _ = hold.recv();
        drop(sock);
    });
    until_got(&mut world, &down("Connection", [doc::string("c1")]));
    send(&mut world, "Listener", down("Pull", [doc::string("c1")]));
    world.run_until_idle().unwrap();
    send(&mut world, "Listener", down("Pull", [doc::string("c1")]));
    assert_eq!(
        got(&world),
        down("Error", [doc::string("c1"), doc::string("busy")])
    );
    let _ = release.send(());
    client.join().unwrap();
    send(&mut world, "Listener", doc::string("Stop"));
    assert_eq!(got(&world), doc::string("Stopped"));
}

const HTTP: &str = r#"
import "kith:http" Http Server;
export Probe;
export Main;
component Probe {
  field got: any;
  http: Http;
  server: Server;
  start { on up message:any { got -> message; } }
}
export scene Main {
  Probe() { http = Http(); server = Server(); }
}
"#;

fn serve(response: &'static [u8]) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = sock.read(&mut buf);
        sock.write_all(response).unwrap();
    });
    port
}

#[test]
// req:edges-002
fn http_speaks_http11_without_redirects_and_uses_tls_for_https() {
    let source = std::fs::read_to_string("examples/http.kith").unwrap();
    assert!(source.contains("component Http"));
    assert!(!source.contains("kith:http"));

    let port = serve(
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\nX-Test: yes\r\n\r\nok",
    );
    let mut world = start(HTTP);
    send(
        &mut world,
        "Http",
        down(
            "GET",
            [
                doc::string(format!("http://127.0.0.1:{port}/hi")),
                doc::object([("X-Test", doc::string("yes"))]),
            ],
        ),
    );
    settle(&mut world);
    let status = doc::items(&got(&world)).unwrap();
    assert_eq!(doc::as_str(&status[0]).as_deref(), Some("Status"));
    assert_eq!(doc::as_u64(&status[1]), Some(200));
    send(&mut world, "Http", doc::string("Pull"));
    assert_eq!(got(&world), down("Chunk", [doc::bytes(b"ok".to_vec())]));
    send(&mut world, "Http", doc::string("Pull"));
    assert_eq!(got(&world), doc::string("End"));

    let port = serve(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/gone\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let mut world = start(HTTP);
    let started = Instant::now();
    send(
        &mut world,
        "Http",
        down("GET", [doc::string(format!("http://127.0.0.1:{port}/"))]),
    );
    settle(&mut world);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(
        doc::as_u64(&doc::items(&got(&world)).unwrap()[1]),
        Some(302)
    );

    let (tx, rx) = std::sync::mpsc::channel();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        loop {
            match sock.read(&mut tmp) {
                Ok(0) => break,
                Ok(count) => {
                    buf.extend_from_slice(&tmp[..count]);
                    if buf.windows(4).any(|window| window == b"\r\n\r\n") && buf.ends_with(b"hi") {
                        break;
                    }
                    if buf.windows(2).any(|window| window == b"hi") {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        tx.send(buf).unwrap();
        sock.write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let mut world = start(HTTP);
    send(
        &mut world,
        "Http",
        down(
            "POST",
            [
                doc::string(format!("http://127.0.0.1:{port}/up")),
                doc::null(),
            ],
        ),
    );
    assert_eq!(got(&world), doc::string("Ready"));
    send(
        &mut world,
        "Http",
        down("Push", [doc::bytes(b"hi".to_vec())]),
    );
    assert_eq!(got(&world), doc::string("Pushed"));
    send(&mut world, "Http", doc::string("BodyEnd"));
    settle(&mut world);
    assert_eq!(
        doc::as_u64(&doc::items(&got(&world)).unwrap()[1]),
        Some(201)
    );
    let request =
        String::from_utf8_lossy(&rx.recv_timeout(Duration::from_secs(2)).unwrap()).to_string();
    assert!(request.starts_with("POST /up HTTP/1.1"), "{request}");
    assert!(request.contains("hi"), "{request}");

    send(
        &mut world,
        "Http",
        down("PUT", [doc::string("http://127.0.0.1/x")]),
    );
    assert_eq!(got(&world), down("Error", [doc::string("unknown")]));

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (seen, saw) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = [0u8; 5];
        sock.read_exact(&mut buf).unwrap();
        seen.send(buf[0]).unwrap();
    });
    let mut world = start(HTTP);
    send(
        &mut world,
        "Http",
        down(
            "GET",
            [doc::string(format!("https://127.0.0.1:{port}/secure"))],
        ),
    );
    settle(&mut world);
    let reason = doc::as_str(&doc::items(&got(&world)).unwrap()[1]).unwrap();
    assert!(reason.contains("tls"), "{reason}");
    assert_eq!(saw.recv_timeout(Duration::from_secs(2)).unwrap(), 0x16);

    let mut world = start(HTTP);
    send(&mut world, "Server", down("Listen", [doc::number(0.0)]));
    let port = doc::as_u64(&doc::items(&got(&world)).unwrap()[1]).unwrap();
    let client = std::thread::spawn(move || {
        let mut sock = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        sock.write_all(b"GET /hi?x=1 HTTP/1.1\r\nHost: h\r\nConnection: close\r\n\r\n")
            .unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut buf = Vec::new();
        let _ = sock.read_to_end(&mut buf);
        buf
    });
    until_got(
        &mut world,
        &down(
            "Request",
            [
                doc::string("r1"),
                doc::string("GET"),
                doc::string("/hi?x=1"),
                doc::object([
                    ("Host", doc::string("h")),
                    ("Connection", doc::string("close")),
                ]),
            ],
        ),
    );
    send(&mut world, "Server", down("Pull", [doc::string("r1")]));
    assert_eq!(got(&world), down("End", [doc::string("r1")]));
    send(
        &mut world,
        "Server",
        down(
            "Status",
            [doc::string("r1"), doc::number(200.0), doc::null()],
        ),
    );
    send(
        &mut world,
        "Server",
        down("Push", [doc::string("r1"), doc::bytes(b"ok".to_vec())]),
    );
    assert_eq!(got(&world), down("Pushed", [doc::string("r1")]));
    send(&mut world, "Server", down("BodyEnd", [doc::string("r1")]));
    settle(&mut world);
    assert_eq!(got(&world), down("Responded", [doc::string("r1")]));
    let response = String::from_utf8_lossy(&client.join().unwrap()).to_string();
    assert!(response.contains("200"), "{response}");
    assert!(response.contains("ok"), "{response}");
    send(&mut world, "Server", doc::string("Stop"));
}

#[test]
// req:edges-002
fn server_listens_on_a_host_and_pushes_text() {
    let mut world = start(HTTP);
    send(
        &mut world,
        "Server",
        down("Listen", [doc::string("127.0.0.1"), doc::number(0.0)]),
    );
    let listening = doc::items(&got(&world)).unwrap();
    assert_eq!(doc::as_str(&listening[0]).as_deref(), Some("Listening"));
    let port = doc::as_u64(&listening[1]).unwrap();
    let client = std::thread::spawn(move || {
        let mut sock = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        sock.write_all(b"GET / HTTP/1.1\r\nHost: h\r\nConnection: close\r\n\r\n")
            .unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut buf = Vec::new();
        let _ = sock.read_to_end(&mut buf);
        buf
    });
    until_got(
        &mut world,
        &down(
            "Request",
            [
                doc::string("r1"),
                doc::string("GET"),
                doc::string("/"),
                doc::object([
                    ("Host", doc::string("h")),
                    ("Connection", doc::string("close")),
                ]),
            ],
        ),
    );
    send(
        &mut world,
        "Server",
        down(
            "Status",
            [
                doc::string("r1"),
                doc::number(200.0),
                doc::object([("Content-Type", doc::string("text/plain"))]),
            ],
        ),
    );
    send(
        &mut world,
        "Server",
        down("Push", [doc::string("r1"), doc::string("Hello, World!\n")]),
    );
    assert_eq!(got(&world), down("Pushed", [doc::string("r1")]));
    send(&mut world, "Server", down("BodyEnd", [doc::string("r1")]));
    settle(&mut world);
    let response = String::from_utf8_lossy(&client.join().unwrap()).to_string();
    assert!(response.contains("HTTP/1.1 200"), "{response}");
    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-type: text/plain"),
        "{response}"
    );
    assert!(response.contains("Hello, World!\n"), "{response}");
    send(&mut world, "Server", doc::string("Stop"));
}

#[test]
// req:edges-009
fn hello_world_serves_plain_text_on_port_3000() {
    let source = std::fs::read_to_string("examples/hello-world.kith").unwrap();
    assert!(source.contains("import \"kith:http\" Server"));
    assert!(source.contains("\"Listen\" \"127.0.0.1\" 3000"));
    assert!(source.contains("Server running at http://127.0.0.1:3000/"));
    assert!(source.contains("Hello, World!\\n"));
    let mock = std::fs::read_to_string("examples/http.kith").unwrap();
    assert!(mock.contains("component Http"));
    assert!(!mock.contains("kith:http"));

    let mut child = Command::new(env!("CARGO_BIN_EXE_kith"))
        .arg("run")
        .arg("examples/hello-world.kith")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 512];
        loop {
            match stderr.read(&mut tmp) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    buf.extend_from_slice(&tmp[..count]);
                    if buf
                        .windows(b"Server running at http://127.0.0.1:3000/".len())
                        .any(|window| window == b"Server running at http://127.0.0.1:3000/")
                    {
                        let _ = tx.send(buf);
                        break;
                    }
                }
            }
        }
    });
    let log = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("hello-world did not log that it is listening");
    let log = String::from_utf8_lossy(&log);
    assert!(
        log.contains("Server running at http://127.0.0.1:3000/"),
        "{log}"
    );

    let mut sock = TcpStream::connect("127.0.0.1:3000").unwrap();
    sock.write_all(
        b"POST /hi HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    )
    .unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut buf = Vec::new();
    let _ = sock.read_to_end(&mut buf);
    let _ = child.kill();
    let _ = child.wait();
    let response = String::from_utf8_lossy(&buf).to_string();
    assert!(response.contains("HTTP/1.1 200"), "{response}");
    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-type: text/plain"),
        "{response}"
    );
    assert!(response.ends_with("Hello, World!\n"), "{response}");
}

const STDIO: &str = r#"
import "kith:stdio" Stdin Stdout Stderr;
export Probe;
export Main;
component Probe {
  field got: any;
  input: Stdin;
  out: Stdout;
  err: Stderr;
  start { on up message:any { got -> message; } }
}
export scene Main {
  Probe() { input = Stdin(); out = Stdout(); err = Stderr(); }
}
"#;

#[test]
// req:edges-003
fn stdio_is_the_process_byte_streams() {
    let mut world = start(STDIO);
    let input = child(&world, "Stdin");
    world.script_driver(input, vec![vec![]]);
    send(&mut world, "Stdin", doc::string("Pull"));
    assert!(doc::is_null(&got(&world)));
    send(&mut world, "Stdin", doc::string("Pull"));
    assert_eq!(got(&world), down("Error", [doc::string("busy")]));

    let dir = scratch(&[(
        "main.kith",
        r#"
        import "kith:stdio" Stdin;
        import "kith:encoding" utf8_decode;
        export Probe;
        export Main;
        component Probe {
          field text: any;
          input: Stdin;
          start {
            on init { send to input "Pull"; }
            on up "Chunk" value:any { text -> (utf8_decode value); }
          }
        }
        export scene Main { Probe() { input = Stdin(); } }
        "#,
    )]);
    let mut child_proc = Command::new(env!("CARGO_BIN_EXE_kith"))
        .arg("run")
        .arg(dir.join("main.kith"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child_proc.stdin.take().unwrap().write_all(b"hi").unwrap();
    let output = child_proc.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hi"), "{stdout}");

    let dir = scratch(&[(
        "main.kith",
        r#"
        import "kith:stdio" Stdout Stderr;
        import "kith:encoding" utf8_encode;
        export Probe;
        export Main;
        component Probe {
          out: Stdout;
          err: Stderr;
          start {
            on init {
              (print "diag");
              send to out "Push" (utf8_encode "OUT");
              send to err "Push" (utf8_encode "ERR");
            }
          }
        }
        export scene Main { Probe() { out = Stdout(); err = Stderr(); } }
        "#,
    )]);
    let ran = Command::new(env!("CARGO_BIN_EXE_kith"))
        .args(["run", dir.join("main.kith").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert_eq!(ran.stdout, b"OUT");
    let stderr = String::from_utf8_lossy(&ran.stderr);
    assert!(stderr.contains("diag"), "{stderr}");
    assert!(stderr.contains("ERR"), "{stderr}");
}

const FS: &str = r#"
import "kith:fs" File;
export Probe;
export Main;
component Probe {
  field got: any;
  file: File;
  start { on up message:any { got -> message; } }
}
export scene Main { Probe() { file = File(); } }
"#;

#[test]
// req:edges-004
fn files_stream_bytes_and_sorted_names() {
    let dir = scratch(&[]);
    let path = dir.join("note");
    let text = path.to_str().unwrap();
    let mut world = start(FS);
    send(
        &mut world,
        "File",
        down("Open", [doc::string(text), doc::string("write")]),
    );
    assert_eq!(got(&world), doc::string("Opened"));
    let mut bytes = vec![b'a'; 5000];
    bytes.extend(b"Z");
    send(&mut world, "File", down("Push", [doc::bytes(bytes)]));
    assert_eq!(got(&world), doc::string("Pushed"));
    send(&mut world, "File", doc::string("Close"));
    assert_eq!(got(&world), doc::string("Closed"));
    send(
        &mut world,
        "File",
        down("Open", [doc::string(text), doc::string("read")]),
    );
    send(&mut world, "File", doc::string("Pull"));
    let chunk = doc::items(&got(&world)).unwrap();
    assert_eq!(doc::as_bytes(&chunk[1]).unwrap().len(), 4096);
    send(&mut world, "File", doc::string("Pull"));
    let rest = doc::as_bytes(&doc::items(&got(&world)).unwrap()[1]).unwrap();
    assert_eq!(rest.len(), 905);
    assert_eq!(*rest.last().unwrap(), b'Z');
    send(&mut world, "File", doc::string("Pull"));
    assert_eq!(got(&world), doc::string("End"));

    send(
        &mut world,
        "File",
        down("Open", [doc::string(text), doc::string("write")]),
    );
    send(
        &mut world,
        "File",
        down("Push", [doc::bytes(b"bye".to_vec())]),
    );
    send(&mut world, "File", doc::string("Close"));
    send(
        &mut world,
        "File",
        down("Open", [doc::string(text), doc::string("read")]),
    );
    send(&mut world, "File", doc::string("Pull"));
    assert_eq!(
        doc::as_bytes(&doc::items(&got(&world)).unwrap()[1]).unwrap(),
        b"bye"
    );

    let missing = dir.join("nope").join("file");
    send(
        &mut world,
        "File",
        down(
            "Open",
            [doc::string(missing.to_str().unwrap()), doc::string("write")],
        ),
    );
    assert_eq!(got(&world), down("Error", [doc::string("missing parent")]));

    std::fs::write(dir.join("b"), "b").unwrap();
    std::fs::write(dir.join("a"), "a").unwrap();
    send(
        &mut world,
        "File",
        down("List", [doc::string(dir.to_str().unwrap())]),
    );
    assert_eq!(got(&world), doc::string("Opened"));
    send(&mut world, "File", doc::string("Pull"));
    assert_eq!(got(&world), down("Chunk", [doc::string("a")]));
    send(&mut world, "File", doc::string("Pull"));
    assert_eq!(got(&world), down("Chunk", [doc::string("b")]));
    send(&mut world, "File", doc::string("Pull"));
    assert_eq!(got(&world), down("Chunk", [doc::string("note")]));
    send(&mut world, "File", doc::string("Pull"));
    assert_eq!(got(&world), doc::string("End"));
    send(&mut world, "File", down("Remove", [doc::string(text)]));
    assert_eq!(got(&world), doc::string("Removed"));
    assert!(!path.exists());

    let file = child(&world, "File");
    world.script_driver(file, vec![vec![]]);
    send(&mut world, "File", doc::string("Pull"));
    send(&mut world, "File", doc::string("Pull"));
    assert_eq!(got(&world), down("Error", [doc::string("busy")]));
}

#[test]
// req:edges-005
fn encoding_converts_text_and_bytes() {
    let mut world = start(
        r#"
        import "kith:encoding" utf8_encode utf8_decode base64_encode base64_decode hex_encode hex_decode;
        export Probe;
        export Main;
        component Probe {
          field got: any;
          start {
            on down "B64" { got -> (base64_encode (utf8_encode "hi")); }
            on down "Hex" { got -> (hex_encode (utf8_encode "hi")); }
            on down "Back" { got -> (utf8_decode (base64_decode "aGk=")); }
            on down "Bad" { got -> (base64_decode "aGk"); }
          }
        }
        export scene Main { Probe() {} }
        "#,
    );
    world
        .send(world.root_id(), serde_json::json!("B64"))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(doc::as_str(&got(&world)).as_deref(), Some("aGk="));
    world
        .send(world.root_id(), serde_json::json!("Hex"))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(doc::as_str(&got(&world)).as_deref(), Some("6869"));
    world
        .send(world.root_id(), serde_json::json!("Back"))
        .unwrap();
    world.run_until_idle().unwrap();
    assert_eq!(doc::as_str(&got(&world)).as_deref(), Some("hi"));
    world
        .send(world.root_id(), serde_json::json!("Bad"))
        .unwrap();
    let error = world.run_until_idle().unwrap_err().message;
    assert!(error.contains("base64_decode"), "{error}");
}

#[test]
// req:edges-006
fn crypto_digests_bytes_and_draws_from_its_own_stream() {
    fn pair(draw: bool) -> (f64, f64) {
        let mut world = start(
            r#"
            import "kith:crypto" hmac_sha256 sha256_bytes Crypto;
            import "kith:encoding" utf8_encode hex_encode;
            export Probe;
            export Main;
            component Probe {
              field got: any;
              field n: number = 0;
              crypto: Crypto;
              start {
                on down "A" { n -> (rand 0 1000000); }
                on down "C" { n -> (rand 0 1000000); }
                on down "Sha" { got -> (hex_encode (sha256_bytes (utf8_encode "hi"))); }
                on down "Mac" {
                  got -> (hex_encode (hmac_sha256 (utf8_encode "key") (utf8_encode "The quick brown fox jumps over the lazy dog")));
                }
                on down "Draw" { send to crypto "Random" 4; }
                on up message:any { got -> message; }
              }
            }
            export scene Main { Probe() { crypto = Crypto(); } }
            "#,
        );
        world.send(world.root_id(), serde_json::json!("A")).unwrap();
        world.run_until_idle().unwrap();
        let first = world.field(world.root_id(), "n").unwrap().as_f64().unwrap();
        if draw {
            world
                .send(world.root_id(), serde_json::json!("Draw"))
                .unwrap();
            world.run_until_idle().unwrap();
            let bytes = doc::as_bytes(&doc::items(&got(&world)).unwrap()[1]).unwrap();
            assert_eq!(bytes.len(), 4);
        }
        world.send(world.root_id(), serde_json::json!("C")).unwrap();
        world.run_until_idle().unwrap();
        let second = world.field(world.root_id(), "n").unwrap().as_f64().unwrap();
        if !draw {
            world
                .send(world.root_id(), serde_json::json!("Sha"))
                .unwrap();
            world.run_until_idle().unwrap();
            assert_eq!(
                doc::as_str(&got(&world)).as_deref(),
                Some("8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4")
            );
            world
                .send(world.root_id(), serde_json::json!("Mac"))
                .unwrap();
            world.run_until_idle().unwrap();
            assert_eq!(
                doc::as_str(&got(&world)).as_deref(),
                Some("f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8")
            );
            let crypto = child(&world, "Crypto");
            world
                .send_document(crypto, down("Random", [doc::number(-1.0)]))
                .unwrap();
            world.run_until_idle().unwrap();
            assert_eq!(got(&world), down("Error", [doc::string("length")]));
        }
        (first, second)
    }
    assert_eq!(pair(false), pair(true));

    let program = load(
        r#"
        import "kith:crypto" Crypto;
        export Probe;
        export Main;
        component Probe {
          field got: any;
          crypto: Crypto;
          start { on up message:any { got -> message; } }
        }
        export scene Main { Probe() { crypto = Crypto(); } }
        "#,
    );
    let mut world = World::start(program.clone()).unwrap();
    let bytes = doc::bytes(b"abcd".to_vec());
    world.script_driver(
        child(&world, "Crypto"),
        vec![vec![down("Bytes", [bytes.clone()])]],
    );
    let snap = world.snapshot().unwrap();
    let mut restored = World::restore(program, &snap).unwrap();
    send(&mut restored, "Crypto", down("Random", [doc::number(4.0)]));
    assert_eq!(got(&restored), down("Bytes", [bytes]));
}

#[test]
// req:edges-007
fn paths_are_pure_slashes() {
    let mut world = start(
        r#"
        import "kith:path" join dirname basename extname resolve;
        export Probe;
        export Main;
        component Probe {
          field got: any;
          start {
            on down "Join" { got -> (join "a" "" "b"); }
            on down "Abs" { got -> (join "a" "/b" "c"); }
            on down "Dir" { got -> (dirname "a"); }
            on down "Root" { got -> (dirname "/"); }
            on down "Base" { got -> (basename "/"); }
            on down "Dot" { got -> (extname ".gitignore"); }
            on down "Ext" { got -> (extname "a.tar.gz"); }
            on down "Rel" { got -> (resolve "/base" "../x"); }
            on down "Top" { got -> (resolve "/base" "/abs"); }
          }
        }
        export scene Main { Probe() {} }
        "#,
    );
    for (message, expected) in [
        ("Join", "a/b"),
        ("Abs", "/b/c"),
        ("Dir", "."),
        ("Root", "/"),
        ("Base", ""),
        ("Dot", ""),
        ("Ext", ".gz"),
        ("Rel", "/base/../x"),
        ("Top", "/abs"),
    ] {
        world
            .send(world.root_id(), serde_json::json!(message))
            .unwrap();
        world.run_until_idle().unwrap();
        assert_eq!(
            doc::as_str(&got(&world)).as_deref(),
            Some(expected),
            "{message}"
        );
    }
}

#[test]
// req:edges-008
fn process_freezes_argv_env_and_exit() {
    let source = r#"
        import "kith:process" Process;
        export Probe;
        export Main;
        component Probe {
          field got: any;
          proc: Process;
          start {
            on down "Argv" { send to proc "Argv"; }
            on down "Env" name:string { send to proc "Env" name; }
            on down "Stop" { send to proc "Exit" 4; }
            on up message:any { got -> message; }
          }
        }
        export scene Main { Probe() { proc = Process(); } }
    "#;
    let program = load(source);
    let mut world = World::start(program.clone()).unwrap();
    world.set_argv(vec!["one".into(), "two".into()]);
    world.replace_process(vec!["one".into(), "two".into()], &[("ONLY", "yes")]);
    send(&mut world, "Process", doc::string("Argv"));
    assert_eq!(
        got(&world),
        down(
            "Argv",
            [doc::array([doc::string("one"), doc::string("two")])]
        )
    );
    send(&mut world, "Process", doc::string("Argv"));
    assert_eq!(
        got(&world),
        down(
            "Argv",
            [doc::array([doc::string("one"), doc::string("two")])]
        )
    );
    send(&mut world, "Process", down("Env", [doc::string("ONLY")]));
    assert_eq!(
        got(&world),
        down("Env", [doc::string("ONLY"), doc::string("yes")])
    );
    send(&mut world, "Process", down("Env", [doc::string("PATH")]));
    assert_eq!(got(&world), down("Missing", [doc::string("PATH")]));
    let snap = world.snapshot().unwrap();
    let mut restored = World::restore(program, &snap).unwrap();
    send(&mut restored, "Process", doc::string("Argv"));
    assert_eq!(
        got(&restored),
        down(
            "Argv",
            [doc::array([doc::string("one"), doc::string("two")])]
        )
    );
    send(&mut restored, "Process", down("Env", [doc::string("ONLY")]));
    assert_eq!(
        got(&restored),
        down("Env", [doc::string("ONLY"), doc::string("yes")])
    );
    let before = got(&world);
    send(&mut world, "Process", down("Exit", [doc::number(4.0)]));
    assert_eq!(got(&world), before);
    assert_eq!(world.exit_code(), Some(4));

    let dir = scratch(&[(
        "main.kith",
        r#"
        import "kith:process" Process;
        export Probe;
        export Main;
        component Probe {
          field got: any;
          proc: Process;
          start {
            on init { send to proc "Argv"; }
            on up "Argv" args:any {
              got -> args;
              send to proc "Exit" 3;
            }
          }
        }
        export scene Main { Probe() { proc = Process(); } }
        "#,
    )]);
    let ran = Command::new(env!("CARGO_BIN_EXE_kith"))
        .arg("run")
        .arg(dir.join("main.kith"))
        .arg("one")
        .arg("two")
        .output()
        .unwrap();
    assert_eq!(
        ran.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let stdout = String::from_utf8_lossy(&ran.stdout);
    assert!(stdout.contains("one"), "{stdout}");
    assert!(stdout.contains("two"), "{stdout}");
}
