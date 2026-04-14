use crate::queries;

pub fn format_event_date(timestamp_secs: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(timestamp_secs, 0).map(|dt| dt.format("%Y-%m-%d").to_string())
}

pub fn split_prefix_and_name(entrant_name: &str) -> (String, String) {
    let split_name: Vec<&str> = entrant_name.split('|').collect();
    if split_name.len() == 1 {
        (String::new(), entrant_name.trim().to_string())
    } else {
        let prefix = split_name[..split_name.len() - 1]
            .iter()
            .map(|part| part.trim())
            .collect::<Vec<_>>()
            .join("|");
        let name = split_name[split_name.len() - 1].trim().to_string();
        (prefix, name)
    }
}

pub fn id_to_string(id: &queries::scalars::StartggId) -> String {
    id.as_string().to_string()
}

pub fn winner_id_to_string(winner_id: i32) -> String {
    winner_id.to_string()
}

pub fn summarize_body(body: &str, max_len: usize) -> String {
    if body.len() <= max_len {
        body.to_string()
    } else {
        format!("{}...", &body[..max_len])
    }
}

pub fn content_type_header(response: &reqwest::Response) -> String {
    response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string()
}

pub fn format_page_counter(page: i32, total_pages: Option<i32>) -> String {
    match total_pages {
        Some(total) => format!("page {}/{}", page, total),
        None => format!("page {}", page),
    }
}
