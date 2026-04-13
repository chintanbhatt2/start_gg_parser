mod queries;

use cynic::{GraphQlResponse, QueryBuilder};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tauri::Emitter;

#[cynic::schema("startgg")]
mod schema {}

#[derive(Debug, serde::Serialize)]
struct TournamentRow {
    tournament_slug: String,
    tournament_id: Option<String>,
    event_id: Option<String>,
    event_name: String,
    event_date: Option<String>,
    entrant_id: String,
    player_id: Option<String>,
    placement: Option<i32>,
    wins: i32,
    losses: i32,
    player_name: String,
    player_prefix: String,
    discord_usernames: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgressEvent {
    tournament_slug: String,
    page: i32,
    message: String,
    done: bool,
}

#[derive(Debug, Default)]
struct MutableTournamentRow {
    tournament_slug: String,
    tournament_id: Option<String>,
    event_id: Option<String>,
    event_name: String,
    event_date: Option<String>,
    entrant_id: String,
    player_id: Option<String>,
    placement: Option<i32>,
    wins: i32,
    losses: i32,
    player_name: String,
    player_prefix: String,
    discord_usernames: String,
}

#[derive(Debug)]
struct EventAccumulator {
    tournament_slug: String,
    tournament_id: Option<String>,
    event_id: Option<String>,
    event_name: String,
    event_date: Option<String>,
    row_by_entrant_id: HashMap<String, MutableTournamentRow>,
    winner_by_set_id: HashMap<String, String>,
    set_ids_by_entrant_id: HashMap<String, HashSet<String>>,
}

impl From<MutableTournamentRow> for TournamentRow {
    fn from(value: MutableTournamentRow) -> Self {
        Self {
            tournament_slug: value.tournament_slug,
            tournament_id: value.tournament_id,
            event_id: value.event_id,
            event_name: value.event_name,
            event_date: value.event_date,
            entrant_id: value.entrant_id,
            player_id: value.player_id,
            placement: value.placement,
            wins: value.wins,
            losses: value.losses,
            player_name: value.player_name,
            player_prefix: value.player_prefix,
            discord_usernames: value.discord_usernames,
        }
    }
}

fn get_entrant_discord_usernames(entrant: &queries::tournament::EntrantsEntrant) -> String {
    let mut usernames: Vec<String> = Vec::new();

    if let Some(participants) = entrant.participants.as_ref() {
        for maybe_participant in participants {
            let Some(participant) = maybe_participant.as_ref() else {
                continue;
            };

            let Some(user) = participant.user.as_ref() else {
                continue;
            };

            let Some(authorizations) = user.authorizations.as_ref() else {
                continue;
            };

            for maybe_auth in authorizations {
                let Some(auth) = maybe_auth.as_ref() else {
                    continue;
                };

                let Some(username) = auth.external_username.as_ref() else {
                    continue;
                };

                let trimmed = username.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if !usernames.iter().any(|existing| existing == trimmed) {
                    usernames.push(trimmed.to_string());
                }
            }
        }
    }

    usernames.join(" | ")
}

fn format_event_date(timestamp_secs: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(timestamp_secs, 0).map(|dt| dt.format("%Y-%m-%d").to_string())
}

fn split_prefix_and_name(entrant_name: &str) -> (String, String) {
    let split_name: Vec<&str> = entrant_name.split('|').collect();
    if split_name.len() == 1 {
        (String::new(), entrant_name.trim().to_string())
    } else {
        let prefix = split_name[..split_name.len() - 1]
            .iter()
            .map(|part| part.trim())
            .collect::<Vec<_>>()
            .join(" | ");
        let name = split_name[split_name.len() - 1].trim().to_string();
        (prefix, name)
    }
}

fn id_to_string(id: &queries::scalars::StartggId) -> String {
    id.as_string().to_string()
}

fn winner_id_to_string(winner_id: i32) -> String {
    winner_id.to_string()
}

fn summarize_body(body: &str, max_len: usize) -> String {
    let trimmed = body.trim();
    let mut excerpt: String = trimmed.chars().take(max_len).collect();
    if trimmed.chars().count() > max_len {
        excerpt.push_str("...");
    }
    excerpt
}

fn content_type_header(response: &reqwest::Response) -> String {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<missing>")
        .to_string()
}

async fn execute_graphql_operation<T: serde::de::DeserializeOwned>(
    http_client: &reqwest::Client,
    auth_token: &str,
    operation: impl serde::Serialize,
    tournament_slug: &str,
    operation_name: &str,
) -> Result<(GraphQlResponse<T>, Duration), String> {
    let started_at = Instant::now();

    let http_response = http_client
        .post("https://api.start.gg/gql/alpha")
        .bearer_auth(auth_token)
        .json(&operation)
        .send()
        .await
        .map_err(|e| {
            format!(
                "HTTP request failed for {} [{}]: {}",
                tournament_slug, operation_name, e
            )
        })?;

    let status = http_response.status();
    let content_type = content_type_header(&http_response);

    let body = http_response.text().await.map_err(|e| {
        format!(
            "Failed to read response body for {} [{}]: {}",
            tournament_slug, operation_name, e
        )
    })?;

    if !status.is_success() {
        return Err(format!(
            "start.gg returned HTTP {} for {} [{}] (content-type: {}). Body: {}",
            status.as_u16(),
            tournament_slug,
            operation_name,
            content_type,
            summarize_body(&body, 300)
        ));
    }

    let graphql_response = serde_json::from_str(&body).map_err(|e| {
        format!(
            "JSON parse error for {} [{}]: {} (content-type: {}). Body: {}",
            tournament_slug,
            operation_name,
            e,
            content_type,
            summarize_body(&body, 300)
        )
    })?;

    Ok((graphql_response, started_at.elapsed()))
}

fn format_page_counter(page: i32, total_pages: Option<i32>) -> String {
    match total_pages {
        Some(total) if total > 0 => format!("page {} of {}", page, total),
        _ => format!("page {} of ?", page),
    }
}

impl EventAccumulator {
    fn new(
        tournament_slug: &str,
        tournament_id: Option<String>,
        event_id: Option<String>,
        event_name: String,
        event_date: Option<String>,
    ) -> Self {
        Self {
            tournament_slug: tournament_slug.to_string(),
            tournament_id,
            event_id,
            event_name,
            event_date,
            row_by_entrant_id: HashMap::new(),
            winner_by_set_id: HashMap::new(),
            set_ids_by_entrant_id: HashMap::new(),
        }
    }

    fn absorb_entrants_page(&mut self, event: queries::tournament::EntrantsEvent) -> bool {
        let mut changed = false;

        if let Some(entrant_nodes) = event
            .entrants
            .as_ref()
            .and_then(|entrants| entrants.nodes.as_ref())
        {
            for maybe_entrant in entrant_nodes {
                let Some(entrant) = maybe_entrant else {
                    continue;
                };

                let Some(entrant_id) = entrant.id.as_ref().map(id_to_string) else {
                    continue;
                };

                let entrant_name = entrant.name.clone().unwrap_or_default();
                let (player_prefix, player_name) = split_prefix_and_name(&entrant_name);
                let discord_usernames = get_entrant_discord_usernames(entrant);

                let was_known_entrant = self.row_by_entrant_id.contains_key(&entrant_id);
                self
                    .row_by_entrant_id
                    .entry(entrant_id.clone())
                    .or_insert(MutableTournamentRow {
                        tournament_slug: self.tournament_slug.clone(),
                        tournament_id: self.tournament_id.clone(),
                        event_id: self.event_id.clone(),
                        event_name: self.event_name.clone(),
                        event_date: self.event_date.clone(),
                        entrant_id: entrant_id.clone(),
                        player_id: None,
                        placement: None,
                        wins: 0,
                        losses: 0,
                        player_name,
                        player_prefix,
                        discord_usernames: discord_usernames.clone(),
                    });

                if let Some(row) = self.row_by_entrant_id.get_mut(&entrant_id) {
                    if row.discord_usernames.is_empty() && !discord_usernames.is_empty() {
                        row.discord_usernames = discord_usernames;
                        changed = true;
                    }
                }

                if !was_known_entrant {
                    changed = true;
                }

                if let Some(entrant_sets) = entrant
                    .paginated_sets
                    .as_ref()
                    .and_then(|sets| sets.nodes.as_ref())
                {
                    let set_ids = self
                        .set_ids_by_entrant_id
                        .entry(entrant_id)
                        .or_default();

                    for maybe_set in entrant_sets {
                        let Some(set_item) = maybe_set.as_ref() else {
                            continue;
                        };

                        let Some(set_id) = set_item.id.as_ref().map(id_to_string) else {
                            continue;
                        };

                        changed |= set_ids.insert(set_id);
                    }
                }
            }
        }

        if let Some(standing_nodes) = event.standings.and_then(|standings| standings.nodes) {
            for maybe_standing in standing_nodes {
                let Some(standing) = maybe_standing else {
                    continue;
                };

                let Some(entrant_id) = standing
                    .entrant
                    .and_then(|entrant| entrant.id)
                    .as_ref()
                    .map(id_to_string)
                else {
                    continue;
                };

                if let Some(row) = self.row_by_entrant_id.get_mut(&entrant_id) {
                    let new_player_id = standing
                        .player
                        .and_then(|player| player.id)
                        .as_ref()
                        .map(id_to_string);

                    if row.placement != standing.placement || row.player_id != new_player_id {
                        row.placement = standing.placement;
                        row.player_id = new_player_id;
                        changed = true;
                    }
                }
            }
        }

        changed
    }

    fn absorb_sets_page(&mut self, event: queries::tournament::SetsEvent) -> bool {
        let mut changed = false;

        if let Some(set_nodes) = event.sets.and_then(|sets| sets.nodes) {
            for maybe_set in set_nodes {
                let Some(set_item) = maybe_set else {
                    continue;
                };

                let (Some(set_id), Some(winner_id)) = (set_item.id, set_item.winner_id) else {
                    continue;
                };

                let inserted = self
                    .winner_by_set_id
                    .insert(id_to_string(&set_id), winner_id_to_string(winner_id))
                    .is_none();
                changed |= inserted;
            }
        }

        changed
    }

    fn into_rows(mut self) -> Vec<TournamentRow> {
        for (entrant_id, row) in &mut self.row_by_entrant_id {
            let Some(set_ids) = self.set_ids_by_entrant_id.get(entrant_id) else {
                continue;
            };

            for set_id in set_ids {
                let Some(winner_entrant_id) = self.winner_by_set_id.get(set_id) else {
                    continue;
                };

                if winner_entrant_id == entrant_id {
                    row.wins += 1;
                } else {
                    row.losses += 1;
                }
            }
        }

        self.row_by_entrant_id
            .into_values()
            .map(TournamentRow::from)
            .collect()
    }
}

async fn query_tournament_rows(
    tournament_slugs: Vec<String>,
    auth_token: String,
    mut on_progress: impl FnMut(DownloadProgressEvent),
) -> Result<Vec<TournamentRow>, String> {
    const QUERY_PER_PAGE: i32 = 10;
    const MAX_PAGES_PER_TOURNAMENT: i32 = 1000;

    let mut all_rows: Vec<TournamentRow> = Vec::new();
    let http_client = reqwest::Client::new();

    for (index, tournament_slug) in tournament_slugs.into_iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let tournament_started_at = Instant::now();

        let mut page = 1;
        let mut known_total_pages: Option<i32> = None;
        let mut event_by_id: HashMap<String, EventAccumulator> = HashMap::new();
        let mut saw_events = false;

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

            let entrants_operation = queries::tournament::TournamentEntrantsQuery::build(
                queries::tournament::TournamentQueryVariables {
                    tourney_slug: Some(&tournament_slug),
                    page,
                    per_page: QUERY_PER_PAGE,
                },
            );
            let sets_operation = queries::tournament::TournamentSetsQuery::build(
                queries::tournament::TournamentQueryVariables {
                    tourney_slug: Some(&tournament_slug),
                    page,
                    per_page: QUERY_PER_PAGE,
                },
            );

            let (entrants_response_result, sets_response_result) = tokio::join!(
                execute_graphql_operation::<queries::tournament::TournamentEntrantsQuery>(
                    &http_client,
                    &auth_token,
                    entrants_operation,
                    &tournament_slug,
                    "entrants-standings"
                ),
                execute_graphql_operation::<queries::tournament::TournamentSetsQuery>(
                    &http_client,
                    &auth_token,
                    sets_operation,
                    &tournament_slug,
                    "sets"
                )
            );
            let (entrants_response, entrants_elapsed) = entrants_response_result?;
            let (sets_response, sets_elapsed) = sets_response_result?;

            on_progress(DownloadProgressEvent {
                tournament_slug: tournament_slug.clone(),
                page,
                message: format!(
                    "{}: fetched {} (entrants/standings {} ms, sets {} ms)",
                    tournament_slug,
                    format_page_counter(page, known_total_pages),
                    entrants_elapsed.as_millis(),
                    sets_elapsed.as_millis()
                ),
                done: false,
            });

            if let Some(errors) = entrants_response.errors {
                return Err(format!(
                    "GraphQL errors for {} [entrants-standings]: {:?}",
                    tournament_slug, errors
                ));
            }
            if let Some(errors) = sets_response.errors {
                return Err(format!(
                    "GraphQL errors for {} [sets]: {:?}",
                    tournament_slug, errors
                ));
            }

            let Some(entrants_data) = entrants_response.data else {
                return Err(format!(
                    "Missing GraphQL data for {} [entrants-standings]",
                    tournament_slug
                ));
            };
            let Some(sets_data) = sets_response.data else {
                return Err(format!("Missing GraphQL data for {} [sets]", tournament_slug));
            };

            let Some(entrants_tournament) = entrants_data.tournament else {
                return Err(format!(
                    "Tournament not found for slug '{}'. Use a tournament slug like tournament/my-event.",
                    tournament_slug
                ));
            };
            let Some(sets_tournament) = sets_data.tournament else {
                return Err(format!(
                    "Tournament not found for slug '{}' [sets].",
                    tournament_slug
                ));
            };

            let tournament_id = entrants_tournament.id.as_ref().map(id_to_string);
            let entrants_events = entrants_tournament.events.unwrap_or_default();
            let sets_events = sets_tournament.events.unwrap_or_default();

            if entrants_events.is_empty() && sets_events.is_empty() {
                break;
            };

            let mut page_added_any_data = false;

            for maybe_event in entrants_events {
                let Some(event) = maybe_event else {
                    continue;
                };

                saw_events = true;

                let mut event_total_pages = page;
                if let Some(total_pages) = event
                    .standings
                    .as_ref()
                    .and_then(|conn| conn.page_info.as_ref())
                    .and_then(|info| info.total_pages)
                {
                    event_total_pages = event_total_pages.max(total_pages);
                }
                if let Some(total_pages) = event
                    .entrants
                    .as_ref()
                    .and_then(|conn| conn.page_info.as_ref())
                    .and_then(|info| info.total_pages)
                {
                    event_total_pages = event_total_pages.max(total_pages);
                }
                if let Some(entrant_nodes) = event
                    .entrants
                    .as_ref()
                    .and_then(|entrants| entrants.nodes.as_ref())
                {
                    for maybe_entrant in entrant_nodes {
                        let Some(entrant) = maybe_entrant else {
                            continue;
                        };

                        if let Some(total_pages) = entrant
                            .paginated_sets
                            .as_ref()
                            .and_then(|conn| conn.page_info.as_ref())
                            .and_then(|info| info.total_pages)
                        {
                            event_total_pages = event_total_pages.max(total_pages);
                        }
                    }
                }
                known_total_pages = Some(match known_total_pages {
                    Some(previous) => previous.max(event_total_pages),
                    None => event_total_pages,
                });

                let Some(event_id_key) = event.id.as_ref().map(id_to_string) else {
                    continue;
                };

                let event_name = event.name.clone().unwrap_or_default();
                let event_date = event.start_at.as_ref().and_then(|t| format_event_date(t.0));

                let accumulator = event_by_id.entry(event_id_key.clone()).or_insert_with(|| {
                    EventAccumulator::new(
                        &tournament_slug,
                        tournament_id.clone(),
                        Some(event_id_key.clone()),
                        event_name,
                        event_date,
                    )
                });

                page_added_any_data |= accumulator.absorb_entrants_page(event);
            }

            for maybe_event in sets_events {
                let Some(event) = maybe_event else {
                    continue;
                };

                saw_events = true;

                if let Some(total_pages) = event
                    .sets
                    .as_ref()
                    .and_then(|conn| conn.page_info.as_ref())
                    .and_then(|info| info.total_pages)
                {
                    known_total_pages = Some(match known_total_pages {
                        Some(previous) => previous.max(total_pages),
                        None => total_pages,
                    });
                }

                let Some(event_id_key) = event.id.as_ref().map(id_to_string) else {
                    continue;
                };

                let accumulator = event_by_id.entry(event_id_key.clone()).or_insert_with(|| {
                    EventAccumulator::new(
                        &tournament_slug,
                        tournament_id.clone(),
                        Some(event_id_key),
                        String::new(),
                        None,
                    )
                });

                page_added_any_data |= accumulator.absorb_sets_page(event);
            }

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

            page += 1;
        }

        if !saw_events {
            return Err(format!(
                "No events found for tournament slug '{}'.",
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

fn rows_to_csv(rows: &[TournamentRow]) -> Result<String, String> {
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

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn save_text_file(path: String, contents: String) -> Result<String, String> {
    match std::fs::write(&path, contents) {
        Ok(_) => Ok(format!("Saved file to {}", path)),
        Err(e) => Err(format!("Failed to save file to {}: {}", path, e)),
    }
}

#[tauri::command]
async fn get_tournament_json(tournamentName: String, authToken: String) -> String {
    println!("Received tournament name: {}", tournamentName);
    query_tournament_json(tournamentName, authToken).await
}

#[tauri::command]
async fn get_tournament_rows_json(
    app: tauri::AppHandle,
    tournamentNames: Vec<String>,
    authToken: String,
) -> Result<String, String> {
    match query_tournament_rows(tournamentNames, authToken, |progress| {
        let _ = app.emit("download-progress", progress);
    })
    .await
    {
        Ok(rows) => serde_json::to_string_pretty(&rows)
            .map_err(|e| format!("Failed to serialize rows as JSON: {}", e)),
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn get_tournament_rows_csv(
    app: tauri::AppHandle,
    tournamentNames: Vec<String>,
    authToken: String,
) -> Result<String, String> {
    match query_tournament_rows(tournamentNames, authToken, |progress| {
        let _ = app.emit("download-progress", progress);
    })
    .await
    {
        Ok(rows) => rows_to_csv(&rows).map_err(|e| format!("CSV export failed: {}", e)),
        Err(e) => Err(e),
    }
}

async fn query_tournament_json(tournament_name: String, auth_token: String) -> String {
    let operation = queries::tournament::TournamentQuery::build(
        queries::tournament::TournamentQueryVariables {
            tourney_slug: Some(&tournament_name),
            page: 1,
            per_page: 10,
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
    let content_type = content_type_header(&http_response);

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
            summarize_body(&body, 300)
        );
    }

    let graphql_response = match serde_json::from_str::<
        GraphQlResponse<queries::tournament::TournamentQuery>,
    >(&body)
    {
        Err(e) => {
            println!("JSON parse error: {}", e);
            return format!(
                "JSON parse error for {}: {} (content-type: {}). Body: {}",
                tournament_name,
                e,
                content_type,
                summarize_body(&body, 300)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            save_text_file,
            get_tournament_json,
            get_tournament_rows_json,
            get_tournament_rows_csv
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
