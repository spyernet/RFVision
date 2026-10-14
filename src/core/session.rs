//! Bounded session state. A missing sample never produces an interpolated value.
use rfv_ipc::Record;
#[derive(Default)]
pub struct Session {
    previous: std::collections::BTreeMap<String, Vec<Record>>,
    pub history: crate::coverage::History,
}
impl Session {
    pub fn ingest(&mut self, rows: Vec<Record>) -> Result<Vec<Record>, String> {
        let mut rows = crate::process(rows)?;
        let interfaces: Vec<_> = rows
            .iter()
            .filter(|r| r.get("kind").map(String::as_str) == Some("sample"))
            .filter_map(|r| r.get("interface").cloned())
            .collect();
        if interfaces.len() > 16 {
            return Err("session interface limit".into());
        }
        if interfaces.is_empty() {
            return Ok(rows);
        }
        rows.retain(|r| r.get("kind").map(String::as_str) != Some("recommendation"));
        for interface in interfaces {
            let subset: Vec<_> = rows
                .iter()
                .filter(|r| {
                    r.get("interface") == Some(&interface)
                        && r.get("kind").map(String::as_str) == Some("measurement")
                })
                .cloned()
                .collect();
            let recommendations =
                crate::optimizer::assess(&subset, self.previous.get(&interface).map(Vec::as_slice));
            if self.previous.len() >= 16 && !self.previous.contains_key(&interface) {
                self.previous.clear();
            }
            self.previous.insert(interface, subset);
            rows.extend(recommendations);
        }
        for observation in crate::coverage::from_batch(&rows, None) {
            // Synthetic algorithm fixtures are never promoted into live coverage.
            if let Err(e) = self.history.push(observation) {
                rows.push(rfv_ipc::record(&[
                    ("kind", "diagnostic"),
                    ("code", "COVERAGE_REJECTED"),
                    ("message", &e),
                ]));
            }
        }
        Ok(rows)
    }
}
