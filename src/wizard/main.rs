//! RFVision CLI and safe installation/assessment wizard.
#![forbid(unsafe_code)]
mod install;
mod recording;
mod ui;
use rfv_ipc::{record, Record, DISCOVER, SAMPLE};
use std::{io, path::Path, time::Duration};
fn request(socket: &str, kind: u16, interface: Option<&str>) -> io::Result<Vec<Record>> {
    let rows = interface
        .map(|s| vec![record(&[("interface", s)])])
        .unwrap_or_default();
    rfv_ipc::transport::call(Path::new(socket), kind, rows)
}
fn assessment(socket: Option<&str>) -> io::Result<Vec<Record>> {
    let mut records = rfv_hal::inventory(Path::new("/sys"))?;
    if let Some(s) = socket {
        records.extend(request(s, DISCOVER, None)?);
    }
    records.push(record(&[("kind","assessment"),("mode","read_only"),("driver_installation","never required by this version"),("physical_capabilities","not hardware validated; source expectations and driver advertisements are distinct"),("csi","unverified; acquisition not implemented"),("next_step","start userspace adapter/core and request sample; no driver replacement or firmware writes"),("without_service","adapter availability requires a service report; local discovery alone is not a driver capability assertion")]));
    Ok(records)
}
fn validate(socket: &str) -> io::Result<(Vec<Record>, bool)> {
    let before = rfv_hal::discover(Path::new("/sys"))?;
    let mut rows = request(socket, DISCOVER, None)?;
    let targets: Vec<_> = rows
        .iter()
        .filter(|r| {
            r.get("kind").map(String::as_str) == Some("device")
                && r.get("adapter_supported").map(String::as_str) == Some("true")
        })
        .filter_map(|r| r.get("interface").cloned())
        .collect();
    let mut sampled = 0;
    let mut failed = false;
    for name in targets {
        match request(socket, SAMPLE, Some(&name)) {
            Ok(sample) => {
                if sample
                    .iter()
                    .any(|r| r.get("kind").map(String::as_str) == Some("measurement"))
                {
                    sampled += 1;
                } else {
                    failed = true;
                }
                rows.extend(sample);
            }
            Err(e) => {
                failed = true;
                rows.push(rfv_hal::diagnostic(
                    &name,
                    "VALIDATION_ACQUISITION_FAILED",
                    &e.to_string(),
                ));
            }
        }
    }
    let after = rfv_hal::discover(Path::new("/sys"))?;
    let unchanged = before.len() == after.len()
        && before.iter().all(|b| {
            after.iter().any(|a| {
                a.identity == b.identity
                    && a.interface == b.interface
                    && a.driver == b.driver
                    && a.fields.get("carrier") == b.fields.get("carrier")
                    && a.fields.get("operstate") == b.fields.get("operstate")
            })
        });
    let complete = !before.is_empty() && sampled > 0 && !failed && unchanged;
    rows.push(record(&[
        ("kind", "validation"),
        (
            "result",
            if complete {
                "PASS_PASSIVE_CHECKS"
            } else {
                "INCOMPLETE_OR_FAILED"
            },
        ),
        ("ipc", "passed"),
        (
            "observed_netdev_state",
            if before.is_empty() {
                "NO_WIFI_DEVICE"
            } else if unchanged {
                "UNCHANGED"
            } else {
                "CHANGED"
            },
        ),
        ("interfaces_with_measurements", &sampled.to_string()),
        ("internet_connectivity", "NOT_TESTED; no active probes sent"),
        ("physical_rf_accuracy", "NOT_TESTED"),
        ("csi", "NOT_ACQUIRED"),
        ("driver_changes", "none"),
    ]));
    Ok((rows, complete))
}
const USAGE: &str = "RFVision 0.1 — passive Linux RF diagnostics\n\
  rfv discover\n\
  rfv budget INPUT.tsv\n\
  rfv range NODES.tsv\n\
  rfv csi analyze CAPTURE.tsv\n\
  rfv record CORE_SOCKET INTERFACE FILE COUNT INTERVAL_MS [REFERENCE LABEL X_M Y_M POSITION_SOURCE]\n\
  rfv replay FILE\n\
  rfv coverage FILE [FILE ...]\n\
  rfv ui CORE_SOCKET INTERFACE PORT (RFV_UI_TOKEN environment required)\n\
  rfv report /absolute/private/core.sock\n\
  rfv sample /absolute/private/core.sock INTERFACE\n\
  rfv watch /absolute/private/core.sock INTERFACE COUNT INTERVAL_MS\n\
  rfv wizard assess [CORE_SOCKET]\n\
  rfv wizard validate CORE_SOCKET\n\
  rfv wizard plan PREFIX BUILT_BINARY_DIRECTORY\n\
  rfv wizard install PREFIX BUILT_BINARY_DIRECTORY --apply\n\
  rfv wizard plan-rtw88 PREFIX BUILT_BINARY_DIRECTORY\n\
  rfv wizard install-rtw88 PREFIX BUILT_BINARY_DIRECTORY --apply\n\
  rfv wizard rollback PREFIX --apply\n\
Output: JSON array; watch: one array per line. Unknown data are omitted.\n\
No scan, monitor creation, transmission, firmware changes or system Wi-Fi driver replacement.";
fn run(args: &[String]) -> io::Result<i32> {
    let a: Vec<_> = args.iter().map(String::as_str).collect();
    let rows = match a.as_slice() {
        [] | ["--help"] | ["help"] => {
            println!("{USAGE}");
            return Ok(0);
        }
        ["budget", path] => rfv_core::link_budget::calculate(
            &rfv_core::link_budget::parse(&read_input(path)?).map_err(io::Error::other)?,
        )
        .map_err(io::Error::other)?,
        ["range", path] => rfv_core::range::assess(&read_input(path)?).map_err(io::Error::other)?,
        ["csi", "analyze", path] => rfv_core::csi_capture::Capture::parse(&read_input(path)?)
            .and_then(|c| c.analyze())
            .map_err(io::Error::other)?,
        ["record", socket, interface, file, count, delay] => {
            recording::record(
                socket,
                interface,
                file,
                count.parse().map_err(io::Error::other)?,
                delay.parse().map_err(io::Error::other)?,
                None,
            )?;
            return Ok(0);
        }
        ["record", socket, interface, file, count, delay, reference, label, x, y, source] => {
            let position = rfv_core::coverage::Position {
                reference: reference.to_string(),
                label: label.to_string(),
                x_m: x.parse().map_err(io::Error::other)?,
                y_m: y.parse().map_err(io::Error::other)?,
                source: source.to_string(),
            };
            recording::record(
                socket,
                interface,
                file,
                count.parse().map_err(io::Error::other)?,
                delay.parse().map_err(io::Error::other)?,
                Some(position),
            )?;
            return Ok(0);
        }
        ["replay", path] => {
            for batch in recording::replay(Path::new(path))? {
                println!("{}", rfv_ipc::json(&batch));
            }
            return Ok(0);
        }
        ["coverage", paths @ ..] if !paths.is_empty() => {
            if paths.len() > 128 {
                return Err(io::Error::other("at most 128 recordings"));
            }
            let mut batches = Vec::new();
            let mut total_bytes = 0u64;
            for path in paths {
                total_bytes = total_bytes.saturating_add(std::fs::metadata(path)?.len());
                if total_bytes > 64 * 1024 * 1024 {
                    return Err(io::Error::other("combined recording limit 64 MiB"));
                }
                let next = recording::replay(Path::new(path))?;
                if batches.len() + next.len() > 10000 {
                    return Err(io::Error::other("combined batch limit 10000"));
                }
                batches.extend(next);
            }
            recording::coverage(&batches)?
        }
        ["ui", socket, interface, port] => {
            ui::serve(socket, interface, port.parse().map_err(io::Error::other)?)?;
            return Ok(0);
        }
        ["discover"] => rfv_hal::inventory(Path::new("/sys"))?,
        ["report", socket] => request(socket, DISCOVER, None)?,
        ["sample", socket, interface] => request(socket, SAMPLE, Some(interface))?,
        ["watch", socket, interface, count, interval] => {
            let count: usize = count
                .parse()
                .map_err(|_| io::Error::other("invalid count"))?;
            let interval: u64 = interval
                .parse()
                .map_err(|_| io::Error::other("invalid interval"))?;
            if !(1..=10000).contains(&count) || !(1000..=3600000).contains(&interval) {
                return Err(io::Error::other(
                    "count 1..10000; delay 1000..3600000 ms between completed samples",
                ));
            }
            for i in 0..count {
                println!(
                    "{}",
                    rfv_ipc::json(&request(socket, SAMPLE, Some(interface))?)
                );
                if i + 1 < count {
                    std::thread::sleep(Duration::from_millis(interval));
                }
            }
            return Ok(0);
        }
        ["wizard", "assess"] => assessment(None)?,
        ["wizard", "assess", socket] => assessment(Some(socket))?,
        ["wizard", "validate", socket] => {
            let (rows, complete) = validate(socket)?;
            println!("{}", rfv_ipc::json(&rows));
            return Ok(if complete { 0 } else { 2 });
        }
        ["wizard", "plan", prefix, source] => install::plan(Path::new(prefix), Path::new(source)),
        ["wizard", "plan-rtw88", prefix, source] => {
            install::plan_rtw88(Path::new(prefix), Path::new(source))
        }
        ["wizard", "install", prefix, source, "--apply"] => {
            install::install(Path::new(prefix), Path::new(source))?
        }
        ["wizard", "install-rtw88", prefix, source, "--apply"] => {
            install::install_rtw88(Path::new(prefix), Path::new(source))?
        }
        ["wizard", "rollback", prefix, "--apply"] => install::rollback(Path::new(prefix))?,
        _ => return Err(io::Error::other(USAGE)),
    };
    println!("{}", rfv_ipc::json(&rows));
    Ok(0)
}
fn read_input(path: &str) -> io::Result<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(io::Error::other("input limit 4 MiB"));
    }
    String::from_utf8(bytes).map_err(io::Error::other)
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!(
                "{}",
                rfv_ipc::json(&[rfv_ipc::error("RFV_ERROR", &e.to_string())])
            );
            std::process::exit(1);
        }
    }
}
