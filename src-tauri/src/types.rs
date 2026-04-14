use std::collections::HashMap;

#[derive(Debug, serde::Serialize)]
pub struct TournamentRow {
    pub tournament_slug: String,
    pub tournament_id: Option<String>,
    pub event_id: Option<String>,
    pub event_name: String,
    pub event_date: Option<String>,
    pub entrant_id: String,
    pub player_id: Option<String>,
    pub placement: Option<i32>,
    pub wins: i32,
    pub losses: i32,
    pub player_name: String,
    pub player_prefix: String,
    pub player_discriminator: String,
    pub discord_usernames: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    pub tournament_slug: String,
    pub page: i32,
    pub message: String,
    pub done: bool,
}

#[derive(Debug, Default)]
pub struct MutableTournamentRow {
    pub tournament_slug: String,
    pub tournament_id: Option<String>,
    pub event_id: Option<String>,
    pub event_name: String,
    pub event_date: Option<String>,
    pub entrant_id: String,
    pub player_id: Option<String>,
    pub placement: Option<i32>,
    pub wins: i32,
    pub losses: i32,
    pub player_name: String,
    pub player_prefix: String,
    pub player_discriminator: String,
    pub discord_usernames: String,
}

#[derive(Debug)]
pub struct EventAccumulator {
    pub tournament_slug: String,
    pub tournament_id: Option<String>,
    pub event_id: Option<String>,
    pub event_name: String,
    pub event_date: Option<String>,
    pub row_by_entrant_id: HashMap<String, MutableTournamentRow>,
    pub wins_losses: HashMap<String, (i32, i32)>,
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
            player_discriminator: value.player_discriminator,
            discord_usernames: value.discord_usernames,
        }
    }
}
