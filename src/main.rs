use clap::Parser;
use gix_features::zlib;
use gix_pack::data::{entry, input};
use gix_protocol::{
    fetch,
    transport::{self, client::connect},
};
use std::io::Write;
use std::sync::mpsc;

#[derive(Parser, Debug)]
#[command(about = "git cat-file -p on a remote using the \"smart\" protocol")]
pub struct Opts {
    #[arg(required = true, value_parser = parse_url)]
    pub url: gix_url::Url,

    #[arg(required = true)]
    pub id: gix_hash::ObjectId,
}

pub fn parse_url(s: &str) -> Result<gix_url::Url, gix_url::parse::Error> {
    gix_url::parse(s.into())
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing git url: {0}")]
    Connecting(#[from] connect::Error),

    #[error("io error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("thread panicked")]
    ThreadPanicked,

    #[error("server does not advertise version 2 protocol")]
    UnsupportedServer,

    #[error("git client error: {0}")]
    GitClientError(#[from] transport::client::Error),

    #[error("git response error: {0}")]
    Fetching(#[from] fetch::response::Error),

    #[error("reading pkt-line: {0}")]
    ReadingPktLine(#[from] gix_packetline::decode::Error),

    #[error("with pack response: {0}")]
    BadPack(String),

    #[error("decoding pack band: {0}")]
    DecodingPackBand(#[from] gix_packetline::decode::band::Error),

    #[error("iterating pack entries: {0}")]
    IteratingPackEntries(#[from] input::Error),

    #[error("no commit found")]
    NoCommitFound,

    #[error("decompressing: {0}")]
    Decompressing(#[from] gix_features::zlib::inflate::Error),

    #[error("fitting git sizes into usize")]
    BitWidthMismatch(#[from] std::num::TryFromIntError),
}

impl From<Box<dyn std::any::Any + Send>> for Error {
    fn from(_: Box<dyn std::any::Any + Send>) -> Self {
        Error::ThreadPanicked
    }
}

pub fn main() {
    if let Err(e) = main_inner() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

pub fn main_inner() -> Result<(), Error> {
    let opts = Opts::parse();

    let mut con = connect(
        opts.url,
        connect::Options {
            version: transport::Protocol::V2,
            ..connect::Options::default()
        },
    )?;

    {
        let handshake = con.handshake(transport::Service::UploadPack, &[])?;

        if handshake.actual_protocol != transport::Protocol::V2 {
            Err(Error::UnsupportedServer)?;
        }
    }

    let mut args = fetch::Arguments::new(
        transport::Protocol::V2,
        vec![("no-progress", None), ("shallow", None)],
        false,
    );
    args.want(opts.id);
    args.deepen(1);

    let (pkt_lines_r, mut pkt_lines_w) = os_pipe::pipe()?;

    let (entries_tx, entries_rx) = mpsc::channel();

    let pkt_lines_hdl = std::thread::spawn(move || -> Result<Vec<u8>, Error> {
        let mut buf = args.send(&mut con, true)?;

        let res =
            fetch::Response::from_line_reader(transport::Protocol::V2, &mut buf, true, false)?;

        if !res.has_pack() {
            panic!("no pack from server");
        }

        let mut data = Vec::with_capacity(4096);

        while let Some(line) = buf.readline() {
            use gix_packetline::BandRef;
            match line??.decode_band()? {
                BandRef::Data(d) => {
                    pkt_lines_w.write(d)?;
                    data.extend_from_slice(d);
                }
                BandRef::Progress(_d) => {}
                BandRef::Error(d) => {
                    return Err(Error::BadPack(String::from_utf8_lossy(d).into_owned()));
                }
            }
        }

        Ok(data)
    });

    let entries_hdl = std::thread::spawn(move || -> Result<(), Error> {
        let entries = input::BytesToEntriesIter::new_from_header(
            std::io::BufReader::new(pkt_lines_r),
            input::Mode::Verify,
            input::EntryDataMode::KeepAndCrc32,
            gix_hash::Kind::default(),
        )?;

        Ok(for entry in entries {
            entries_tx.send(entry).unwrap_or(())
        })
    });

    let find_commit_hdl = std::thread::spawn(move || -> Result<input::Entry, Error> {
        while let Ok(msg) = entries_rx.recv() {
            let entry = msg?;

            if entry.header == entry::Header::Commit {
                return Ok(entry);
            }
        }

        Err(Error::NoCommitFound)
    });

    entries_hdl.join()??;

    let data = pkt_lines_hdl.join()??;

    let entry = find_commit_hdl.join()??;

    let entry_size = entry.decompressed_size.try_into()?;

    let entry_offset = (entry.pack_offset + entry.header_size as u64).try_into()?;

    let mut commit_obj = Vec::with_capacity(entry_size);

    let (_status, _consumed_in, _consumed_out) =
        zlib::Inflate::default().once(&data[entry_offset..], &mut commit_obj)?;

    println!("{}", String::from_utf8_lossy(&commit_obj).to_owned());

    Ok(())
}