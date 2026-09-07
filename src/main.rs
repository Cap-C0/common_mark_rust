use clap::Parser;
use common_mark_rust::markdown_to_html;
use std::fs::{self};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// .md file to get string from
    #[arg(short, long, conflicts_with = "string")]
    file: Option<std::path::PathBuf>,
    /// markdown string as is
    #[arg(short, long, conflicts_with = "file")]
    string: Option<String>,
    /// optional output file
    #[arg(short, long)]
    output_file: Option<std::path::PathBuf>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let mut my_str = String::new();
    if let Some(path) = args.file {
        my_str = fs::read_to_string(path)?;
    } else if args.string.is_some() {
        my_str = args.string.unwrap().into();
    }
    let html_out = markdown_to_html(&my_str);
    if let Some(out_path) = args.output_file {
        fs::write(out_path, html_out)?;
    } else {
        print!("{}", html_out);
    }
    Ok(())
}
