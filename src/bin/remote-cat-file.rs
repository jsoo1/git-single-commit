use clap::Parser;
use git_single_object_fetch::cat_file;

#[derive(Parser, Debug)]
#[command(about = "git cat-file -p on a remote using the \"smart\" protocol")]
pub struct Opts {
    #[arg(required = true, value_parser = parse_url)]
    pub url: gix_url::Url,

    #[arg(required = true)]
    pub commit: gix_hash::ObjectId,
}

pub fn parse_url(s: &str) -> Result<gix_url::Url, gix_url::parse::Error> {
    gix_url::parse(s.into())
}

pub fn main() {
    let opts = Opts::parse();

    if let Err(e) = cat_file::main(opts.url, opts.commit) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
