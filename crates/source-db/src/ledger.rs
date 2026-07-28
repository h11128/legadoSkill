use rusqlite::Connection;
use serde::Serialize;
use source_types::LedgerRow;

use crate::Result;

fn wire_str<T: Serialize>(v: &T) -> Result<String> {
    let raw = serde_json::to_string(v)?;
    Ok(raw.trim_matches('"').to_string())
}

pub fn append(conn: &Connection, row: &LedgerRow) -> Result<i64> {
    let row_json = serde_json::to_string(row)?;
    let capability = row.capability.as_ref().map(wire_str).transpose()?;
    let family = row.family.as_ref().map(|f| f.as_str().to_string());
    let layer = row.layer.as_ref().map(wire_str).transpose()?;
    let report_status = row.report_status.as_ref().map(wire_str).transpose()?;
    conn.execute(
        "INSERT INTO ledger_events(
           ts, source_key, step, result, note, waste,
           capability, family, layer, report_status, row_json
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        rusqlite::params![
            row.ts,
            row.url.as_str(),
            row.step.as_str(),
            row.result,
            row.note,
            row.waste,
            capability,
            family,
            layer,
            report_status,
            row_json,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}
