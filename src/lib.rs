use std::collections::{HashMap, HashSet};
use std::ops::{Add, Sub};
use std::str::FromStr;
use std::string::ToString;

use anyhow::{Context, anyhow, bail, ensure};
use regex::{Match, Regex};

type CapturesMap<'a> = HashMap<String, Vec<(usize, usize, &'a str)>>;

#[derive(Debug, Clone)]
pub struct Capture {
    pub regex: Regex,
    pub names: HashSet<String>,
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
pub struct Operator {
    pub target: String,
    pub op: Operation,
    pub value: Param,
}

#[derive(Debug, Clone)]
pub enum Operation {
    Inc,
    Dec,
    Replace,
    Del,
    Swap,
    Mul,
    Div,
    Append,
    Prepend,
    Upper,
    Lower,
}

#[derive(Debug, Clone)]
pub enum Param {
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
                "del" => Self {
                    target,
                    op: Operation::Del,
                    value: Param::Int(0),
                },
                "swap" => Self {
                    target,
                    op: Operation::Swap,
                    value: param.ok_or_else(|| anyhow!("parameter required in 'swap' operator"))?,
                },
                "mul" => Self {
                    target,
                    op: Operation::Mul,
                    value: param.ok_or_else(|| anyhow!("parameter required in 'mul' operator"))?,
                },
                "div" => Self {
                    target,
                    op: Operation::Div,
                    value: param.ok_or_else(|| anyhow!("parameter required in 'div' operator"))?,
                },
                "append" => Self {
                    target,
                    op: Operation::Append,
                    value: param
                        .ok_or_else(|| anyhow!("parameter required in 'append' operator"))?,
                },
                "prepend" => Self {
                    target,
                    op: Operation::Prepend,
                    value: param
                        .ok_or_else(|| anyhow!("parameter required in 'prepend' operator"))?,
                },
                "upper" => Self {
                    target,
                    op: Operation::Upper,
                    value: Param::Int(0),
                },
                "lower" => Self {
                    target,
                    op: Operation::Lower,
                    value: Param::Int(0),
                },
                o => {
                    bail!(format!("'{o}' is not a valid operator"))
                }
            },
        )
    }
}

pub fn process(
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

        if change { Ok(Some(content)) } else { Ok(None) }
    } else {
        regop(regex, ops, content)
    }
}

pub fn regop(
    regex: &[Capture],
    ops: &[Operator],
    mut content: String,
) -> anyhow::Result<Option<String>> {
    let captures_as_values = collect_captures_as_values(ops);
    let captures = collect_value_captures(regex, &content, &captures_as_values)?;
    let mut edits = collect_edits(ops, regex, &content, &captures)?;

    apply_edits(&mut content, &mut edits)?;

    if edits.is_empty() {
        Ok(None)
    } else {
        Ok(Some(content))
    }
}

fn collect_captures_as_values(ops: &[Operator]) -> HashSet<String> {
    ops.iter()
        .filter_map(|op| {
            if let Param::Capture(c) = &op.value {
                if matches!(op.op, Operation::Swap) {
                    None
                } else {
                    Some(c.clone())
                }
            } else {
                None
            }
        })
        .collect::<HashSet<_>>()
}

fn collect_value_captures<'a>(
    regex: &[Capture],
    content: &'a str,
    captures_as_values: &HashSet<String>,
) -> anyhow::Result<CapturesMap<'a>> {
    let mut captures: CapturesMap = HashMap::new();

    // First pass to get all value captures
    for cap in regex {
        for name in &cap.names {
            if captures_as_values.contains(name) {
                for m in cap.regex.captures_iter(content) {
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

    for cap in captures_as_values {
        ensure!(
            captures.contains_key(cap),
            format!("'<{cap}>' used as value but not found")
        );
    }

    Ok(captures)
}

fn collect_edits(
    ops: &[Operator],
    regex: &[Capture],
    content: &str,
    captures: &CapturesMap,
) -> anyhow::Result<Vec<Edit>> {
    let mut edits = Vec::new();

    for op in ops {
        if matches!(op.op, Operation::Swap) {
            collect_swap_edits(op, regex, content, &mut edits)?;
        } else {
            collect_regular_edits(op, regex, content, captures, &mut edits)?;
        }
    }

    Ok(edits)
}

fn collect_swap_edits(
    op: &Operator,
    regex: &[Capture],
    content: &str,
    edits: &mut Vec<Edit>,
) -> anyhow::Result<()> {
    let swap_target = match &op.value {
        Param::String(s) => s.clone(),
        Param::Capture(c) => c.clone(),
        Param::Int(i) => format!("{i}"),
    };

    let mut source_matches = Vec::new();
    let mut target_matches = Vec::new();

    // Collect all matches for both source and target
    for cap in regex {
        if cap.names.contains(&op.target) {
            for m in cap.regex.captures_iter(content) {
                if let Some(m) = m.name(&op.target) {
                    source_matches.push((m.start(), m.end(), &content[m.start()..m.end()]));
                }
            }
        }
        if cap.names.contains(&swap_target) {
            for m in cap.regex.captures_iter(content) {
                if let Some(m) = m.name(&swap_target) {
                    target_matches.push((m.start(), m.end(), &content[m.start()..m.end()]));
                }
            }
        }
    }

    ensure!(
        source_matches.len() == target_matches.len(),
        format!(
            "Cannot swap '{}' and '{}': different number of matches ({} vs {})",
            op.target,
            swap_target,
            source_matches.len(),
            target_matches.len()
        )
    );

    // Create edits for swapping
    for (source, target) in source_matches.iter().zip(target_matches.iter()) {
        edits.push(Edit {
            start: source.0,
            end: source.1,
            new: target.2.to_string(),
        });
        edits.push(Edit {
            start: target.0,
            end: target.1,
            new: source.2.to_string(),
        });
    }

    Ok(())
}

fn collect_regular_edits(
    op: &Operator,
    regex: &[Capture],
    content: &str,
    captures: &CapturesMap,
    edits: &mut Vec<Edit>,
) -> anyhow::Result<()> {
    for cap in regex {
        if cap.names.contains(&op.target) {
            for m in cap.regex.captures_iter(content) {
                if let Some(m) = m.name(&op.target) {
                    edits.push(edit(op, &m, &content[m.start()..m.end()], captures)?);
                }
            }
        }
    }
    Ok(())
}

fn apply_edits(content: &mut String, edits: &mut Vec<Edit>) -> anyhow::Result<()> {
    edits.sort_by_key(|e| e.start);
    edits.reverse();
    for ed in edits.windows(2) {
        distance(ed[0].start, ed[0].end, ed[1].start, ed[1].end)
            .ok_or_else(|| anyhow!("edits overlap each other"))?;
    }

    for ed in edits {
        content.replace_range(ed.start..ed.end, &ed.new);
    }

    Ok(())
}

pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub new: String,
}

pub fn edit<'a>(
    op: &Operator,
    m: &Match<'_>,
    old: &'a str,
    captures: &CapturesMap<'a>,
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
        Operation::Del => String::new(),
        Operation::Swap => match value {
            Param::String(s) => s,
            Param::Int(i) => format!("{i}"),
            Param::Capture(_) => bail!("this should not happen"),
        },
        Operation::Mul => match value {
            Param::Int(num) => parse_int(old)?.wrapping_mul(num).to_string(),
            Param::String(num) => parse_int(old)?.wrapping_mul(parse_int(&num)?).to_string(),
            Param::Capture(_) => bail!("this should not happen"),
        },
        Operation::Div => match value {
            Param::Int(num) => {
                ensure!(num != 0, "division by zero");
                (parse_int(old)? / num).to_string()
            }
            Param::String(num) => {
                let divisor = parse_int(&num)?;
                ensure!(divisor != 0, "division by zero");
                (parse_int(old)? / divisor).to_string()
            }
            Param::Capture(_) => bail!("this should not happen"),
        },
        Operation::Append => match value {
            Param::String(s) => format!("{old}{s}"),
            Param::Int(i) => format!("{old}{i}"),
            Param::Capture(_) => bail!("this should not happen"),
        },
        Operation::Prepend => match value {
            Param::String(s) => format!("{s}{old}"),
            Param::Int(i) => format!("{i}{old}"),
            Param::Capture(_) => bail!("this should not happen"),
        },
        Operation::Upper => old.to_uppercase(),
        Operation::Lower => old.to_lowercase(),
    };

    Ok(Edit { start, end, new })
}

pub fn parse_int(s: &str) -> anyhow::Result<isize> {
    s.parse::<isize>()
        .context(format!("cannot parse '{s}' as int"))
}

#[must_use]
pub const fn distance(start_a: usize, end_a: usize, start_b: usize, end_b: usize) -> Option<usize> {
    if end_a <= start_b {
        Some(start_b - end_a)
    } else if end_b <= start_a {
        Some(start_a - end_b)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a capture from a string
    fn capture(s: &str) -> Capture {
        s.parse().unwrap()
    }

    // Helper function to create an operator from a string
    fn operator(s: &str) -> Operator {
        s.parse().unwrap()
    }

    #[test]
    fn test_inc_operation() {
        let captures = vec![capture(r"version = (?<version>\d+)")];
        let operators = vec![operator("<version>:inc")];
        let content = "version = 5".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("version = 6".to_string()));
    }

    #[test]
    fn test_inc_operation_with_value() {
        let captures = vec![capture(r"version = (?<version>\d+)")];
        let operators = vec![operator("<version>:inc:10")];
        let content = "version = 5".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("version = 15".to_string()));
    }

    #[test]
    fn test_dec_operation() {
        let captures = vec![capture(r"version = (?<version>\d+)")];
        let operators = vec![operator("<version>:dec")];
        let content = "version = 10".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("version = 9".to_string()));
    }

    #[test]
    fn test_dec_operation_with_value() {
        let captures = vec![capture(r"version = (?<version>\d+)")];
        let operators = vec![operator("<version>:dec:3")];
        let content = "version = 10".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("version = 7".to_string()));
    }

    #[test]
    fn test_replace_operation() {
        let captures = vec![capture(r"name = (?<name>\w+)")];
        let operators = vec![operator(r#"<name>:rep:new_name"#)];
        let content = "name = old_name".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("name = new_name".to_string()));
    }

    #[test]
    fn test_replace_operation_with_number() {
        let captures = vec![capture(r"count = (?<count>\d+)")];
        let operators = vec![operator("<count>:rep:42")];
        let content = "count = 10".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("count = 42".to_string()));
    }

    #[test]
    fn test_del_operation() {
        let captures = vec![capture(r"temp = (?<temp>\w+)")];
        let operators = vec![operator("<temp>:del")];
        let content = "temp = value".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("temp = ".to_string()));
    }

    #[test]
    fn test_swap_operation() {
        let captures = vec![
            capture(r"first = (?<first>\w+)"),
            capture(r"second = (?<second>\w+)"),
        ];
        let operators = vec![operator("<first>:swap:<second>")];
        let content = "first = A\nsecond = B".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("first = B\nsecond = A".to_string()));
    }

    #[test]
    fn test_swap_operation_same_regex() {
        let captures = vec![capture(r"(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)")];
        let operators = vec![operator("<major>:swap:<patch>")];
        let content = "1.2.3".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("3.2.1".to_string()));
    }

    #[test]
    fn test_multiple_operations() {
        let captures = vec![capture(
            r"version = (?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)",
        )];
        let operators = vec![
            operator("<major>:inc"),
            operator("<minor>:dec:2"),
            operator("<patch>:rep:0"),
        ];
        let content = "version = 1.5.9".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("version = 2.3.0".to_string()));
    }

    #[test]
    fn test_capture_as_value() {
        let captures = vec![capture(r"(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)")];
        let operators = vec![operator("<major>:rep:<patch>")];
        let content = "1.2.3".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("3.2.3".to_string()));
    }

    #[test]
    fn test_no_matches() {
        let captures = vec![capture(r"version = (?<version>\d+)")];
        let operators = vec![operator("<version>:inc")];
        let content = "no matches here".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_multiple_matches() {
        let captures = vec![capture(r"(?<num>\d+)")];
        let operators = vec![operator("<num>:inc")];
        let content = "1 and 2 and 3".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("2 and 3 and 4".to_string()));
    }

    #[test]
    fn test_process_lines_mode() {
        let captures = vec![capture(r"value: (?<num>\d+)")];
        let operators = vec![operator("<num>:inc")];
        let content = "value: 5".to_string();

        let result = process(true, &captures, &operators, content).unwrap();
        assert_eq!(result, Some("value: 6".to_string()));
    }

    #[test]
    fn test_invalid_operator_format() {
        let result = "invalid".parse::<Operator>();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_regex() {
        let result = "[invalid".parse::<Capture>();
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_parameter_for_replace() {
        let result = "<test>:rep".parse::<Operator>();
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_parameter_for_swap() {
        let result = "<test>:swap".parse::<Operator>();
        assert!(result.is_err());
    }

    #[test]
    fn test_swap_mismatched_count() {
        let captures = vec![
            capture(r"first = (?<first>\w+)"),
            capture(r"second = (?<second>\w+)"),
        ];
        let operators = vec![operator("<first>:swap:<second>")];
        let content = "first = A\nfirst = B\nsecond = C".to_string();

        let result = regop(&captures, &operators, content);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("different number of matches")
        );
    }

    #[test]
    fn test_parse_int_success() {
        assert_eq!(parse_int("42").unwrap(), 42);
        assert_eq!(parse_int("-10").unwrap(), -10);
    }

    #[test]
    fn test_parse_int_failure() {
        assert!(parse_int("not_a_number").is_err());
    }

    #[test]
    fn test_distance_function() {
        assert_eq!(distance(0, 5, 10, 15), Some(5));
        assert_eq!(distance(10, 15, 0, 5), Some(5));
        assert_eq!(distance(0, 10, 5, 15), None); // Overlapping
        assert_eq!(distance(5, 15, 0, 10), None); // Overlapping
    }

    #[test]
    fn test_param_from_str() {
        // Test integer parsing
        let param = Param::from("42");
        matches!(param, Param::Int(42));

        // Test string parsing
        let param = Param::from("hello");
        matches!(param, Param::String(_));

        // Test capture parsing
        let param = Param::from("<capture>");
        matches!(param, Param::Capture(_));
    }

    #[test]
    fn test_negative_numbers() {
        let captures = vec![capture(r"value = (?<value>-?\d+)")];
        let operators = vec![operator("<value>:inc:5")];
        let content = "value = -10".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("value = -5".to_string()));
    }

    #[test]
    fn test_zero_operations() {
        let captures = vec![capture(r"value = (?<value>\d+)")];
        let operators = vec![operator("<value>:inc:0")];
        let content = "value = 5".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("value = 5".to_string()));
    }

    #[test]
    fn test_large_numbers() {
        let captures = vec![capture(r"value = (?<value>\d+)")];
        let operators = vec![operator("<value>:inc:1000000")];
        let content = "value = 999999".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("value = 1999999".to_string()));
    }

    #[test]
    fn test_empty_string_replacement() {
        let captures = vec![capture(r"text = (?<text>\w*)")];
        let operators = vec![operator("<text>:del")];
        let content = "text = hello".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("text = ".to_string()));
    }

    #[test]
    fn test_special_characters_in_replacement() {
        let captures = vec![capture(r"text = (?<text>\w+)")];
        let operators = vec![operator(r#"<text>:rep:hello@world.com"#)];
        let content = "text = old".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("text = hello@world.com".to_string()));
    }

    #[test]
    fn test_unicode_support() {
        let captures = vec![capture(r"name = (?<name>\w+)")];
        let operators = vec![operator("<name>:rep:josé")];
        let content = "name = john".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("name = josé".to_string()));
    }

    #[test]
    fn test_mixed_operations_order() {
        let captures = vec![capture(r"(?<a>\d+) (?<b>\d+) (?<c>\d+)")];
        let operators = vec![
            operator("<c>:inc:1"),
            operator("<a>:dec:1"),
            operator("<b>:rep:99"),
        ];
        let content = "5 10 15".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("4 99 16".to_string()));
    }

    #[test]
    fn test_capture_group_not_found() {
        let captures = vec![capture(r"version = (?<version>\d+)")];
        let operators = vec![operator("<nonexistent>:inc")];
        let content = "version = 5".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_multiple_regex_patterns() {
        let captures = vec![
            capture(r"version = (?<version>\d+)"),
            capture(r"count = (?<count>\d+)"),
        ];
        let operators = vec![operator("<version>:inc"), operator("<count>:dec")];
        let content = "version = 1\ncount = 10".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("version = 2\ncount = 9".to_string()));
    }

    #[test]
    fn test_overlapping_matches_error() {
        let captures = vec![capture(r"(?<all>\w+(?<part>\w+))")];
        let operators = vec![operator("<all>:rep:new"), operator("<part>:rep:part")];
        let content = "hello".to_string();

        let result = regop(&captures, &operators, content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("overlap"));
    }

    #[test]
    fn test_string_increment_with_capture() {
        let captures = vec![capture(r"(?<a>\d+) plus (?<b>\d+)")];
        let operators = vec![operator("<a>:inc:<b>")];
        let content = "5 plus 3".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("8 plus 3".to_string()));
    }

    #[test]
    fn test_dec_with_string_capture() {
        let captures = vec![capture(r"(?<a>\d+) minus (?<b>\d+)")];
        let operators = vec![operator("<a>:dec:<b>")];
        let content = "10 minus 3".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("7 minus 3".to_string()));
    }

    #[test]
    fn test_whitespace_handling() {
        let captures = vec![capture(r"value\s*=\s*(?<value>\d+)")];
        let operators = vec![operator("<value>:inc")];
        let content = "value   =   5".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("value   =   6".to_string()));
    }

    #[test]
    fn test_case_sensitive_regex() {
        let captures = vec![capture(r"Version = (?<version>\d+)")];
        let operators = vec![operator("<version>:inc")];
        let content = "version = 5".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_replace_with_space() {
        let captures = vec![capture(r"text = (?<text>\w+)")];
        let operators = vec![operator("<text>:rep: ")];
        let content = "text = hello".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("text =  ".to_string()));
    }

    #[test]
    fn test_mul_operation() {
        let captures = vec![capture(r"value = (?<value>\d+)")];
        let operators = vec![operator("<value>:mul:3")];
        let content = "value = 5".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("value = 15".to_string()));
    }

    #[test]
    fn test_mul_operation_with_capture() {
        let captures = vec![capture(r"(?<a>\d+) times (?<b>\d+)")];
        let operators = vec![operator("<a>:mul:<b>")];
        let content = "4 times 6".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("24 times 6".to_string()));
    }

    #[test]
    fn test_div_operation() {
        let captures = vec![capture(r"value = (?<value>\d+)")];
        let operators = vec![operator("<value>:div:2")];
        let content = "value = 10".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("value = 5".to_string()));
    }

    #[test]
    fn test_div_operation_with_capture() {
        let captures = vec![capture(r"(?<a>\d+) divided by (?<b>\d+)")];
        let operators = vec![operator("<a>:div:<b>")];
        let content = "20 divided by 4".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("5 divided by 4".to_string()));
    }

    #[test]
    fn test_div_by_zero_error() {
        let captures = vec![capture(r"value = (?<value>\d+)")];
        let operators = vec![operator("<value>:div:0")];
        let content = "value = 10".to_string();

        let result = regop(&captures, &operators, content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("division by zero"));
    }

    #[test]
    fn test_append_operation() {
        let captures = vec![capture(r"name = (?<name>\w+)")];
        let operators = vec![operator("<name>:append:_suffix")];
        let content = "name = test".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("name = test_suffix".to_string()));
    }

    #[test]
    fn test_append_operation_with_number() {
        let captures = vec![capture(r"version = (?<version>\d+)")];
        let operators = vec![operator("<version>:append:42")];
        let content = "version = 1".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("version = 142".to_string()));
    }

    #[test]
    fn test_prepend_operation() {
        let captures = vec![capture(r"name = (?<name>\w+)")];
        let operators = vec![operator("<name>:prepend:prefix_")];
        let content = "name = test".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("name = prefix_test".to_string()));
    }

    #[test]
    fn test_prepend_operation_with_number() {
        let captures = vec![capture(r"version = (?<version>\d+)")];
        let operators = vec![operator("<version>:prepend:v")];
        let content = "version = 123".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("version = v123".to_string()));
    }

    #[test]
    fn test_upper_operation() {
        let captures = vec![capture(r"text = (?<text>\w+)")];
        let operators = vec![operator("<text>:upper")];
        let content = "text = hello".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("text = HELLO".to_string()));
    }

    #[test]
    fn test_upper_operation_mixed_case() {
        let captures = vec![capture(r"name = (?<name>[A-Za-z]+)")];
        let operators = vec![operator("<name>:upper")];
        let content = "name = JohnDoe".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("name = JOHNDOE".to_string()));
    }

    #[test]
    fn test_lower_operation() {
        let captures = vec![capture(r"text = (?<text>\w+)")];
        let operators = vec![operator("<text>:lower")];
        let content = "text = HELLO".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("text = hello".to_string()));
    }

    #[test]
    fn test_lower_operation_mixed_case() {
        let captures = vec![capture(r"name = (?<name>[A-Za-z]+)")];
        let operators = vec![operator("<name>:lower")];
        let content = "name = JohnDoe".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("name = johndoe".to_string()));
    }

    #[test]
    fn test_multiple_new_operations() {
        let captures = vec![capture(r"(?<text>\w+) = (?<value>\d+)")];
        let operators = vec![operator("<text>:upper"), operator("<value>:mul:2")];
        let content = "count = 5".to_string();

        let result = regop(&captures, &operators, content).unwrap();
        assert_eq!(result, Some("COUNT = 10".to_string()));
    }

    #[test]
    fn test_missing_parameter_for_mul() {
        let result = "<test>:mul".parse::<Operator>();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("parameter required in 'mul' operator")
        );
    }

    #[test]
    fn test_missing_parameter_for_div() {
        let result = "<test>:div".parse::<Operator>();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("parameter required in 'div' operator")
        );
    }

    #[test]
    fn test_missing_parameter_for_append() {
        let result = "<test>:append".parse::<Operator>();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("parameter required in 'append' operator")
        );
    }

    #[test]
    fn test_missing_parameter_for_prepend() {
        let result = "<test>:prepend".parse::<Operator>();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("parameter required in 'prepend' operator")
        );
    }

    #[test]
    fn test_mul_overflow_protection() {
        let captures = vec![capture(r"value = (?<value>\d+)")];
        let operators = vec![operator("<value>:mul:1000000000000")];
        let content = "value = 1000000000000".to_string();

        // Should not panic due to wrapping_mul
        let result = regop(&captures, &operators, content).unwrap();
        assert!(result.is_some());
    }
}
