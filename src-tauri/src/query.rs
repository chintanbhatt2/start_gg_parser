use crate::http::execute_graphql_operation;
use crate::queries;
use crate::types::{DownloadProgressEvent, EventAccumulator, TournamentRow};
use crate::utils::{format_event_date, format_page_counter, id_to_string};
use cynic::{GraphQlResponse, QueryBuilder};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct SetsPageResponse {
    sets: GraphQlResponse<queries::tournament::TournamentSetsQuery>,
}

pub struct ProcessSetsPageContext<'a> {
    tournament_slug: &'a str,
    event_by_id: &'a mut HashMap<String, EventAccumulator>,
    known_total_pages: &'a mut Option<i32>,
}

fn is_page_size_too_high_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("your query complexity is too high") || normalized.contains("http 504")
}

pub async fn fetch_tournament_metadata(
    http_client: &reqwest::Client,
    auth_token: &str,
    tournament_slug: &str,
) -> Result<
    (
        Option<String>,
        Vec<Option<queries::tournament::MetadataEvent>>,
    ),
    String,
> {
    let metadata_operation = queries::tournament::TournamentMetadataQuery::build(
        queries::tournament::TournamentMetadataVariables {
            tourney_slug: Some(tournament_slug),
        },
    );

    let (metadata_response, _) = execute_graphql_operation::<
        queries::tournament::TournamentMetadataQuery,
        queries::tournament::TournamentMetadataVariables,
    >(
        http_client,
        auth_token,
        metadata_operation,
        tournament_slug,
        "metadata",
    )
    .await?;

    if let Some(errors) = metadata_response.errors {
        return Err(format!(
            "GraphQL errors for {} [metadata]: {:?}",
            tournament_slug, errors
        ));
    }

    let Some(metadata_data) = metadata_response.data else {
        return Err(format!(
            "Missing GraphQL data for {} [metadata]",
            tournament_slug
        ));
    };

    let Some(metadata_tournament) = metadata_data.tournament else {
        return Err(format!(
            "Tournament not found for slug '{}'. Use a tournament slug like tournament/my-event.",
            tournament_slug
        ));
    };

    let tournament_id = metadata_tournament.id.as_ref().map(id_to_string);
    let metadata_events = metadata_tournament.events.unwrap_or_default();

    if metadata_events.is_empty() {
        return Err(format!(
            "No events found for tournament slug '{}'.",
            tournament_slug
        ));
    }

    Ok((tournament_id, metadata_events))
}

pub async fn fetch_page_data(
    http_client: &reqwest::Client,
    auth_token: &str,
    tournament_slug: &str,
    page: i32,
    sets_per_page: i32,
) -> Result<SetsPageResponse, String> {
    let sets_operation = queries::tournament::TournamentSetsQuery::build(
        queries::tournament::TournamentDataVariables {
            tourney_slug: Some(tournament_slug),
            page,
            per_page: sets_per_page,
        },
    );

    let (sets_response, _) = execute_graphql_operation::<
        queries::tournament::TournamentSetsQuery,
        queries::tournament::TournamentDataVariables,
    >(
        http_client,
        auth_token,
        sets_operation,
        tournament_slug,
        "sets",
    )
    .await?;

    Ok(SetsPageResponse {
        sets: sets_response,
    })
}

pub fn process_page_data(
    page_responses: SetsPageResponse,
    context: ProcessSetsPageContext<'_>,
) -> Result<bool, String> {
    let SetsPageResponse {
        sets: sets_response,
    } = page_responses;
    let tournament_slug = context.tournament_slug;

    if let Some(errors) = sets_response.errors {
        return Err(format!(
            "GraphQL errors for {} [sets]: {:?}",
            tournament_slug, errors
        ));
    }

    let Some(sets_data) = sets_response.data else {
        return Err(format!(
            "Missing GraphQL data for {} [sets]",
            tournament_slug
        ));
    };

    let sets_events = sets_data
        .tournament
        .and_then(|t| t.events)
        .unwrap_or_default();

    let mut page_added_any_data = false;

    // Process sets
    for maybe_event in sets_events {
        let Some(event) = maybe_event else {
            continue;
        };

        if let Some(total_pages) = event
            .sets
            .as_ref()
            .and_then(|conn| conn.page_info.as_ref())
            .and_then(|info| info.total_pages)
        {
            *context.known_total_pages = Some(match *context.known_total_pages {
                Some(previous) => previous.max(total_pages),
                None => total_pages,
            });
        }

        let Some(event_id_key) = event.id.as_ref().map(id_to_string) else {
            continue;
        };

        if let Some(accumulator) = context.event_by_id.get_mut(&event_id_key) {
            page_added_any_data |= accumulator.absorb_sets_page(event);
        }
    }

    Ok(page_added_any_data)
}

async fn fetch_event_standings_page(
    http_client: &reqwest::Client,
    auth_token: &str,
    tournament_slug: &str,
    event_id: &queries::scalars::StartggId,
    page: i32,
    per_page: i32,
) -> Result<GraphQlResponse<queries::tournament::EventStandingsQuery>, String> {
    let standings_operation = queries::tournament::EventStandingsQuery::build(
        queries::tournament::EventStandingsVariables {
            event_id: Some(event_id.clone()),
            page,
            per_page,
        },
    );

    let (response, _) = execute_graphql_operation::<
        queries::tournament::EventStandingsQuery,
        queries::tournament::EventStandingsVariables,
    >(
        http_client,
        auth_token,
        standings_operation,
        tournament_slug,
        "standings-by-event",
    )
    .await?;

    Ok(response)
}

async fn fetch_all_event_standings(
    http_client: &reqwest::Client,
    auth_token: &str,
    tournament_slug: &str,
    tournament_id: &Option<String>,
    metadata_events: &[Option<queries::tournament::MetadataEvent>],
    standings_per_page: i32,
    event_by_id: &mut HashMap<String, EventAccumulator>,
    mut on_progress: impl FnMut(DownloadProgressEvent),
) -> Result<(), String> {
    for metadata_event in metadata_events {
        let Some(metadata_event) = metadata_event.as_ref() else {
            continue;
        };
        let Some(event_id) = metadata_event.id.as_ref() else {
            continue;
        };

        let event_id_key = id_to_string(event_id);
        ensure_event_accumulator(
            tournament_slug,
            tournament_id,
            metadata_events,
            event_by_id,
            &event_id_key,
        );

        let event_name = metadata_event
            .name
            .clone()
            .unwrap_or_else(|| event_id_key.clone());
        let mut page = 1;
        let mut total_pages = 1;
        let mut per_page = standings_per_page.max(1);

        loop {
            on_progress(DownloadProgressEvent {
                tournament_slug: tournament_slug.to_string(),
                page,
                message: format!(
                    "{}: downloading standings for {} ({}/{})...",
                    tournament_slug, event_name, page, total_pages
                ),
                done: false,
            });

            let response = match fetch_event_standings_page(
                http_client,
                auth_token,
                tournament_slug,
                event_id,
                page,
                per_page,
            )
            .await
            {
                Ok(response) => response,
                Err(error_message) if is_page_size_too_high_error(&error_message) => {
                    let reduced_per_page = (per_page / 2).max(1);
                    if reduced_per_page == per_page {
                        return Err(format!(
                            "Query is still too heavy for {} [standings {}] even with per_page=1. Last error: {}",
                            tournament_slug, event_id_key, error_message
                        ));
                    }

                    per_page = reduced_per_page;
                    on_progress(DownloadProgressEvent {
                        tournament_slug: tournament_slug.to_string(),
                        page,
                        message: format!(
                            "{}: standings request for {} was too heavy ({}), reducing per_page to {} and retrying page {}...",
                            tournament_slug, event_name, error_message, per_page, page
                        ),
                        done: false,
                    });
                    continue;
                }
                Err(error_message) => return Err(error_message),
            };

            if let Some(errors) = response.errors {
                let error_message = format!(
                    "GraphQL errors for {} [standings {}]: {:?}",
                    tournament_slug, event_id_key, errors
                );

                if is_page_size_too_high_error(&error_message) {
                    let reduced_per_page = (per_page / 2).max(1);
                    if reduced_per_page == per_page {
                        return Err(format!(
                            "Query is still too heavy for {} [standings {}] even with per_page=1. Last error: {}",
                            tournament_slug, event_id_key, error_message
                        ));
                    }

                    per_page = reduced_per_page;
                    on_progress(DownloadProgressEvent {
                        tournament_slug: tournament_slug.to_string(),
                        page,
                        message: format!(
                            "{}: standings query for {} too heavy, reducing per_page to {} and retrying page {}...",
                            tournament_slug, event_name, per_page, page
                        ),
                        done: false,
                    });
                    continue;
                }

                return Err(error_message);
            }

            let Some(data) = response.data else {
                return Err(format!(
                    "Missing GraphQL data for {} [standings {}]",
                    tournament_slug, event_id_key
                ));
            };

            let Some(event) = data.event else {
                break;
            };

            if let Some(found_total_pages) = event
                .standings
                .as_ref()
                .and_then(|conn| conn.page_info.as_ref())
                .and_then(|info| info.total_pages)
            {
                total_pages = found_total_pages.max(1);
            }

            let resolved_event_id = event
                .id
                .as_ref()
                .map(id_to_string)
                .unwrap_or(event_id_key.clone());
            ensure_event_accumulator(
                tournament_slug,
                tournament_id,
                metadata_events,
                event_by_id,
                &resolved_event_id,
            );

            if let Some(accumulator) = event_by_id.get_mut(&resolved_event_id) {
                accumulator.absorb_event_standings_page(event);
            }

            if page >= total_pages {
                break;
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
            page += 1;
        }
    }

    Ok(())
}

fn event_metadata_by_id(
    metadata_events: &[Option<queries::tournament::MetadataEvent>],
    event_id_key: &str,
) -> (String, Option<String>) {
    metadata_events
        .iter()
        .find_map(|metadata_event| {
            metadata_event.as_ref().and_then(|event| {
                let metadata_event_id = event.id.as_ref().map(id_to_string)?;
                if metadata_event_id == event_id_key {
                    Some((
                        event.name.clone().unwrap_or_default(),
                        event
                            .start_at
                            .as_ref()
                            .and_then(|timestamp| format_event_date(timestamp.0)),
                    ))
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| (String::new(), None))
}

fn ensure_event_accumulator(
    tournament_slug: &str,
    tournament_id: &Option<String>,
    metadata_events: &[Option<queries::tournament::MetadataEvent>],
    event_by_id: &mut HashMap<String, EventAccumulator>,
    event_id_key: &str,
) {
    if event_by_id.contains_key(event_id_key) {
        return;
    }

    let (event_name, event_date) = event_metadata_by_id(metadata_events, event_id_key);
    event_by_id.insert(
        event_id_key.to_string(),
        EventAccumulator::new(
            tournament_slug,
            tournament_id.clone(),
            Some(event_id_key.to_string()),
            event_name,
            event_date,
        ),
    );
}

async fn fetch_event_entrants_page(
    http_client: &reqwest::Client,
    auth_token: &str,
    tournament_slug: &str,
    event_id: &queries::scalars::StartggId,
    page: i32,
    per_page: i32,
) -> Result<GraphQlResponse<queries::tournament::EventEntrantsQuery>, String> {
    let entrants_operation = queries::tournament::EventEntrantsQuery::build(
        queries::tournament::EventEntrantsVariables {
            event_id: Some(event_id.clone()),
            page,
            per_page,
        },
    );

    let (response, _) = execute_graphql_operation::<
        queries::tournament::EventEntrantsQuery,
        queries::tournament::EventEntrantsVariables,
    >(
        http_client,
        auth_token,
        entrants_operation,
        tournament_slug,
        "entrants-by-event",
    )
    .await?;

    Ok(response)
}

async fn fetch_all_event_entrants(
    http_client: &reqwest::Client,
    auth_token: &str,
    tournament_slug: &str,
    tournament_id: &Option<String>,
    metadata_events: &[Option<queries::tournament::MetadataEvent>],
    entrants_per_page: i32,
    event_by_id: &mut HashMap<String, EventAccumulator>,
    mut on_progress: impl FnMut(DownloadProgressEvent),
) -> Result<(), String> {
    for metadata_event in metadata_events {
        let Some(metadata_event) = metadata_event.as_ref() else {
            continue;
        };
        let Some(event_id) = metadata_event.id.as_ref() else {
            continue;
        };

        let event_id_key = id_to_string(event_id);
        ensure_event_accumulator(
            tournament_slug,
            tournament_id,
            metadata_events,
            event_by_id,
            &event_id_key,
        );

        let event_name = metadata_event
            .name
            .clone()
            .unwrap_or_else(|| event_id_key.clone());
        let mut page = 1;
        let mut total_pages = 1;
        let mut per_page = entrants_per_page.max(1);

        loop {
            on_progress(DownloadProgressEvent {
                tournament_slug: tournament_slug.to_string(),
                page,
                message: format!(
                    "{}: downloading entrants for {} ({}/{})...",
                    tournament_slug, event_name, page, total_pages
                ),
                done: false,
            });

            let response = match fetch_event_entrants_page(
                http_client,
                auth_token,
                tournament_slug,
                event_id,
                page,
                per_page,
            )
            .await
            {
                Ok(response) => response,
                Err(error_message) if is_page_size_too_high_error(&error_message) => {
                    let reduced_per_page = (per_page / 2).max(1);
                    if reduced_per_page == per_page {
                        return Err(format!(
                            "Query is still too heavy for {} [entrants {}] even with per_page=1. Last error: {}",
                            tournament_slug, event_id_key, error_message
                        ));
                    }

                    per_page = reduced_per_page;
                    on_progress(DownloadProgressEvent {
                        tournament_slug: tournament_slug.to_string(),
                        page,
                        message: format!(
                            "{}: entrants request for {} was too heavy ({}), reducing per_page to {} and retrying page {}...",
                            tournament_slug, event_name, error_message, per_page, page
                        ),
                        done: false,
                    });
                    continue;
                }
                Err(error_message) => return Err(error_message),
            };

            if let Some(errors) = response.errors {
                let error_message = format!(
                    "GraphQL errors for {} [entrants {}]: {:?}",
                    tournament_slug, event_id_key, errors
                );

                if is_page_size_too_high_error(&error_message) {
                    let reduced_per_page = (per_page / 2).max(1);
                    if reduced_per_page == per_page {
                        return Err(format!(
                            "Query is still too heavy for {} [entrants {}] even with per_page=1. Last error: {}",
                            tournament_slug, event_id_key, error_message
                        ));
                    }

                    per_page = reduced_per_page;
                    on_progress(DownloadProgressEvent {
                        tournament_slug: tournament_slug.to_string(),
                        page,
                        message: format!(
                            "{}: entrants query for {} too heavy, reducing per_page to {} and retrying page {}...",
                            tournament_slug, event_name, per_page, page
                        ),
                        done: false,
                    });
                    continue;
                }

                return Err(format!(
                    "GraphQL errors for {} [entrants {}]: {:?}",
                    tournament_slug, event_id_key, errors
                ));
            }

            let Some(data) = response.data else {
                return Err(format!(
                    "Missing GraphQL data for {} [entrants {}]",
                    tournament_slug, event_id_key
                ));
            };

            let Some(event) = data.event else {
                break;
            };

            if let Some(found_total_pages) = event
                .entrants
                .as_ref()
                .and_then(|conn| conn.page_info.as_ref())
                .and_then(|info| info.total_pages)
            {
                total_pages = found_total_pages.max(1);
            }

            let resolved_event_id = event
                .id
                .as_ref()
                .map(id_to_string)
                .unwrap_or(event_id_key.clone());
            ensure_event_accumulator(
                tournament_slug,
                tournament_id,
                metadata_events,
                event_by_id,
                &resolved_event_id,
            );

            if let Some(accumulator) = event_by_id.get_mut(&resolved_event_id) {
                accumulator.absorb_event_entrants_page(event);
            }

            if page >= total_pages {
                break;
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
            page += 1;
        }
    }

    Ok(())
}

pub async fn query_tournament_rows(
    tournament_slugs: Vec<String>,
    auth_token: String,
    mut on_progress: impl FnMut(DownloadProgressEvent),
) -> Result<Vec<TournamentRow>, String> {
    const INITIAL_STANDINGS_PER_PAGE: i32 = 10;
    const ENTRANTS_PER_PAGE: i32 = 50;
    const INITIAL_SETS_PER_PAGE: i32 = 50;
    const MAX_PAGES_PER_TOURNAMENT: i32 = 1000;

    let mut all_rows: Vec<TournamentRow> = Vec::new();
    let http_client = reqwest::Client::new();

    for (index, tournament_slug) in tournament_slugs.into_iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        let tournament_started_at = Instant::now();

        // ======= STEP 1: Fetch tournament metadata =======
        let (tournament_id, metadata_events) =
            fetch_tournament_metadata(&http_client, &auth_token, &tournament_slug).await?;

        let mut event_by_id: HashMap<String, EventAccumulator> = HashMap::new();

        fetch_all_event_entrants(
            &http_client,
            &auth_token,
            &tournament_slug,
            &tournament_id,
            &metadata_events,
            ENTRANTS_PER_PAGE,
            &mut event_by_id,
            &mut on_progress,
        )
        .await?;

        fetch_all_event_standings(
            &http_client,
            &auth_token,
            &tournament_slug,
            &tournament_id,
            &metadata_events,
            INITIAL_STANDINGS_PER_PAGE,
            &mut event_by_id,
            &mut on_progress,
        )
        .await?;

        // ======= STEP 2: Paginate sets, then merge into entrants =======
        let mut page = 1;
        let mut known_total_pages: Option<i32> = None;
        let mut sets_per_page = INITIAL_SETS_PER_PAGE;

        loop {
            if page > MAX_PAGES_PER_TOURNAMENT {
                return Err(format!(
                    "Reached pagination safety limit ({}) for '{}'.",
                    MAX_PAGES_PER_TOURNAMENT, tournament_slug
                ));
            }

            if let Some(total_pages) = known_total_pages {
                if page > total_pages {
                    on_progress(DownloadProgressEvent {
                        tournament_slug: tournament_slug.clone(),
                        page,
                        message: format!("{}: Creating CSV file...", tournament_slug),
                        done: false,
                    });
                    break;
                }
            }

            let page_started_at = Instant::now();

            on_progress(DownloadProgressEvent {
                tournament_slug: tournament_slug.clone(),
                page,
                message: format!(
                    "{}: downloading {}...",
                    tournament_slug,
                    format_page_counter(page, known_total_pages)
                ),
                done: false,
            });

            let page_responses = match fetch_page_data(
                &http_client,
                &auth_token,
                &tournament_slug,
                page,
                sets_per_page,
            )
            .await
            {
                Ok(response) => response,
                Err(error_message) if is_page_size_too_high_error(&error_message) => {
                    let reduced_sets_per_page = (sets_per_page / 2).max(1);

                    if reduced_sets_per_page == sets_per_page {
                        return Err(format!(
                            "Query is still too heavy for {} at page {} even with per_page=1. Last error: {}",
                            tournament_slug, page, error_message
                        ));
                    }

                    sets_per_page = reduced_sets_per_page;
                    on_progress(DownloadProgressEvent {
                        tournament_slug: tournament_slug.clone(),
                        page,
                        message: format!(
                            "{}: sets request too heavy at page {} ({}), reducing sets_per_page to {}, then retrying...",
                            tournament_slug,
                            page,
                            error_message,
                            sets_per_page
                        ),
                        done: false,
                    });
                    continue;
                }
                Err(error_message) => return Err(error_message),
            };

            on_progress(DownloadProgressEvent {
                tournament_slug: tournament_slug.clone(),
                page,
                message: format!(
                    "{}: fetched {}",
                    tournament_slug,
                    format_page_counter(page, known_total_pages)
                ),
                done: false,
            });

            let page_added_any_data = match process_page_data(
                page_responses,
                ProcessSetsPageContext {
                    tournament_slug: &tournament_slug,
                    event_by_id: &mut event_by_id,
                    known_total_pages: &mut known_total_pages,
                },
            ) {
                Ok(result) => result,
                Err(error_message) if is_page_size_too_high_error(&error_message) => {
                    let reduced_sets_per_page = (sets_per_page / 2).max(1);

                    if reduced_sets_per_page == sets_per_page {
                        return Err(format!(
                            "Query is still too heavy for {} at page {} even with per_page=1. Last error: {}",
                            tournament_slug, page, error_message
                        ));
                    }

                    sets_per_page = reduced_sets_per_page;
                    on_progress(DownloadProgressEvent {
                        tournament_slug: tournament_slug.clone(),
                        page,
                        message: format!(
                            "{}: sets query too heavy at page {}, reducing sets_per_page to {}, then retrying...",
                            tournament_slug, page, sets_per_page
                        ),
                        done: false,
                    });
                    continue;
                }
                Err(error_message) => return Err(error_message),
            };

            if !page_added_any_data {
                break;
            }

            let current_entrants: usize = event_by_id
                .values()
                .map(|event_acc| event_acc.row_by_entrant_id.len())
                .sum();
            let page_elapsed_ms = page_started_at.elapsed().as_millis();
            on_progress(DownloadProgressEvent {
                tournament_slug: tournament_slug.clone(),
                page,
                message: format!(
                    "{}: finished {} ({} entrants aggregated, total {} ms)",
                    tournament_slug,
                    format_page_counter(page, known_total_pages),
                    current_entrants,
                    page_elapsed_ms
                ),
                done: false,
            });

            // Throttle requests to avoid rate limits
            tokio::time::sleep(Duration::from_millis(400)).await;

            page += 1;
        }

        if event_by_id.is_empty() {
            return Err(format!(
                "No entrant or set data found for tournament slug '{}'.",
                tournament_slug
            ));
        }

        let slug_rows: Vec<TournamentRow> = event_by_id
            .into_values()
            .flat_map(EventAccumulator::into_rows)
            .collect();

        if slug_rows.is_empty() {
            return Err(format!(
                "No entrant or set data found for tournament slug '{}'.",
                tournament_slug
            ));
        }

        on_progress(DownloadProgressEvent {
            tournament_slug: tournament_slug.clone(),
            page,
            message: format!(
                "{}: complete ({} rows across {} pages, total {} ms)",
                tournament_slug,
                slug_rows.len(),
                page.saturating_sub(1),
                tournament_started_at.elapsed().as_millis()
            ),
            done: true,
        });

        all_rows.extend(slug_rows);
    }

    Ok(all_rows)
}

pub async fn query_tournament_json(tournament_name: String, auth_token: String) -> String {
    let operation = queries::tournament::TournamentMetadataQuery::build(
        queries::tournament::TournamentMetadataVariables {
            tourney_slug: Some(&tournament_name),
        },
    );

    let http_response = reqwest::Client::new()
        .post("https://api.start.gg/gql/alpha")
        .bearer_auth(&auth_token)
        .json(&operation)
        .send()
        .await;

    let http_response = match http_response {
        Err(e) => {
            println!("HTTP request failed: {}", e);
            return format!("HTTP request failed: {}", e);
        }
        Ok(r) => r,
    };

    let status = http_response.status();
    let content_type = crate::utils::content_type_header(&http_response);

    let body = match http_response.text().await {
        Err(e) => {
            println!("Failed to read response body: {}", e);
            return format!("Failed to read response body: {}", e);
        }
        Ok(t) => t,
    };

    if !status.is_success() {
        return format!(
            "start.gg returned HTTP {} for {} (content-type: {}). Body: {}",
            status.as_u16(),
            tournament_name,
            content_type,
            crate::utils::summarize_body(&body, 300)
        );
    }

    let graphql_response = match serde_json::from_str::<
        GraphQlResponse<queries::tournament::TournamentMetadataQuery>,
    >(&body)
    {
        Err(e) => {
            println!("JSON parse error: {}", e);
            return format!(
                "JSON parse error for {}: {} (content-type: {}). Body: {}",
                tournament_name,
                e,
                content_type,
                crate::utils::summarize_body(&body, 300)
            );
        }
        Ok(r) => r,
    };

    if let Some(errors) = graphql_response.errors {
        return format!("GraphQL errors: {:?}", errors);
    }

    let data = graphql_response.data.unwrap();

    format!("{:?}", data.tournament)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn standings_query_returns_rows_for_evo_2025() {
        let auth_token = match env::var("START_GG_AUTH_TOKEN") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => {
                eprintln!("Skipping standings test because START_GG_AUTH_TOKEN is not set.");
                return;
            }
        };

        let http_client = reqwest::Client::new();

        let metadata_operation = queries::tournament::TournamentMetadataQuery::build(
            queries::tournament::TournamentMetadataVariables {
                tourney_slug: Some("tournament/evo-2025"),
            },
        );

        let (metadata_response, _) = execute_graphql_operation::<
            queries::tournament::TournamentMetadataQuery,
            queries::tournament::TournamentMetadataVariables,
        >(
            &http_client,
            &auth_token,
            metadata_operation,
            "tournament/evo-2025",
            "standings-metadata-test",
        )
        .await
        .expect("metadata query should complete successfully");

        assert!(
            metadata_response.errors.is_none(),
            "unexpected metadata GraphQL errors: {:?}",
            metadata_response.errors
        );

        let metadata = metadata_response
            .data
            .expect("metadata query should return data")
            .tournament
            .expect("evo-2025 tournament should exist in metadata query response");

        let first_event_id = metadata
            .events
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .find_map(|event| event.id);

        let event_id = first_event_id.expect("metadata query should include at least one event ID");

        let standings_operation = queries::tournament::EventStandingsQuery::build(
            queries::tournament::EventStandingsVariables {
                event_id: Some(event_id),
                page: 1,
                per_page: 10,
            },
        );

        let (response, _) = execute_graphql_operation::<
            queries::tournament::EventStandingsQuery,
            queries::tournament::EventStandingsVariables,
        >(
            &http_client,
            &auth_token,
            standings_operation,
            "tournament/evo-2025",
            "standings-by-event-test",
        )
        .await
        .expect("event standings query should complete successfully");

        assert!(
            response.errors.is_none(),
            "unexpected GraphQL errors: {:?}",
            response.errors
        );

        let data = response
            .data
            .expect("standings query should return a GraphQL data payload");
        let event = data
            .event
            .expect("event standings query should include the event payload");

        let has_standings_row = event
            .standings
            .and_then(|conn| conn.nodes)
            .map(|nodes| {
                nodes.into_iter().flatten().any(|standing| {
                    standing.placement.is_some()
                        || standing
                            .entrant
                            .as_ref()
                            .and_then(|entrant| entrant.id.as_ref())
                            .is_some()
                        || standing
                            .player
                            .as_ref()
                            .and_then(|player| player.id.as_ref())
                            .is_some()
                })
            })
            .unwrap_or(false);

        assert!(
            has_standings_row,
            "expected tournament/evo-2025 event standings query to return at least one standings row"
        );
    }
}
