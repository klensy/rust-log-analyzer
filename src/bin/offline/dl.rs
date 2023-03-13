use crate::offline;
use crate::rla;
use crate::rla::ci::{self, CiPlatform};
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::Path;

use reqwest::blocking::Client as ReqwestClient;

const LOG_DL_MAX_ATTEMPTS: u32 = 3;

pub fn cat(input: &Path, strip_control: bool, decode_utf8: bool) -> rla::Result<()> {
    let mut data = offline::fs::load_maybe_compressed(input)?;

    if strip_control {
        data.retain(|&b| b == b'\n' || !b.is_ascii_control());
    }

    if decode_utf8 {
        let stdout = io::stdout();
        stdout
            .lock()
            .write_all(String::from_utf8_lossy(&data).as_bytes())?;
    } else {
        let stdout = io::stdout();
        stdout.lock().write_all(&data)?;
    }

    Ok(())
}

pub fn download(
    ci: &dyn CiPlatform,
    repo: &str,
    output: &Path,
    count: u32,
    offset: u32,
    filter_branches: &[String],
    only_passed: bool,
    only_failed: bool,
) -> rla::Result<()> {
    let client = ReqwestClient::new();
    let filter_branches = filter_branches
        .iter()
        .map(|s| s.as_str())
        .collect::<HashSet<_>>();

    let check_outcome = |outcome: &dyn rla::ci::Outcome| {
        (!only_passed || outcome.is_passed()) && (!only_failed || outcome.is_failed())
    };
    let builds = ci.query_builds(repo, count, offset, &|build| {
        (filter_branches.is_empty() || filter_branches.contains(build.branch_name()))
            && check_outcome(build.outcome())
    })?;

    let compression_pool = threadpool::Builder::new().build();

    'job_loop: for job in builds.iter().flat_map(|b| b.jobs()) {
        if !check_outcome(job.outcome()) {
            continue;
        }

        let save_path = output.join(offline::fs::encode_path(&format!(
            "{}.log.brotli",
            job.log_file_name()
        )));
        if save_path.is_file() {
            warn!("Skipping log for {} because the output file exists.", job);
            continue;
        }

        let data;
        let mut attempt = 0;

        loop {
            attempt += 1;
            info!(
                "Downloading log for {} [Attempt {}/{}]...",
                job, attempt, LOG_DL_MAX_ATTEMPTS
            );

            match ci::download_log(ci, job, &client) {
                Some(Ok(d)) => {
                    data = d;
                    break;
                }
                None => {
                    warn!("no log for {}", job);
                }
                Some(Err(e)) => {
                    if attempt >= LOG_DL_MAX_ATTEMPTS {
                        warn!("Failed to download log, skipping: {}", e);
                        continue 'job_loop;
                    }
                }
            }
        }

        debug!("Compressing...");

        // When the pool is busy this compresses on the main thread, to avoid using a huge amount
        // of RAM to store all the queued logs.
        if compression_pool.active_count() >= compression_pool.max_count() {
            debug!("compression pool is busy, compressing on the main thread...");
            offline::fs::save_compressed(&save_path, &data)?;
        } else {
            debug!("compressing on the pool...");
            compression_pool.execute(move || {
                crate::util::log_and_exit_error(move || {
                    offline::fs::save_compressed(&save_path, &data)
                });
            });
        }
    }

    Ok(())
}
