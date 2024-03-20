use crate::offline;

use rla::index::IndexStorage;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use walkdir::WalkDir;

struct Line<'a> {
    _original: &'a [u8],
    sanitized: Vec<u8>,
}

impl<'a> rla::index::IndexData for Line<'a> {
    fn sanitized(&self) -> &[u8] {
        &self.sanitized
    }
}

fn load_lines<'a>(ci: &dyn rla::ci::CiPlatform, log: &'a [u8]) -> Vec<Line<'a>> {
    rla::sanitize::split_lines(log)
        .iter()
        .map(|&line| Line {
            _original: line,
            sanitized: rla::sanitize::clean(ci, line),
        })
        .collect()
}

pub fn dir(
    ci: &dyn rla::ci::CiPlatform,
    index_file: &IndexStorage,
    src_dir: &Path,
    dst_dir: &Path,
) -> rla::Result<()> {
    let config = rla::extract::Config::default();
    let index = rla::Index::load(index_file)?;

    for entry in walk_non_hidden_children(dst_dir) {
        let entry = entry?;

        if entry.file_type().is_dir() {
            continue;
        }

        fs::remove_file(entry.path())?;
    }

    let progress_every = Duration::from_secs(1);
    let mut last_print = Instant::now();

    for (count, entry) in walk_non_hidden_children(&src_dir).enumerate() {
        let entry = entry?;

        if entry.file_type().is_dir() {
            continue;
        }

        let now = Instant::now();

        if now - last_print >= progress_every {
            last_print = now;
            debug!(
                "Extracting errors from {} [{}/?]...",
                entry.path().display(),
                count
            );
        } else {
            trace!(
                "Extracting errors from {} [{}/?]...",
                entry.path().display(),
                count
            );
        }

        let log = offline::fs::load_maybe_compressed(entry.path())?;
        let lines = load_lines(ci, &log);
        let blocks = rla::extract::extract(&config, &index, &lines);

        let mut out_name = entry.file_name().to_owned();
        out_name.push(".err");

        write_blocks_to(
            io::BufWriter::new(fs::File::create(dst_dir.join(out_name))?),
            &blocks,
        )?;
    }

    Ok(())
}

pub fn one(
    ci: &dyn rla::ci::CiPlatform,
    index_file: &IndexStorage,
    log_file: &Path,
) -> rla::Result<()> {
    let config = rla::extract::Config::default();
    let index = rla::Index::load(index_file)?;

    let log = offline::fs::load_maybe_compressed(log_file)?;
    let lines = load_lines(ci, &log);
    let blocks = rla::extract::extract(&config, &index, &lines);

    let stdout = io::stdout();
    write_blocks_to(stdout.lock(), &blocks)?;

    Ok(())
}

fn write_blocks_to<W: Write>(mut w: W, blocks: &[Vec<&Line>]) -> rla::Result<()> {
    let mut first = true;

    for block in blocks {
        if !first {
            writeln!(w, "---")?;
        }
        first = false;

        for &line in block {
            w.write_all(&line.sanitized)?;
            w.write_all(b"\n")?;
        }
    }

    Ok(())
}

fn walk_non_hidden_children(
    root: &Path,
) -> Box<dyn Iterator<Item = walkdir::Result<walkdir::DirEntry>>> {
    fn not_hidden(entry: &walkdir::DirEntry) -> bool {
        !entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    }

    Box::new(
        WalkDir::new(root)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_entry(not_hidden),
    )
}
