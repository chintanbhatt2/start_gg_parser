use crate::types::TournamentRow;

pub fn rows_to_csv(rows: &[TournamentRow]) -> Result<String, String> {
    let mut writer = csv::Writer::from_writer(Vec::new());

    for row in rows {
        writer
            .serialize(row)
            .map_err(|e| format!("Failed to write CSV row: {}", e))?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| format!("Failed to finalize CSV output: {}", e))?;

    String::from_utf8(bytes).map_err(|e| format!("CSV output is not valid UTF-8: {}", e))
}
