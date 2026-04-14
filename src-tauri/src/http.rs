use crate::utils::{content_type_header, summarize_body};
use cynic::GraphQlResponse;
use std::collections::VecDeque;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const MAX_REQUESTS_PER_WINDOW: usize = 80;
const REQUEST_WINDOW: Duration = Duration::from_secs(60);

fn request_timestamps() -> &'static Mutex<VecDeque<Instant>> {
    static REQUEST_TIMESTAMPS: OnceLock<Mutex<VecDeque<Instant>>> = OnceLock::new();
    REQUEST_TIMESTAMPS.get_or_init(|| Mutex::new(VecDeque::new()))
}

async fn wait_for_request_slot() {
    loop {
        let now = Instant::now();
        let mut timestamps = request_timestamps().lock().await;

        while let Some(&oldest) = timestamps.front() {
            if now.duration_since(oldest) >= REQUEST_WINDOW {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        if timestamps.len() < MAX_REQUESTS_PER_WINDOW {
            timestamps.push_back(now);
            return;
        }

        let sleep_for = timestamps
            .front()
            .map(|oldest| REQUEST_WINDOW.saturating_sub(now.duration_since(*oldest)))
            .unwrap_or(REQUEST_WINDOW);

        drop(timestamps);
        tokio::time::sleep(sleep_for).await;
    }
}

pub async fn execute_graphql_operation<T: serde::de::DeserializeOwned, V: serde::Serialize>(
    http_client: &reqwest::Client,
    auth_token: &str,
    operation: cynic::Operation<T, V>,
    tournament_slug: &str,
    query_type: &str,
) -> Result<(GraphQlResponse<T>, Duration), String> {
    const MAX_RETRIES: u32 = 6;
    let mut retry_count = 0;
    let start_time = Instant::now();

    loop {
        wait_for_request_slot().await;

        let response = http_client
            .post("https://api.start.gg/gql/alpha")
            .bearer_auth(auth_token)
            .json(&operation)
            .send()
            .await
            .map_err(|e| {
                format!(
                    "HTTP request failed for {} [{}]: {}",
                    tournament_slug, query_type, e
                )
            })?;

        let status = response.status();
        let content_type = content_type_header(&response);

        let body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        match status.as_u16() {
            429 => {
                // Rate limited - retry with exponential backoff
                if retry_count < MAX_RETRIES {
                    let backoff_ms = 200u64 * 4u64.pow(retry_count);
                    eprintln!(
                        "Rate limited (429) for {} [{}], retrying in {}ms (attempt {}/{})",
                        tournament_slug,
                        query_type,
                        backoff_ms,
                        retry_count + 1,
                        MAX_RETRIES
                    );
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    retry_count += 1;
                    continue;
                } else {
                    return Err(format!(
                        "Rate limited (429) for {} [{}] after {} retries",
                        tournament_slug, query_type, MAX_RETRIES
                    ));
                }
            }
            502 | 503 => {
                if retry_count < MAX_RETRIES {
                    let backoff_ms = 250u64 * 2u64.pow(retry_count);
                    eprintln!(
                        "Transient HTTP {} for {} [{}], retrying in {}ms (attempt {}/{})",
                        status.as_u16(),
                        tournament_slug,
                        query_type,
                        backoff_ms,
                        retry_count + 1,
                        MAX_RETRIES
                    );
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    retry_count += 1;
                    continue;
                }

                return Err(format!(
                    "HTTP {} for {} [{}] after {} retries (content-type: {}). Body: {}",
                    status.as_u16(),
                    tournament_slug,
                    query_type,
                    MAX_RETRIES,
                    content_type,
                    summarize_body(&body, 300)
                ));
            }
            504 => {
                // 504 is treated as a payload-size signal by query orchestration,
                // so return immediately and let the caller reduce per_page.
                return Err(format!(
                    "HTTP 504 for {} [{}] (content-type: {}). Body: {}",
                    tournament_slug,
                    query_type,
                    content_type,
                    summarize_body(&body, 300)
                ));
            }
            _ => {
                if !status.is_success() {
                    return Err(format!(
                        "HTTP {} for {} [{}] (content-type: {}). Body: {}",
                        status.as_u16(),
                        tournament_slug,
                        query_type,
                        content_type,
                        summarize_body(&body, 300)
                    ));
                }

                let normalized_body = body.trim_start_matches('\u{feff}').trim();

                if normalized_body.is_empty() {
                    if retry_count < MAX_RETRIES {
                        let backoff_ms = 250u64 * 2u64.pow(retry_count);
                        eprintln!(
                            "Empty response body for {} [{}], retrying in {}ms (attempt {}/{})",
                            tournament_slug,
                            query_type,
                            backoff_ms,
                            retry_count + 1,
                            MAX_RETRIES
                        );
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        retry_count += 1;
                        continue;
                    }

                    return Err(format!(
                        "Empty response body for {} [{}] after {} retries (content-type: {})",
                        tournament_slug, query_type, MAX_RETRIES, content_type
                    ));
                }

                let graphql_response: GraphQlResponse<T> =
                    serde_json::from_str(normalized_body).map_err(|e| {
                        format!(
                            "Failed to parse GraphQL response for {} [{}]: {} (HTTP {}, content-type: {}). Body: {}",
                            tournament_slug,
                            query_type,
                            e,
                            status.as_u16(),
                            content_type,
                            summarize_body(normalized_body, 300)
                        )
                    })?;

                return Ok((graphql_response, start_time.elapsed()));
            }
        }
    }
}
