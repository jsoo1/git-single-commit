use gix_protocol::transport::{
    self,
    client::{self, connect, TransportWithoutIO},
};
use std::io::Write;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing git url: {0}")]
    Connecting(#[from] connect::Error),

    #[error("parsing commit: {0}")]
    ParsingCommit(#[from] gix_object::decode::Error),

    #[error("io error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("git client error: {0}")]
    GitClientError(#[from] transport::client::Error),

    #[error("reading pkt-line: {0}")]
    ReadingPktLine(#[from] gix_packetline::decode::Error),
}

pub fn main<'a>(
    url: gix_url::Url,
    _commit: gix_object::CommitRef<'a>,
    id: gix_hash::ObjectId,
    rref: &[u8],
) -> Result<(), Error> {
    let mut con = connect(
        url,
        connect::Options {
            version: transport::Protocol::V1,
            ..connect::Options::default()
        },
    )?;

    let mut line_writer = con.request(
        client::WriteMode::OneLfTerminatedLinePerWriteCall,
        client::MessageKind::Text(&b"done"[..]),
        false,
    )?;

    let create: &'static [u8] = {
        let mut b = Vec::from(b"0000000000000000000000000000000000000000 ");

        b.extend_from_slice(id.to_string().as_bytes());

        b.extend_from_slice(b" ");

        b.extend_from_slice(rref);

        b.extend_from_slice(&[0; 1]);

        b.extend_from_slice(b" ");

        b.leak()
    };

    std::io::stderr().write_all(create)?;

    std::io::stderr().write_all(b"\n")?;

    line_writer.write_message(client::MessageKind::Text(create))?;

    let (w, r) = line_writer.into_parts();

    let mut line_writer = client::RequestWriter::new_from_bufread(
        w,
        r,
        client::WriteMode::Binary,
        client::MessageKind::Flush,
        false,
    );

    line_writer.write_message(client::MessageKind::Text(b"PACK"))?;

    let mut buf = line_writer.into_read()?;

    while let Some(line) = buf.readline() {
        let l = line??.as_slice().unwrap_or(b"line was not a slice");
        eprintln!("{}", String::from_utf8_lossy(l).to_owned());
    }

    Ok(())
}
