//! Core service: deliberately has no dependency on any driver adapter or HAL.
#![forbid(unsafe_code)]
use std::path::{Path, PathBuf};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = if args.len() == 4 && args[1] == "serve" && args[2] != args[3] {
        let adapter = PathBuf::from(&args[3]);
        rfv_ipc::transport::serve(
            Path::new(&args[2]),
            "rfvision-core",
            move |kind, records| {
                let rows = rfv_ipc::transport::call(&adapter, kind, records.to_vec())
                    .map_err(|e| format!("ADAPTER_UNAVAILABLE: {e}"))?;
                rfv_core::process(rows)
            },
        )
    } else {
        Err(std::io::Error::other("usage: rfvd serve /absolute/private/core.sock /absolute/private/adapter.sock (distinct paths)"))
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
