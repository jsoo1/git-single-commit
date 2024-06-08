use clap::Parser;
use git_single_object_fetch::push;
use std::io::Read;

#[derive(Parser, Debug)]
#[command(about = "push a single object from stdin to a remote")]
pub struct Opts {
    #[arg(required = true, value_parser = parse_url)]
    pub url: gix_url::Url,
}

pub fn parse_url(s: &str) -> Result<gix_url::Url, gix_url::parse::Error> {
    gix_url::parse(s.into())
}

pub fn main() {
    let opts = Opts::parse();

    let obj = {
        let mut o = Vec::new();

        if let Err(e) = std::io::stdin().read_to_end(&mut o) {
            eprintln!("{}", e);
            std::process::exit(1);
        }

        o
    };

    if let Err(e) = push::main(opts.url, obj) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
