//! Transport setup for the LSP connection.
//!
//! `lsp_server` can build connections over stdio and TCP; the named-pipe /
//! Unix-domain-socket transport is implemented here on top of the public
//! `Message::read`/`Message::write` API, mirroring `lsp_server::socket`.

use std::io::{self, BufRead, Write};
use std::thread;

use crossbeam_channel::bounded;
use lsp_server::{Connection, IoThreads, Message};

use crate::cli::Transport;

/// IO threads owned by this crate (named pipes only).
pub struct OwnedIoThreads {
    reader: thread::JoinHandle<io::Result<()>>,
    writer: thread::JoinHandle<io::Result<()>>,
    dropper: thread::JoinHandle<()>,
}

impl OwnedIoThreads {
    pub fn join(self) -> io::Result<()> {
        match self.reader.join() {
            Ok(result) => result?,
            Err(payload) => std::panic::resume_unwind(payload),
        }
        if let Err(payload) = self.dropper.join() {
            std::panic::resume_unwind(payload);
        }
        match self.writer.join() {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

/// The IO threads for whichever transport was selected.
pub enum Io {
    Lsp(IoThreads),
    Owned(OwnedIoThreads),
}

impl Io {
    pub fn join(self) -> io::Result<()> {
        match self {
            Io::Lsp(threads) => threads.join(),
            Io::Owned(threads) => threads.join(),
        }
    }
}

/// Build the LSP connection for `transport`.
pub fn create(transport: &Transport) -> io::Result<(Connection, Io)> {
    match transport {
        Transport::Stdio => {
            let (connection, threads) = Connection::stdio();
            Ok((connection, Io::Lsp(threads)))
        }
        Transport::Socket { port } => {
            // Upstream's `createServerSocketTransport` connects to the
            // client's listener on the loopback interface.
            let (connection, threads) = Connection::connect(("127.0.0.1", *port))?;
            Ok((connection, Io::Lsp(threads)))
        }
        Transport::Pipe { name } => {
            let (connection, threads) = connect_pipe(name)?;
            Ok((connection, Io::Owned(threads)))
        }
    }
}

#[cfg(unix)]
fn connect_pipe(name: &str) -> io::Result<(Connection, OwnedIoThreads)> {
    let stream = std::os::unix::net::UnixStream::connect(name)?;
    let reader = stream.try_clone()?;
    Ok(stream_transport(io::BufReader::new(reader), stream))
}

#[cfg(windows)]
fn connect_pipe(name: &str) -> io::Result<(Connection, OwnedIoThreads)> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(name)?;
    let reader = file.try_clone()?;
    Ok(stream_transport(io::BufReader::new(reader), file))
}

#[cfg(not(any(unix, windows)))]
fn connect_pipe(_name: &str) -> io::Result<(Connection, OwnedIoThreads)> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "named pipe transport is not supported on this platform",
    ))
}

/// Build a `Connection` over an arbitrary duplex stream pair.
///
/// This mirrors `lsp_server`'s private socket transport: the reader/writer
/// threads move `Message`s across bounded channels and a third thread drops
/// already-written messages so the writer keeps its zero-capacity semantics.
fn stream_transport<R, W>(reader: R, writer: W) -> (Connection, OwnedIoThreads)
where
    R: BufRead + Send + 'static,
    W: Write + Send + 'static,
{
    let (drop_sender, drop_receiver) = bounded::<Message>(0);
    let (writer_sender, writer_receiver) = bounded::<Message>(0);
    let writer = thread::spawn(move || {
        let mut writer = writer;
        writer_receiver.into_iter().try_for_each(|message| {
            let result = message.write(&mut writer);
            let _ = drop_sender.send(message);
            result
        })
    });
    let dropper = thread::spawn(move || drop_receiver.into_iter().for_each(drop));

    let (reader_sender, reader_receiver) = bounded::<Message>(0);
    let reader = thread::spawn(move || {
        let mut reader = reader;
        while let Some(message) = Message::read(&mut reader)? {
            let is_exit = matches!(&message, Message::Notification(n) if n.method == "exit");
            if let Err(error) = reader_sender.send(message) {
                return Err(io::Error::other(error));
            }
            if is_exit {
                break;
            }
        }
        Ok(())
    });

    (
        Connection {
            sender: writer_sender,
            receiver: reader_receiver,
        },
        OwnedIoThreads {
            reader,
            writer,
            dropper,
        },
    )
}
