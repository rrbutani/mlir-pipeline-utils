use std::io::{self, BufRead, Write};

use color_eyre::owo_colors::OwoColorize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogKind {
    Before,
    After,
    Unknown,
}

impl LogKind {
    pub fn short(&self) -> &'static str {
        use LogKind::*;
        match self {
            // just so that lexicographic order is correct
            Before => "0b",
            After => "1a",
            Unknown => "2u",
        }
    }
}

impl<'s> From<&'s str> for LogKind {
    fn from(s: &'s str) -> Self {
        match s {
            "Before" => Self::Before,
            "After" => Self::After,
            other => {
                eprintln!(
                    "{}: unknown kind `{}`",
                    "warning".yellow().bold(),
                    other.bold()
                );
                Self::Unknown
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LogInfo {
    pub pass_name: String,
    pub pass_cmdline_opt_and_extras: String,
    pub kind: LogKind,
}

// i.e. "// -----// IR Dump After LinalgNamedOpConversion (linalg-named-op-conversion) //----- //"
const PASS_TITLE_START: &str = "// -----// IR Dump ";
const PASS_TITLE_END: &str = " //----- //\n";

pub fn process_log_stream<W: Write, E: From<io::Error>>(
    mut inp: impl BufRead,
    mut func: impl FnMut(LogInfo) -> Result<W, E>,
    initial_output: W,
) -> Result<(), E> {
    let mut output = initial_output;

    let mut line = String::new();
    loop {
        line.clear();

        if inp.read_line(&mut line)? == 0 {
            break;
        }

        if let Some(pass_info) = line
            .strip_prefix(PASS_TITLE_START)
            .and_then(|l| l.strip_suffix(PASS_TITLE_END))
        {
            if let Some((kind, (pass_name, extras))) = pass_info
                .trim()
                .split_once(' ')
                .and_then(|(k, r)| Some((k, r.split_once(' ')?)))
            {
                let info = LogInfo {
                    pass_name: pass_name.to_string(),
                    pass_cmdline_opt_and_extras: extras.to_string(),
                    kind: kind.into(),
                };
                output = func(info)?;
            } else {
                eprintln!(
                    "{}: unable to parse line: `{}`",
                    "warning".yellow().bold(),
                    line.bold()
                );
            }
        }

        output.write_all(line.as_bytes())?;
    }

    Ok(())
}
