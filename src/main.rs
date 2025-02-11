use std::collections::{HashMap, HashSet};
use std::fs;
use std::ops::{Add, Sub};
use std::str::FromStr;
use std::string::ToString;

use anyhow::{anyhow, bail, ensure, Context};
use clap::Parser;
use regex::{Match, Regex};

mod diff;

#[derive(Debug, Clone)]
struct Capture {
    regex: Regex,
    names: HashSet<String>,
}

impl FromStr for Capture {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let regex = Regex::new(s).context(format!("'{s}' not a valid regex"))?;
        let names = regex
            .capture_names()
            .filter_map(|n| n.map(ToString::to_string))
            .collect::<HashSet<_>>();
        Ok(Self { regex, names })
    }
}

#[derive(Debug, Clone)]
struct Operator {
    target: String,
    op: Operation,
    value: Param,
}

#[derive(Debug, Clone)]
enum Operation {
    Inc,
    Dec,
    Replace,
}

#[derive(Debug, Clone)]
enum Param {
    Int(isize),
    String(String),
    Capture(String),
}

#[allow(clippy::unwrap_used)]
impl From<&str> for Param {
    fn from(value: &str) -> Self {
        value.parse::<isize>().map_or_else(
            |_| {
                let re = Regex::new(r"<([^>]+)>").unwrap();
                re.captures(value).map_or_else(
                    || Self::String(value.to_string()),
                    |m| Self::Capture(m.get(1).unwrap().as_str().to_string()),
                )
            },
            Self::Int,
        )
    }
}

impl FromStr for Operator {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(r"<([^>]+)>:([^:]+):?([^:]+)?")?;
        let m = re
            .captures(s)
            .ok_or_else(|| anyhow!(format!("'{s}' not a valid operator format")))?;
        ensure!(m.len() == 4, format!("'{s}' not a valid operator format"));

        let target = m
            .get(1)
            .ok_or_else(|| anyhow!("no target in operator"))?
            .as_str()
            .to_string();

        let param = m.get(3).map(|p| Param::from(p.as_str()));

        Ok(
            match m
                .get(2)
                .ok_or_else(|| anyhow!("no operation in operator"))?
                .as_str()
            {
                "inc" => Self {
                    target,
                    op: Operation::Inc,
                    value: param.unwrap_or(Param::Int(1)),
                },
                "dec" => Self {
                    target,
                    op: Operation::Dec,
                    value: param.unwrap_or(Param::Int(1)),
                },
                "rep" => Self {
                    target,
                    op: Operation::Replace,
                    value: param.ok_or_else(|| anyhow!("parameter required in 'rep' operator"))?,
                },
                o => {
                    bail!(format!("'{o}' is not a valid operator"))
                }
            },
        )
    }
}

/// regop tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Regop {
    /// Overwrite existing files
    #[arg(short, long)]
    #[clap(default_value_t = false)]
    save: bool,

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

    /// File to operate on, can be repeated
    #[arg()]
    file: Vec<String>,
}

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

fn handle_file(regop: &Regop, file: &str) -> anyhow::Result<()> {
    let old_content = fs::read_to_string(file).context(format!("unable to read file '{file}'"))?;
    if !regop.save {
        if let Some(new_content) =
            process(regop.lines, &regop.regex, &regop.op, old_content.clone())?
        {
            diff::diff(file, &old_content, &new_content);
        }
    } else if let Some(new_content) = process(regop.lines, &regop.regex, &regop.op, old_content)? {
        fs::write(file, new_content).context(format!("unable to write file '{file}'"))?;
    }

    Ok(())
}

fn process(
    lines: bool,
    regex: &[Capture],
    ops: &[Operator],
    mut content: String,
) -> anyhow::Result<Option<String>> {
    if lines {
        let mut change = false;

        for line in content.clone().lines() {
            if let Some(new_line) = regop(regex, ops, line.to_string())? {
                change = true;
                let start = content
                    .find(line)
                    .ok_or_else(|| anyhow!("unable to find line index"))?;
                content.replace_range(start..start + line.len(), &new_line);
            }
        }

        if change {
            Ok(Some(content))
        } else {
            Ok(None)
        }
    } else {
        regop(regex, ops, content)
    }
}

fn regop(
    regex: &[Capture],
    ops: &[Operator],
    mut content: String,
) -> anyhow::Result<Option<String>> {
    let captures_as_values = ops
        .iter()
        .filter_map(|op| {
            if let Param::Capture(c) = &op.value {
                Some(c.clone())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    let mut captures: HashMap<String, Vec<(usize, usize, &str)>> = HashMap::new();

    // First pass to get all value captures
    for cap in regex {
        for name in &cap.names {
            if captures_as_values.contains(name) {
                for m in cap.regex.captures_iter(&content) {
                    for n in &cap.names {
                        if let Some(m) = m.name(n) {
                            let e = captures.entry(n.clone()).or_default();
                            e.push((m.start(), m.end(), &content[m.start()..m.end()]));
                        }
                    }
                }
                break;
            }
        }
    }

    for cap in &captures_as_values {
        ensure!(
            captures.contains_key(cap),
            format!("'<{cap}>' used as value but not found")
        );
    }

    let mut edits = Vec::new();

    // Second pass to collect edits
    for op in ops {
        for cap in regex {
            if cap.names.contains(&op.target) {
                for m in cap.regex.captures_iter(&content) {
                    if let Some(m) = m.name(&op.target) {
                        edits.push(edit(op, &m, &content[m.start()..m.end()], &captures)?);
                    }
                }
            }
        }
    }

    // Apply edits
    edits.sort_by_key(|e| e.start);
    edits.reverse();
    for ed in edits.windows(2) {
        distance(ed[0].start, ed[0].end, ed[1].start, ed[1].end)
            .ok_or_else(|| anyhow!("edits overlap each other"))?;
    }

    for ed in &edits {
        content.replace_range(ed.start..ed.end, &ed.new);
    }

    if edits.is_empty() {
        Ok(None)
    } else {
        Ok(Some(content))
    }
}

struct Edit {
    start: usize,
    end: usize,
    new: String,
}

fn edit<'a>(
    op: &Operator,
    m: &Match<'_>,
    old: &'a str,
    captures: &HashMap<String, Vec<(usize, usize, &'a str)>>,
) -> anyhow::Result<Edit> {
    let start = m.start();
    let end = m.end();

    let value = match &op.value {
        Param::Capture(name) => {
            let c = captures.get(name).map(|v| {
                let mut c = v
                    .iter()
                    .map(|c| (distance(start, end, c.0, c.1), c.2))
                    .collect::<Vec<_>>();
                c.sort_by_key(|c| c.0);
                #[allow(clippy::unwrap_used)]
                c.first().unwrap().1 // It is safe to unwrap here
            });
            Param::String(
                c.ok_or_else(|| anyhow!(format!("no capture found named '{name}'")))?
                    .to_string(),
            )
        }
        v => v.clone(),
    };

    let new = match op.op {
        Operation::Inc => match value {
            Param::Int(num) => parse_int(old)?.add(num).to_string(),
            Param::String(num) => parse_int(old)?.add(parse_int(&num)?).to_string(),
            Param::Capture(_) => bail!("this should not happen"),
        },
        Operation::Dec => match value {
            Param::Int(num) => parse_int(old)?.sub(num).to_string(),
            Param::String(num) => parse_int(old)?.sub(parse_int(&num)?).to_string(),
            Param::Capture(_) => bail!("this should not happen"),
        },
        Operation::Replace => match value {
            Param::Int(i) => format!("{i}"),
            Param::String(s) => s,
            Param::Capture(_) => bail!("this should not happen"),
        },
    };

    Ok(Edit { start, end, new })
}

fn parse_int(s: &str) -> anyhow::Result<isize> {
    s.parse::<isize>()
        .context(format!("cannot parse '{s}' as int"))
}

const fn distance(start_a: usize, end_a: usize, start_b: usize, end_b: usize) -> Option<usize> {
    if end_a <= start_b {
        Some(start_b - end_a)
    } else if end_b <= start_a {
        Some(start_a - end_b)
    } else {
        None
    }
}
