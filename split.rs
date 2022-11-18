use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    time::Duration,
};

use clap::Parser;
use color_eyre::{eyre::Context, owo_colors::OwoColorize, Result as Res};
use indicatif::{ProgressBar, ProgressStyle};
use zstd::Encoder;

/// Splits MLIR pass pipeline logs.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about)]
struct Args {
    /// Output directory to place the individual MLIR files in.
    #[clap(default_value = "dump")]
    output_directory: PathBuf,

    /// Disables compressing with `zstd`.
    #[clap(short, long, default_value_t = false)]
    no_compress: bool,

    /// `zstd` compression level to use.
    #[clap(short, long, default_value_t = 6)]
    zstd_compression_level: i32,

    /// Number of threads `zstd` should use during compression.
    ///
    /// Defaults to 1 (separate I/O and compression thread).
    #[clap(short, long, default_value_t = 1)]
    threads: u32,

    /// Delete `output_directory` if it already exists.
    #[clap(short, long, default_value_t = false)]
    delete: bool,
}

fn progress_style() -> ProgressStyle {
    let template = format!(
        "{} {{msg}} {{pos}} {{bytes}} {{elapsed}}",
        "{spinner}".cyan()
    );
    ProgressStyle::with_template(&*template).unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogKind {
    Before,
    After,
    Unknown,
}

impl LogKind {
    fn short(&self) -> &'static str {
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

pub fn process_log_stream<W: Write>(
    mut inp: impl BufRead,
    mut func: impl FnMut(LogInfo) -> Res<W>,
    initial_output: W,
) -> Res<()> {
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

fn main() -> Res<()> {
    color_eyre::install()?;

    let Args {
        output_directory,
        no_compress,
        zstd_compression_level,
        threads,
        delete,
    } = Args::parse();

    if fs::read_dir(&output_directory)
        .map(|d| d.count())
        .unwrap_or(0)
        != 0
    {
        eprintln!(
            "{}: output directory `{}` is not empty!",
            "warning".yellow().bold(),
            output_directory.display()
        );

        if delete {
            eprintln!("{}: clearing...", "warning".yellow().bold());
            fs::remove_dir_all(&output_directory)?;
        }
    }
    fs::create_dir_all(&output_directory).wrap_err_with(|| {
        format!(
            "Unable to create output directory `{}`",
            output_directory.display()
        )
    })?;

    let new_file = |num: usize, name: &str, ext: &str| -> Res<_> {
        let p = output_directory.join(format!(
            "{num:04}-{name}.{ext}{}",
            if no_compress { "" } else { ".zst" }
        ));
        let f = File::create(p)?;
        let w = BufWriter::new(f);

        Ok(w)
    };
    let new_file_zstd = |num: usize, name: &str, ext: &str| {
        let w = new_file(num, name, ext)?;

        let mut e = Encoder::new(w, zstd_compression_level)?;

        e.multithread(threads)?;

        Ok(e.auto_finish())
    };

    let p = ProgressBar::new_spinner();
    p.enable_steady_tick(Duration::from_millis(100));
    p.set_message("Waiting for input on stdin...");

    fn func<'p, W: Write>(
        p: &'p ProgressBar,
        new_file_func: impl Fn(usize, &str, &str) -> Res<W> + 'p,
    ) -> impl FnMut(LogInfo) -> Res<W> + 'p {
        let mut last_pass_info: Option<LogInfo> = None;

        move |pass_info: LogInfo| {
            let skip_increment = if let Some(ref last_pass) = last_pass_info {
                let same_pass_name = last_pass.pass_name == pass_info.pass_name
                    && last_pass.pass_cmdline_opt_and_extras == pass_info.pass_cmdline_opt_and_extras;
                same_pass_name
                    && last_pass.kind == LogKind::Before
                    && pass_info.kind == LogKind::After
            } else {
                p.set_style(progress_style());
                p.set_position(0);

                false
            };
            last_pass_info = Some(pass_info.clone());

            if !skip_increment {
                p.inc(1);
            }
            let num = p.position() as usize;

            // TODO: set p.message
            p.set_message(pass_info.pass_name.clone());

            new_file_func(
                num,
                &*format!("{}-{}", pass_info.kind.short(), pass_info.pass_name),
                "mlir",
            )
        }
    }

    let stdin = io::stdin();
    let stdin = stdin.lock();
    let inp = BufReader::new(stdin);

    if no_compress {
        let prelude = new_file(0, "prelude", "txt")?;
        process_log_stream(inp, func(&p, new_file), prelude)?;
    } else {
        let prelude = new_file_zstd(0, "prelude", "txt")?;
        process_log_stream(inp, func(&p, new_file_zstd), prelude)?;
    };

    p.set_style(ProgressStyle::default_spinner());
    p.finish_with_message(format!(
        "Processed output from {} passes.",
        p.position().bold()
    ));

    Ok(())

    // p.inc(1);
    // p.inc(10000);
    // p.inc(1);
    // p.inc(1);

    // thread::sleep(Duration::from_millis(2000));
    // p.set_length(20000);
    // p.set_prefix("df");
    // thread::sleep(Duration::from_millis(2000));
    // drop(p);
    // let p = ProgressBar::new(34);
    // p.enable_steady_tick(Duration::from_millis(100));
    // thread::sleep(Duration::from_millis(200));
    // p.inc(20);
    // p.set_prefix("df");
    // p.set_message("Waiting for input on stdin...");

    // thread::sleep(Duration::from_millis(20000));

    // Ok(())
}
