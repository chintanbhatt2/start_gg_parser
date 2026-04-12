mod queries;

use cynic::{GraphQlResponse, QueryBuilder};
use std::collections::HashMap;

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
        }
    }
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

fn process_tournament_response(
    tournament_slug: &str,
    tournament: queries::tournament::Tournament,
) -> Vec<TournamentRow> {
    let mut rows: Vec<TournamentRow> = Vec::new();

    let tournament_id = tournament.id.as_ref().map(id_to_string);
    let Some(events) = tournament.events else {
        return rows;
    };

    for maybe_event in events {
        let Some(event) = maybe_event else {
            continue;
        };

        let event_id = event.id.as_ref().map(id_to_string);
        let event_name = event.name.clone().unwrap_or_default();
        let event_date = event.start_at.as_ref().and_then(|t| format_event_date(t.0));

        let mut row_by_entrant_id: HashMap<String, MutableTournamentRow> = HashMap::new();
        let mut winner_by_set_id: HashMap<String, String> = HashMap::new();

        if let Some(set_nodes) = event.sets.and_then(|sets| sets.nodes) {
            for maybe_set in set_nodes {
                let Some(set_item) = maybe_set else {
                    continue;
                };

                let (Some(set_id), Some(winner_id)) = (set_item.id, set_item.winner_id) else {
                    continue;
                };

                winner_by_set_id.insert(id_to_string(&set_id), winner_id_to_string(winner_id));
            }
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

                let Some(entrant_id) = entrant.id.as_ref().map(id_to_string) else {
                    continue;
                };

                let entrant_name = entrant.name.clone().unwrap_or_default();
                let (player_prefix, player_name) = split_prefix_and_name(&entrant_name);

                row_by_entrant_id
                    .entry(entrant_id.clone())
                    .or_insert(MutableTournamentRow {
                        tournament_slug: tournament_slug.to_string(),
                        tournament_id: tournament_id.clone(),
                        event_id: event_id.clone(),
                        event_name: event_name.clone(),
                        event_date: event_date.clone(),
                        entrant_id,
                        player_id: None,
                        placement: None,
                        wins: 0,
                        losses: 0,
                        player_name,
                        player_prefix,
                    });
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

                if let Some(row) = row_by_entrant_id.get_mut(&entrant_id) {
                    row.placement = standing.placement;
                    row.player_id = standing
                        .player
                        .and_then(|player| player.id)
                        .as_ref()
                        .map(id_to_string);
                }
            }
        }

        if let Some(entrant_nodes) = event.entrants.and_then(|entrants| entrants.nodes) {
            for maybe_entrant in entrant_nodes {
                let Some(entrant) = maybe_entrant else {
                    continue;
                };

                let Some(entrant_id) = entrant.id.as_ref().map(id_to_string) else {
                    continue;
                };

                let Some(entrant_sets) = entrant.paginated_sets.and_then(|sets| sets.nodes) else {
                    continue;
                };

                for maybe_set in entrant_sets {
                    let Some(set_item) = maybe_set else {
                        continue;
                    };

                    let Some(set_id) = set_item.id.as_ref().map(id_to_string) else {
                        continue;
                    };

                    let Some(winner_entrant_id) = winner_by_set_id.get(&set_id) else {
                        continue;
                    };

                    if let Some(row) = row_by_entrant_id.get_mut(&entrant_id) {
                        if winner_entrant_id == &entrant_id {
                            row.wins += 1;
                        } else {
                            row.losses += 1;
                        }
                    }
                }
            }
        }

        rows.extend(row_by_entrant_id.into_values().map(TournamentRow::from));
    }

    rows
}

async fn query_tournament_rows(
    tournament_slugs: Vec<String>,
    auth_token: String,
) -> Result<Vec<TournamentRow>, String> {
    let mut all_rows: Vec<TournamentRow> = Vec::new();

    for tournament_slug in tournament_slugs {
        let operation = queries::tournament::TournamentQuery::build(
            queries::tournament::TournamentQueryVariables {
                tourney_slug: Some(&tournament_slug),
            },
        );

        let http_response = reqwest::Client::new()
            .post("https://api.start.gg/gql/alpha")
            .bearer_auth(&auth_token)
            .json(&operation)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed for {}: {}", tournament_slug, e))?;

        let body = http_response.text().await.map_err(|e| {
            format!(
                "Failed to read response body for {}: {}",
                tournament_slug, e
            )
        })?;

        let graphql_response: GraphQlResponse<queries::tournament::TournamentQuery> =
            serde_json::from_str(&body).map_err(|e| {
                format!(
                    "JSON parse error for {}: {}\nBody: {}",
                    tournament_slug, e, body
                )
            })?;

        if let Some(errors) = graphql_response.errors {
            return Err(format!(
                "GraphQL errors for {}: {:?}",
                tournament_slug, errors
            ));
        }

        let Some(data) = graphql_response.data else {
            return Err(format!("Missing GraphQL data for {}", tournament_slug));
        };

        let Some(tournament) = data.tournament else {
            return Err(format!(
                "Tournament not found for slug '{}'. Use a tournament slug like tournament/my-event.",
                tournament_slug
            ));
        };

        let events_missing = match tournament.events.as_ref() {
            None => true,
            Some(events) => events.is_empty(),
        };
        if events_missing {
            return Err(format!(
                "No events found for tournament slug '{}'.",
                tournament_slug
            ));
        }

        let slug_rows = process_tournament_response(&tournament_slug, tournament);
        if slug_rows.is_empty() {
            return Err(format!(
                "No entrant or set data found for tournament slug '{}'.",
                tournament_slug
            ));
        }

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
    tournamentNames: Vec<String>,
    authToken: String,
) -> Result<String, String> {
    match query_tournament_rows(tournamentNames, authToken).await {
        Ok(rows) => serde_json::to_string_pretty(&rows)
            .map_err(|e| format!("Failed to serialize rows as JSON: {}", e)),
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn get_tournament_rows_csv(
    tournamentNames: Vec<String>,
    authToken: String,
) -> Result<String, String> {
    match query_tournament_rows(tournamentNames, authToken).await {
        Ok(rows) => rows_to_csv(&rows).map_err(|e| format!("CSV export failed: {}", e)),
        Err(e) => Err(e),
    }
}

async fn query_tournament_json(tournament_name: String, auth_token: String) -> String {
    let operation = queries::tournament::TournamentQuery::build(
        queries::tournament::TournamentQueryVariables {
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

    let body = match http_response.text().await {
        Err(e) => {
            println!("Failed to read response body: {}", e);
            return format!("Failed to read response body: {}", e);
        }
        Ok(t) => t,
    };

    let graphql_response = match serde_json::from_str::<
        GraphQlResponse<queries::tournament::TournamentQuery>,
    >(&body)
    {
        Err(e) => {
            println!("JSON parse error: {}", e);
            return format!("JSON parse error: {}\nBody: {}", e, body);
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
