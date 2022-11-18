use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Write},
    path::PathBuf,
    time::Duration,
};

use clap::Parser;
use color_eyre::{eyre::Context, owo_colors::OwoColorize, Result as Res};
use indicatif::{ProgressBar, ProgressStyle};
use mlir_pipeline_utils::{process_log_stream, LogInfo, LogKind};
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
        "{} Pass #{{pos}}: {{msg}} ({{elapsed}} elapsed)",
        "{spinner}".cyan()
    );
    ProgressStyle::with_template(&*template).unwrap()
}

fn gen_pass_log_process_func<'p, W: Write>(
    p: &'p ProgressBar,
    new_file_func: impl Fn(usize, &str, &str) -> Res<W> + 'p,
) -> impl FnMut(LogInfo) -> Res<W> + 'p {
    let mut last_pass_info: Option<LogInfo> = None;

    move |pass_info: LogInfo| {
        let skip_increment = if let Some(ref last_pass) = last_pass_info {
            let same_pass_name = last_pass.pass_name == pass_info.pass_name
                && last_pass.pass_cmdline_opt_and_extras == pass_info.pass_cmdline_opt_and_extras;
            same_pass_name && last_pass.kind == LogKind::Before && pass_info.kind == LogKind::After
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

    let stdin = io::stdin();
    let stdin = stdin.lock();
    let inp = BufReader::new(stdin);

    if no_compress {
        let prelude = new_file(0, "prelude", "txt")?;
        process_log_stream(inp, gen_pass_log_process_func(&p, new_file), prelude)?;
    } else {
        let prelude = new_file_zstd(0, "prelude", "txt")?;
        process_log_stream(inp, gen_pass_log_process_func(&p, new_file_zstd), prelude)?;
    };

    p.set_style(ProgressStyle::default_spinner());
    p.finish_with_message(format!(
        "Processed output from {} passes.",
        p.position().bold()
    ));

    Ok(())
}
