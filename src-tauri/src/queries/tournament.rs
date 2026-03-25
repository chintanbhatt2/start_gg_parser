use crate::schema;
use cynic;

use super::scalars::StartggId;

#[derive(cynic::QueryVariables, Debug)]
pub struct TournamentQueryVariables<'a> {
    pub tourney_slug: Option<&'a str>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "TournamentQueryVariables")]
pub struct TournamentQuery {
    #[arguments(slug: $tourney_slug)]
    pub tournament: Option<Tournament>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Tournament {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    pub events: Option<Vec<Option<Event>>>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Event {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    pub start_at: Option<Timestamp>,
    #[arguments(query: { page: 1, perPage: 500 })]
    pub standings: Option<StandingConnection>,
    #[arguments(query: { page: 1, perPage: 500 })]
    pub entrants: Option<EntrantConnection>,
    #[arguments(page: 1, perPage: 500)]
    pub sets: Option<SetConnection>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct SetConnection {
    pub nodes: Option<Vec<Option<Set>>>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Set {
    pub id: Option<StartggId>,
    pub winner_id: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct StandingConnection {
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
pub struct EntrantConnection {
    pub nodes: Option<Vec<Option<Entrant2>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Entrant")]
pub struct Entrant2 {
    pub id: Option<StartggId>,
    pub name: Option<String>,
    #[arguments(page: 1, perPage: 500)]
    pub paginated_sets: Option<SetConnection2>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "SetConnection")]
pub struct SetConnection2 {
    pub nodes: Option<Vec<Option<Set2>>>,
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
