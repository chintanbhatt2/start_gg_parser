use crate::schema;
use cynic;

use super::scalars::StartggId;

// Variables for metadata query (slug only)
#[derive(cynic::QueryVariables, Debug)]
pub struct TournamentMetadataVariables<'a> {
    pub tourney_slug: Option<&'a str>,
}

// Variables for paginated data queries (slug + page + per_page)
#[derive(cynic::QueryVariables, Debug)]
pub struct TournamentDataVariables<'a> {
    pub tourney_slug: Option<&'a str>,
    pub page: i32,
    pub per_page: i32,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct EventEntrantsVariables {
    pub event_id: Option<StartggId>,
    pub page: i32,
    pub per_page: i32,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct EventStandingsVariables {
    pub event_id: Option<StartggId>,
    pub page: i32,
    pub per_page: i32,
}

// ============================================================================
// METADATA QUERY: Just tournament ID and event IDs/names/dates
// ============================================================================

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "TournamentMetadataVariables")]
pub struct TournamentMetadataQuery {
    #[arguments(slug: $tourney_slug)]
    pub tournament: Option<MetadataTournament>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentMetadataVariables")]
#[cynic(graphql_type = "Tournament")]
pub struct MetadataTournament {
    pub id: Option<StartggId>,
    pub events: Option<Vec<Option<MetadataEvent>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Event")]
pub struct MetadataEvent {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    pub start_at: Option<Timestamp>,
}

// ============================================================================
// STANDINGS QUERY: Isolated event standings fetch
// ============================================================================

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "StandingConnection")]
pub struct StandingsStandingConnection {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<StandingsStanding>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Standing")]
pub struct StandingsStanding {
    pub placement: Option<i32>,
    pub entrant: Option<Entrant>,
    pub player: Option<StandingsPlayer>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Player")]
pub struct StandingsPlayer {
    pub id: Option<StartggId>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "EventStandingsVariables")]
pub struct EventStandingsQuery {
    #[arguments(id: $event_id)]
    pub event: Option<EventStandingsEvent>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "EventStandingsVariables")]
#[cynic(graphql_type = "Event")]
pub struct EventStandingsEvent {
    pub id: Option<StartggId>,
    #[arguments(query: { page: $page, perPage: $per_page })]
    pub standings: Option<StandingsStandingConnection>,
}

// ============================================================================
// ENTRANTS QUERY: Isolated entrants fetch (no standings, no sets)
// ============================================================================

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "EventEntrantsVariables")]
pub struct EventEntrantsQuery {
    #[arguments(id: $event_id)]
    pub event: Option<EventEntrantsEvent>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "EventEntrantsVariables")]
#[cynic(graphql_type = "Event")]
pub struct EventEntrantsEvent {
    pub id: Option<StartggId>,
    #[arguments(query: { page: $page, perPage: $per_page })]
    pub entrants: Option<EventEntrantsEntrantConnection>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "EventEntrantsVariables")]
#[cynic(graphql_type = "EntrantConnection")]
pub struct EventEntrantsEntrantConnection {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<EntrantsEntrant>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Entrant")]
pub struct EntrantsEntrant {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    pub participants: Option<Vec<Option<EntrantsParticipant>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Participant")]
pub struct EntrantsParticipant {
    pub gamer_tag: Option<String>,
    pub prefix: Option<String>,
    pub user: Option<EntrantsUser>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "User")]
pub struct EntrantsUser {
    pub discriminator: Option<String>,
    #[arguments(types: [DISCORD])]
    pub authorizations: Option<Vec<Option<EntrantsAuthorization>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ProfileAuthorization")]
pub struct EntrantsAuthorization {
    pub external_username: Option<String>,
}

// ============================================================================
// SETS QUERY: Isolated sets fetch with slots (no standings, no entrants)
// ============================================================================

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "TournamentDataVariables")]
pub struct TournamentSetsQuery {
    #[arguments(slug: $tourney_slug)]
    pub tournament: Option<SetsTournament>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentDataVariables")]
#[cynic(graphql_type = "Tournament")]
pub struct SetsTournament {
    pub events: Option<Vec<Option<SetsEvent>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentDataVariables")]
#[cynic(graphql_type = "Event")]
pub struct SetsEvent {
    pub id: Option<StartggId>,
    #[arguments(page: $page, perPage: $per_page)]
    pub sets: Option<SetsSetConnection>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "SetConnection")]
pub struct SetsSetConnection {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<SetsSet>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Set")]
pub struct SetsSet {
    pub id: Option<StartggId>,
    pub winner_id: Option<i32>,
    pub slots: Option<Vec<Option<SetsSetSlot>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "SetSlot")]
pub struct SetsSetSlot {
    pub entrant: Option<Entrant>,
}

// ============================================================================
// SHARED FRAGMENTS
// ============================================================================

#[derive(cynic::QueryFragment, Debug)]
pub struct PageInfo {
    pub total_pages: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Entrant {
    pub id: Option<StartggId>,
}

#[derive(cynic::Scalar, Debug, Clone)]
pub struct Timestamp(pub i64);
