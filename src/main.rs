//! Command-line interface for regop.
//!
//! This binary provides a powerful text transformation tool that uses
//! regular expressions with named capture groups and operators.

use std::fs;
use std::io::Read;

use anyhow::{Context, ensure};
use clap::Parser;

mod diff;

use regop::{Capture, Operator, process};

/// Easy file manipulation with regex and operators.
///
/// Use regular expressions with named capture groups to extract values,
/// then apply operators to transform those values.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    long_about = None,
    after_help = concat!("\x1b[1m\x1b[4mExamples:\x1b[0m", r#"

  # Increment edition in Cargo.toml by one
  regop \
    -r 'edition = "(?<edition>[^"]+)' \
    -o '<edition>:inc' \
    Cargo.toml

  # Swap anyhow major and patch version, increment minor by 3
  regop \
    -r 'anyhow = "(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)"' \
    -o '<major>:swap:<patch>' \
    -o '<minor>:inc:3' \
    Cargo.toml

  # Update all major versions in all toml files
  find -name '*.toml' | regop \
    -w \
    -r '"(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)"' \
    -o '<major>:inc'

  # Read from stdin and write to stdout
  cat Cargo.toml | regop \
    -w \
    -r "version = \"(?<major>\d)\.(?<minor>\d)" \
    -o "<major>:rep:21" \
    -"#)
)]
struct Regop {
    /// Write to files, will write to stdout if input file is `-`
    #[arg(short, long)]
    #[clap(default_value_t = false)]
    write: bool,

    /// Operate on lines induvidually, one by one
    #[arg(short, long)]
    #[clap(default_value_t = false)]
    lines: bool,

    /// Regular expression, can be repeated
    #[arg(short, long, value_parser = clap::value_parser!(Capture))]
    regex: Vec<Capture>,

    /// Operator, can be repeated
    #[arg(short, long, value_parser = clap::value_parser!(Operator))]
    op: Vec<Operator>,

    /// File to operate on, use `-` for stdin, can be repeated
    #[arg()]
    file: Vec<String>,
}

/// Main entry point for the regop CLI.
fn main() -> anyhow::Result<()> {
    let regop = Regop::parse();

    if regop.file.is_empty() {
        ensure!(
            !atty::is(atty::Stream::Stdin),
            "supply filename or pipe a list of files to stdin"
        );
        for file in std::io::stdin().lines() {
            handle_file(&regop, &file?)?;
        }
    } else {
        for file in &regop.file {
            handle_file(&regop, file)?;
        }
    }

    Ok(())
}

/// Process a single file with the given regex patterns and operators.
///
/// Handles both regular files and stdin (when file is "-").
/// In preview mode (default), shows a diff of changes.
/// In write mode (-w flag), applies changes to the file.
fn handle_file(regop: &Regop, file: &str) -> anyhow::Result<()> {
    let old_content = match file {
        "-" => {
            let mut stdin = String::new();
            std::io::stdin().read_to_string(&mut stdin)?;
            stdin
        }
        _ => fs::read_to_string(file).context(format!("unable to read file '{file}'"))?,
    };

    if !regop.write {
        if let Some(new_content) =
            process(regop.lines, &regop.regex, &regop.op, old_content.clone())?
        {
            diff::diff(file, &old_content, &new_content);
        }
    } else if let Some(new_content) = process(regop.lines, &regop.regex, &regop.op, old_content)? {
        match file {
            "-" => print!("{new_content}"),
            _ => fs::write(file, new_content).context(format!("unable to write file '{file}'"))?,
        }
    }

    Ok(())
}
