//! PC-only shared objectives. The script owns one group and one progression result; each
//! skater keeps its own sequence, trigger and Lua-vars history. Only completion, points and
//! objective flow are shared. No mission event is rewritten or delivered twice.

use super::{
    keys::hash64,
    objectives::ObjectiveGroup,
    system::{Event, Field, World},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub struct CoopObjectives {
    teams: Vec<[i32; 2]>,
    partners: BTreeMap<u32, ObjectiveGroup>,
    failures: BTreeMap<(u32, usize), BTreeSet<i32>>,
}

impl CoopObjectives {
    /// The primary owns the script's group, HUD and save. An absent partner disables sharing.
    pub fn set_teams(&mut self, teams: Vec<[i32; 2]>) {
        self.teams = teams;
    }

    fn partner(&self, player: i32) -> Option<i32> {
        self.teams.iter().find(|team| team[0] == player).map(|team| team[1])
    }
}

pub(super) fn reset_group(world: &mut World, group: u32) {
    if let Some(partner) = world.objectives.coop.partners.remove(&group) {
        world.objective_vars_cleared.extend(partner.objectives.iter().map(|o| o.unique_id));
    }
    world.objectives.coop.failures.retain(|&(id, _), _| id != group);
}

pub(super) fn prepare(world: &mut World) {
    let removed: Vec<_> = world.objectives.coop.partners.keys().copied().filter(|&id| {
        world.objectives.group_index(id).is_none_or(|gi| {
            let group = &world.objectives.groups[gi];
            world.objectives.coop.partner(group.player).is_none()
                || group.objectives.len() != world.objectives.coop.partners[&id].objectives.len()
        })
    }).collect();
    for id in removed {
        reset_group(world, id);
    }
    let groups: Vec<_> = world.objectives.groups.iter().filter_map(|group| {
        let partner = world.objectives.coop.partner(group.player)?;
        (!world.objectives.coop.partners.contains_key(&group.id)).then_some((group.clone(), partner))
    }).collect();
    for (mut group, partner) in groups {
        group.player = partner;
        group.sequence_completed_tricks.clear();
        group.last_completed_unique_trick = None;
        for objective in &mut group.objectives {
            world.objectives.next_coop_instance(objective, partner);
            objective.trigger_volumes.clear();
            objective.sequence_volumes.clear();
            objective.completed_tricks.clear();
            objective.completed_grinds.clear();
            objective.last_sequence_points = 0;
            objective.grind_length = 0.0;
            objective.total_grind_length_in_sequence = 0.0;
            objective.max_grind_length_in_sequence = 0.0;
            objective.just_set_session_marker = false;
            objective.just_used_session_marker = false;
            objective.num_pushes = 0;
            objective.award_sequence_points_immediately = false;
            objective.data.reset(true);
        }
        world.objectives.coop.partners.insert(group.id, group);
    }
}

/// Keep transient sequence data and Lua variables private, even when both skaters land in
/// the same tick. The canonical group handle stays unchanged for progression callbacks.
fn share(from: &ObjectiveGroup, to: &mut ObjectiveGroup) {
    to.evaluate = from.evaluate;
    to.points_requirement = from.points_requirement;
    to.completed_unique_tricks.clone_from(&from.completed_unique_tricks);
    for (source, target) in from.objectives.iter().zip(&mut to.objectives) {
        target.state = source.state;
        target.total_points = source.total_points;
        target.total_grind_length = source.total_grind_length;
        target.notify_window = source.notify_window;
        target.dirty |= source.dirty;
        target.title.clone_from(&source.title);
    }
}

pub(super) fn enter_partner(world: &mut World, group: u32) -> Option<ObjectiveGroup> {
    let gi = world.objectives.group_index(group)?;
    let mut partner = world.objectives.coop.partners.remove(&group)?;
    share(&world.objectives.groups[gi], &mut partner);
    Some(std::mem::replace(&mut world.objectives.groups[gi], partner))
}

pub(super) fn leave_partner(world: &mut World, mut primary: ObjectiveGroup) {
    let Some(gi) = world.objectives.group_index(primary.id) else { return };
    share(&world.objectives.groups[gi], &mut primary);
    let partner = std::mem::replace(&mut world.objectives.groups[gi], primary);
    world.objectives.coop.partners.insert(partner.id, partner);
}

pub(super) fn enter_event(world: &mut World, event: &Event) -> Vec<ObjectiveGroup> {
    let player = match event {
        Event::TriggerCollision { player, .. } => Some(*player),
        Event::Challenge(message) => message.fields.as_ref().and_then(|fields| fields.iter().find_map(|(name, value)| {
            if *name == "PlayerID" && let Field::Number(player) = value {
                #[expect(clippy::cast_possible_truncation, reason = "mission message player index")]
                return Some(*player as i32);
            }
            None
        })),
        _ => None,
    };
    let Some(player) = player else { return Vec::new() };
    let groups: Vec<_> = world.objectives.groups.iter().filter(|group| {
        world.objectives.coop.partner(group.player) == Some(player)
    }).map(|group| group.id).collect();
    groups.into_iter().filter_map(|id| enter_partner(world, id)).collect()
}

/// A failure is published only while both partners' most recent evaluations fail. Global
/// timers therefore still expire once; one skater's wipeout alone cannot fail the team.
pub(super) fn failure(world: &mut World, group: u32, index: usize, failed: bool) -> bool {
    let Some(gi) = world.objectives.group_index(group) else { return failed };
    let player = world.objectives.groups[gi].player;
    world.objectives.coop.team_failed(group, index, player, failed)
}

impl CoopObjectives {
    /// Records `player`'s latest failure evaluation of a group's objective and returns whether
    /// the failure stands: a player outside every team fails alone.
    fn team_failed(&mut self, group: u32, index: usize, player: i32, failed: bool) -> bool {
        let Some(team) = self.teams.iter().find(|team| team.contains(&player)).copied() else {
            return failed;
        };
        let failures = self.failures.entry((group, index)).or_default();
        if failed {
            failures.insert(player);
        } else {
            failures.remove(&player);
        }
        failed && team.iter().all(|member| failures.contains(member))
    }
}

impl super::objectives::ObjectivesManager {
    fn next_coop_instance(&mut self, objective: &mut super::objectives::Objective, player: i32) {
        self.next_instance += 1;
        objective.unique_id = hash64(&format!("coop-{:016x}-{}", objective.key, self.next_instance));
        objective.player = player;
        objective.vars_in_vm = false;
    }
}

#[cfg(test)]
mod tests {
    use super::CoopObjectives;

    #[test]
    fn a_team_fails_only_once_both_partners_fail() {
        let mut coop = CoopObjectives::default();
        coop.set_teams(vec![[0, 1]]);
        assert!(!coop.team_failed(7, 0, 0, true), "the first player's wipeout alone");
        assert!(coop.team_failed(7, 0, 1, true), "the partner fails too");
        assert!(!coop.team_failed(7, 0, 0, false), "a passing evaluation never fails");
        assert!(!coop.team_failed(7, 0, 1, true), "the first player recovered");
        assert!(!coop.team_failed(7, 1, 0, true), "objectives keep their own failures");
        assert!(!coop.team_failed(8, 0, 0, true), "groups keep their own failures");
    }

    #[test]
    fn a_player_outside_every_team_fails_alone() {
        let mut coop = CoopObjectives::default();
        assert!(coop.team_failed(7, 0, 0, true));
        coop.set_teams(vec![[0, 1]]);
        assert!(coop.team_failed(7, 0, 2, true));
        assert_eq!(coop.partner(0), Some(1));
        assert_eq!(coop.partner(1), None, "the partner's group is never primary");
    }
}
