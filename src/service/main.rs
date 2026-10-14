//! Core service: deliberately has no dependency on any driver adapter or HAL.
#![forbid(unsafe_code)]
use std::path::{Path, PathBuf};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = if args.len() == 4 && args[1] == "serve" && args[2] != args[3] {
        let adapter = PathBuf::from(&args[3]);
        let session = std::sync::Mutex::new(rfv_core::session::Session::default());
        rfv_ipc::transport::serve(
            Path::new(&args[2]),
            "rfvision-core",
            move |kind, records| {
                let mut session = session.lock().map_err(|_| "session lock poisoned")?;
                let rows = rfv_ipc::transport::call(&adapter, kind, records.to_vec())
                    .map_err(|e| format!("ADAPTER_UNAVAILABLE: {e}"))?;
                session.ingest(rows)
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
