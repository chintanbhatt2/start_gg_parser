mod queries;

use cynic::{GraphQlResponse, QueryBuilder};

#[cynic::schema("startgg")]
mod schema {}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn get_tournament_json(tournamentName: String, authToken: String) -> String {
    println!("Received tournament name: {}", tournamentName);
    query_tournament_json(tournamentName, authToken).await
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
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, get_tournament_json])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
