use core::fmt;
use std::{
    fmt::Display,
    fs::{self, File},
    iter::Peekable,
    path::PathBuf,
};

use clap::Parser;
use color_eyre::{eyre::Context, owo_colors::OwoColorize, Result as Res};
use mlir_pipeline_utils::LogKind;

/// Splits MLIR pass pipeline logs.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about)]
struct Args {
    /// Directory containing the IR dumps to process.
    #[clap(default_value = "dump")]
    dump_directory: PathBuf,
}

#[derive(Debug)]
struct Pipeline {
    passes: Vec<Pass>,
}

#[derive(Debug)]
enum Pass {
    Single(SinglePass),
    Nested {
        parent: SinglePass,
        pipeline: Pipeline,
    },
}

#[derive(Debug)]
struct SinglePass {
    pass_name: String,
    // extra_info: String,
    before: File,
    after: File,
}

impl Pipeline {
    fn display(&self) -> impl Display + '_ {
        struct PipelineDisplayHelper<'i, I> {
            inner: &'i I,
            depth: usize,
        }

        impl<'i, I> PipelineDisplayHelper<'i, I> {
            fn prefix(&self, last_in_level: bool, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                let suffix = if last_in_level { "┖─ " } else { "┠─ " };
                write!(fmt, "  ")?;

                for _ in 0..self.depth {
                    write!(fmt, "┃  ")?;
                }
                write!(fmt, "{suffix}")
            }

            fn within<'t, T>(&self, inner: &'t T) -> PipelineDisplayHelper<'t, T> {
                PipelineDisplayHelper {
                    inner,
                    depth: self.depth,
                }
            }

            fn nested<'t, T>(&self, inner: &'t T) -> PipelineDisplayHelper<'t, T> {
                PipelineDisplayHelper {
                    inner,
                    depth: self.depth + 1,
                }
            }
        }

        impl<'i> Display for PipelineDisplayHelper<'i, Pipeline> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for (idx, i) in self.inner.passes.iter().enumerate() {
                    self.prefix(idx == (self.inner.passes.len() - 1), f)?;
                    write!(f, "{}", self.within(i))?;
                }

                Ok(())
            }
        }

        impl<'i> Display for PipelineDisplayHelper<'i, Pass> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.inner {
                    Pass::Nested { parent: s, .. } | Pass::Single(s) => {
                        write!(f, "{}", s.pass_name)?;
                        if f.alternate() {
                            write!(f, "({:?}, {:?})", s.before, s.after)?;
                        }

                        writeln!(f, "")?;
                    }
                }

                if let Pass::Nested { pipeline, .. } = self.inner {
                    write!(f, "{}", self.nested(pipeline))?;
                }

                Ok(())
            }
        }

        PipelineDisplayHelper {
            depth: 0,
            inner: self,
        }
    }
}

fn infer_pass_pipeline(mut pass_files: Vec<PathBuf>) -> Res<Pipeline> {
    pass_files.sort();

    let mut iter = pass_files
        .iter()
        .map(|path| {
            let filename = path.file_name().unwrap().to_str().unwrap();
            let [_num, kind, pass_name]: [&str; 3] =
                filename.split("-").collect::<Vec<_>>().try_into().unwrap();

            let kind = LogKind::from_short(kind).unwrap();
            (
                kind,
                pass_name.split_once('.').unwrap().0.to_string(),
                path.clone(),
            )
        })
        .peekable();

    fn read_pass(
        looking_for: &str,
        before: PathBuf,
        iterator: &mut Peekable<impl Iterator<Item = (LogKind, String, PathBuf)>>,
    ) -> Pass {
        let pipeline = read_pipeline(iterator);

        let (kind, pass_name, path) = iterator.next().unwrap_or_else(|| {
            panic!("ran out of pass logs without getting the log after `{looking_for}");
        });
        assert_eq!(kind, LogKind::After);
        assert_eq!(pass_name, looking_for, "got a log for after pass `{pass_name}`, expecting a log for after pass `{looking_for}`");

        let this = SinglePass {
            pass_name,
            before: File::open(before).unwrap(),
            after: File::open(path).unwrap(),
        };
        if pipeline.passes.is_empty() {
            Pass::Single(this)
        } else {
            Pass::Nested {
                parent: this,
                pipeline,
            }
        }
    }

    fn read_pipeline(
        iterator: &mut Peekable<impl Iterator<Item = (LogKind, String, PathBuf)>>,
    ) -> Pipeline {
        // keep reading pairs until we either run out or hit a `After`
        let mut passes = vec![];
        while let Some((kind, _, _)) = iterator.peek() {
            use LogKind::*;
            match kind {
                After => break,
                Before => {
                    let (_, looking_for, before) = iterator.next().unwrap();
                    passes.push(read_pass(&*looking_for, before, iterator));
                }
                Unknown => panic!(),
            }
        }

        Pipeline { passes }
    }

    Ok(read_pipeline(&mut iter))
}

fn main() -> Res<()> {
    color_eyre::install()?;
    let Args { dump_directory } = Args::parse();

    let files = fs::read_dir(&dump_directory)
        .wrap_err_with(|| {
            format!(
                "Unable to read dump directory `{}`",
                dump_directory.display()
            )
        })?
        .map(|e| e.unwrap())
        .filter(|e| e.file_type().unwrap().is_file())
        .map(|e| e.path())
        .filter(|e| {
            let filename = e.file_name().unwrap().to_str().unwrap();
            if !(filename.ends_with(".mlir") || filename.ends_with(".mlir.zst")) {
                eprintln!(
                    "{}: skipping file `{}`",
                    "warning".yellow().bold(),
                    filename.bold(),
                );

                false
            } else {
                true
            }
        })
        .collect();

    let pipeline = infer_pass_pipeline(files)?;
    println!("Pipeline:\n{}", pipeline.display());

    Ok(())
}
