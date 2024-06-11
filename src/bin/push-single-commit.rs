use clap::Parser;
use git_single_object_fetch::push;
use std::io::Read;

#[derive(Parser, Debug)]
#[command(about = "push a single object from stdin to a remote")]
pub struct Opts {
    #[arg(required = true, value_parser = parse_url)]
    pub url: gix_url::Url,

    #[arg(required = true)]
    pub rref: String,
}

pub fn parse_url(s: &str) -> Result<gix_url::Url, gix_url::parse::Error> {
    gix_url::parse(s.into())
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    IOError(#[from] std::io::Error),

    #[error("decoding commit: {0}")]
    DecodingCommit(#[from] gix_object::decode::Error),

    #[error("pushing: {0}")]
    Pushing(#[from] push::Error),
}

pub fn main() {
    if let Err(e) = main_inner() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

pub fn main_inner() -> Result<(), Error> {
    let opts = Opts::parse();

    let mut o = Vec::new();

    std::io::stdin().read_to_end(&mut o)?;

    let commit = gix_object::CommitRef::from_bytes(&o)?;

    let id = gix_object::compute_hash(gix_hash::Kind::default(), gix_object::Kind::Commit, &o);

    push::main(opts.url, commit, id, &opts.rref.as_bytes())?;

    Ok(())
}
