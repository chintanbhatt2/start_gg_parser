use crate::queries;
use crate::types::{EventAccumulator, MutableTournamentRow, TournamentRow};
use crate::utils::{id_to_string, split_prefix_and_name, winner_id_to_string};
use std::collections::HashMap;

fn join_non_empty(values: impl IntoIterator<Item = String>, separator: &str) -> String {
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(separator)
}

impl EventAccumulator {
    pub fn new(
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
            wins_losses: HashMap::new(),
        }
    }

    fn absorb_standings_connection(
        &mut self,
        standings: Option<queries::tournament::StandingsStandingConnection>,
    ) -> bool {
        let mut changed = false;

        if let Some(standing_nodes) = standings.and_then(|standings| standings.nodes) {
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

                let was_known_entrant = self.row_by_entrant_id.contains_key(&entrant_id);
                self.row_by_entrant_id
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
                        player_name: String::new(),
                        player_prefix: String::new(),
                        player_discriminator: String::new(),
                        discord_usernames: String::new(),
                    });

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

                if !was_known_entrant {
                    changed = true;
                }
            }
        }

        changed
    }

    pub fn absorb_event_standings_page(
        &mut self,
        event: queries::tournament::EventStandingsEvent,
    ) -> bool {
        self.absorb_standings_connection(event.standings)
    }

    pub fn absorb_event_entrants_page(
        &mut self,
        event: queries::tournament::EventEntrantsEvent,
    ) -> bool {
        self.absorb_event_entrants_connection(event.entrants)
    }

    fn absorb_event_entrants_connection(
        &mut self,
        entrants: Option<queries::tournament::EventEntrantsEntrantConnection>,
    ) -> bool {
        let entrant_nodes = entrants
            .as_ref()
            .and_then(|entrants| entrants.nodes.as_ref());
        self.absorb_entrant_nodes(entrant_nodes)
    }

    fn absorb_entrant_nodes(
        &mut self,
        entrant_nodes: Option<&Vec<Option<queries::tournament::EntrantsEntrant>>>,
    ) -> bool {
        let mut changed = false;

        if let Some(entrant_nodes) = entrant_nodes {
            for maybe_entrant in entrant_nodes {
                let Some(entrant) = maybe_entrant else {
                    continue;
                };

                let Some(entrant_id) = entrant.id.as_ref().map(id_to_string) else {
                    continue;
                };

                let entrant_name = entrant.name.clone().unwrap_or_default();
                let (entrant_prefix, entrant_player_name) = split_prefix_and_name(&entrant_name);

                let participants = entrant.participants.as_deref().unwrap_or(&[]);
                let participant_prefixes = participants.iter().filter_map(|participant| {
                    participant
                        .as_ref()
                        .and_then(|participant| participant.prefix.clone())
                        .map(|prefix| prefix.trim().to_string())
                        .filter(|prefix| !prefix.is_empty())
                });
                let participant_names = participants.iter().filter_map(|participant| {
                    participant
                        .as_ref()
                        .and_then(|participant| participant.gamer_tag.clone())
                        .map(|name| name.trim().to_string())
                        .filter(|name| !name.is_empty())
                });
                let player_discriminators = participants.iter().filter_map(|participant| {
                    participant
                        .as_ref()
                        .and_then(|participant| participant.user.as_ref())
                        .and_then(|user| user.discriminator.clone())
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                });
                let discord_usernames = participants.iter().flat_map(|participant| {
                    participant
                        .as_ref()
                        .and_then(|participant| participant.user.as_ref())
                        .and_then(|user| user.authorizations.as_ref())
                        .into_iter()
                        .flatten()
                        .filter_map(|authorization| {
                            authorization
                                .as_ref()
                                .and_then(|authorization| authorization.external_username.clone())
                                .map(|username| username.trim().to_string())
                                .filter(|username| !username.is_empty())
                        })
                });

                let player_prefix = {
                    let joined = join_non_empty(participant_prefixes, " | ");
                    if joined.is_empty() {
                        entrant_prefix.clone()
                    } else {
                        joined
                    }
                };
                let player_name = {
                    let joined = join_non_empty(participant_names, " / ");
                    if joined.is_empty() {
                        entrant_player_name.clone()
                    } else {
                        joined
                    }
                };
                let player_discriminator = join_non_empty(player_discriminators, " | ");
                let discord_usernames = join_non_empty(discord_usernames, " | ");

                let was_known_entrant = self.row_by_entrant_id.contains_key(&entrant_id);
                self.row_by_entrant_id
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
                        player_name: player_name.clone(),
                        player_prefix: player_prefix.clone(),
                        player_discriminator: player_discriminator.clone(),
                        discord_usernames: discord_usernames.clone(),
                    });

                if let Some(row) = self.row_by_entrant_id.get_mut(&entrant_id) {
                    let mut row_changed = false;

                    if !player_name.is_empty() && row.player_name != player_name {
                        row.player_name = player_name.clone();
                        row_changed = true;
                    }
                    if !player_prefix.is_empty() && row.player_prefix != player_prefix {
                        row.player_prefix = player_prefix.clone();
                        row_changed = true;
                    }
                    if !player_discriminator.is_empty()
                        && row.player_discriminator != player_discriminator
                    {
                        row.player_discriminator = player_discriminator.clone();
                        row_changed = true;
                    }
                    if !discord_usernames.is_empty() && row.discord_usernames != discord_usernames {
                        row.discord_usernames = discord_usernames.clone();
                        row_changed = true;
                    }

                    changed |= row_changed;
                }

                if !was_known_entrant {
                    changed = true;
                }
            }
        }

        changed
    }

    pub fn absorb_sets_page(&mut self, event: queries::tournament::SetsEvent) -> bool {
        let mut changed = false;

        if let Some(set_nodes) = event.sets.and_then(|sets| sets.nodes) {
            for maybe_set in set_nodes {
                let Some(set_item) = maybe_set else {
                    continue;
                };

                let (Some(_set_id), Some(winner_id)) = (set_item.id, set_item.winner_id) else {
                    continue;
                };

                let winner_id_str = winner_id_to_string(winner_id);

                let Some(slots) = set_item.slots else {
                    continue;
                };

                for maybe_slot in slots {
                    let Some(slot) = maybe_slot else {
                        continue;
                    };
                    let Some(entrant) = slot.entrant else {
                        continue;
                    };
                    let Some(entrant_id) = entrant.id.as_ref().map(id_to_string) else {
                        continue;
                    };

                    let entry = self.wins_losses.entry(entrant_id.clone()).or_default();
                    if entrant_id == winner_id_str {
                        entry.0 += 1;
                    } else {
                        entry.1 += 1;
                    }
                    changed = true;
                }
            }
        }

        changed
    }

    pub fn into_rows(mut self) -> Vec<TournamentRow> {
        for (entrant_id, row) in &mut self.row_by_entrant_id {
            if let Some((wins, losses)) = self.wins_losses.get(entrant_id) {
                row.wins = *wins;
                row.losses = *losses;
            }
        }

        self.row_by_entrant_id
            .into_values()
            .map(TournamentRow::from)
            .collect()
    }
}
