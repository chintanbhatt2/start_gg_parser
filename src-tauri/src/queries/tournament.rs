use crate::schema;
use cynic;

use super::scalars::StartggId;

#[derive(cynic::QueryVariables, Debug)]
pub struct TournamentQueryVariables<'a> {
    pub tourney_slug: Option<&'a str>,
    pub page: i32,
    pub per_page: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "TournamentQueryVariables")]
pub struct TournamentQuery {
    #[arguments(slug: $tourney_slug)]
    pub tournament: Option<Tournament>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "TournamentQueryVariables")]
pub struct TournamentEntrantsQuery {
    #[arguments(slug: $tourney_slug)]
    pub tournament: Option<EntrantsTournament>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentQueryVariables")]
#[cynic(graphql_type = "Tournament")]
pub struct EntrantsTournament {
    pub id: Option<StartggId>,
    pub events: Option<Vec<Option<EntrantsEvent>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentQueryVariables")]
#[cynic(graphql_type = "Event")]
pub struct EntrantsEvent {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    pub start_at: Option<Timestamp>,
    #[arguments(query: { page: $page, perPage: $per_page })]
    pub standings: Option<EntrantsStandingConnection>,
    #[arguments(query: { page: $page, perPage: $per_page })]
    pub entrants: Option<EntrantsEntrantConnection>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "StandingConnection")]
pub struct EntrantsStandingConnection {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<EntrantsStanding>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Standing")]
pub struct EntrantsStanding {
    pub placement: Option<i32>,
    pub entrant: Option<Entrant>,
    pub player: Option<Player>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentQueryVariables")]
#[cynic(graphql_type = "EntrantConnection")]
pub struct EntrantsEntrantConnection {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<EntrantsEntrant>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Entrant")]
#[cynic(variables = "TournamentQueryVariables")]
pub struct EntrantsEntrant {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    pub participants: Option<Vec<Option<EntrantParticipant>>>,
    #[arguments(page: $page, perPage: $per_page)]
    pub paginated_sets: Option<EntrantsSetConnection>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Participant")]
pub struct EntrantParticipant {
    pub user: Option<EntrantUser>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "User")]
pub struct EntrantUser {
    #[arguments(types: [DISCORD])]
    pub authorizations: Option<Vec<Option<EntrantAuthorization>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ProfileAuthorization")]
pub struct EntrantAuthorization {
    pub external_username: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "SetConnection")]
pub struct EntrantsSetConnection {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<Set2>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "TournamentQueryVariables")]
pub struct TournamentSetsQuery {
    #[arguments(slug: $tourney_slug)]
    pub tournament: Option<SetsTournament>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentQueryVariables")]
#[cynic(graphql_type = "Tournament")]
pub struct SetsTournament {
    pub events: Option<Vec<Option<SetsEvent>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentQueryVariables")]
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
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentQueryVariables")]
pub struct Tournament {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    pub events: Option<Vec<Option<Event>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentQueryVariables")]
pub struct Event {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    pub start_at: Option<Timestamp>,
    #[arguments(query: { page: $page, perPage: $per_page })]
    pub standings: Option<StandingConnection>,
    #[arguments(query: { page: $page, perPage: $per_page })]
    pub entrants: Option<EntrantConnection>,
    #[arguments(page: $page, perPage: $per_page)]
    pub sets: Option<SetConnection>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct SetConnection {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<Set>>>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Set {
    pub id: Option<StartggId>,
    pub winner_id: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct StandingConnection {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<Standing>>>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Standing {
    pub placement: Option<i32>,
    pub entrant: Option<Entrant>,
    pub player: Option<Player>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Player {
    pub id: Option<StartggId>,
    pub prefix: Option<String>,
    pub gamer_tag: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "TournamentQueryVariables")]
pub struct EntrantConnection {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<Entrant2>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Entrant")]
#[cynic(variables = "TournamentQueryVariables")]
pub struct Entrant2 {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    #[arguments(page: $page, perPage: $per_page)]
    pub paginated_sets: Option<SetConnection2>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "SetConnection")]
pub struct SetConnection2 {
    pub page_info: Option<PageInfo>,
    pub nodes: Option<Vec<Option<Set2>>>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct PageInfo {
    pub total_pages: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Set")]
pub struct Set2 {
    pub id: Option<StartggId>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Entrant {
    pub id: Option<StartggId>,
}

#[derive(cynic::Scalar, Debug, Clone)]
pub struct Timestamp(pub i64);
