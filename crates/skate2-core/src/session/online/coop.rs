//! PC co-op: a second local skater travels over its owner's authenticated connection.
//! It has a separate replay stream, not another lobby member or challenge barrier.

use super::{RemoteSkater, SelectedLiveError, SelectedLiveSession, error, net_header, remote_skater};
use crate::two_player_session::PLAYER2_INDEX;
use skate2_net::{frames::{INTERVAL, MAX_RUN, NetFrame}, protocol::ClientMessage};
use std::sync::Arc;

impl<T, C> SelectedLiveSession<T, C> {
    pub(super) fn online_companion_recipe(&mut self, user_id: u64, data: Vec<u8>) -> Result<(), SelectedLiveError> {
        let Some(game) = self.online.game.as_mut() else { return Ok(()) };
        let Some(player) = game.players.iter().find(|player| player.user_id == user_id) else {
            return Err(error("companion recipe without a lobby member"));
        };
        if Some(user_id) == self.online.user_id {
            return Ok(());
        }
        if data.is_empty() {
            game.companions.remove(&user_id);
            game.companion_recipes.remove(&user_id);
            return Ok(());
        }
        if !game.entered {
            game.companion_recipes.insert(user_id, data);
            return Ok(());
        }
        let remote = game.companions.entry(user_id).or_insert_with(|| RemoteSkater::new(user_id, player.slot + 8));
        let assets = self.inner.assets.as_ref().ok_or_else(|| error("companion assets absent"))?;
        remote.build_character(data, Arc::clone(&assets.mounts), Arc::clone(&assets.vault), assets.cvars.clone()).map_err(error)
    }

    /// The companion's replay frames go out as the console's own do outside a played round: a
    /// header every tick, a pose at each run's end (every `INTERVAL` ticks, at a teleport and
    /// the tick after it). It carries no camera, audio record or HUD.
    pub(super) fn online_coop_tick(&mut self) -> Result<(), SelectedLiveError> {
        let Some(game) = self.online.game.as_ref().filter(|game| game.entered) else { return Ok(()) };
        if !self.two_player.is_player2_spawned() {
            if game.companion_sent {
                self.online.send(ClientMessage::CompanionRecipe(Vec::new()));
                if let Some(game) = self.online.game.as_mut() {
                    game.companion_sent = false;
                    game.companion_run.clear();
                    game.companion_teleport = false;
                }
            }
            return Ok(());
        }
        if self.map_travel.is_some() {
            return Ok(());
        }
        let tick = self.completed_frames;
        let assets = self.inner.assets.as_ref().ok_or_else(|| error("companion assets absent"))?;
        let vault = Arc::clone(&assets.vault);
        let Some(skater) = self.inner.party.iter_mut().find(|skater| skater.player_index() == PLAYER2_INDEX) else { return Ok(()) };
        let Some(frame) = skater.presentation_runtime.submitted_frame(&skater.presentation, &vault).map_err(error)? else { return Ok(()) };
        let game = self.online.game.as_mut().expect("online game");
        let Some(client) = self.online.client.as_ref() else { return Ok(()) };
        if !game.companion_sent && game.companion_sent_at.is_none_or(|at| at.elapsed() >= super::RECIPE_SPACING) {
            client.send(ClientMessage::CompanionRecipe(remote_skater::encode_character(&frame).map_err(error)?));
            game.companion_sent = true;
            game.companion_sent_at = Some(std::time::Instant::now());
        }
        let header = net_header(&frame.header);
        let teleport = header[48] & 0x1000 != 0;
        let ends = tick % INTERVAL == 0 || teleport || game.companion_teleport;
        game.companion_teleport = teleport;
        let pose = ends.then(|| frame.pose.iter().map(|sqt| {
            let mut lanes = [0.0; 12];
            lanes[..4].copy_from_slice(&sqt.scale);
            lanes[4..8].copy_from_slice(&sqt.rotation);
            lanes[8..].copy_from_slice(&sqt.translation);
            lanes
        }).collect());
        game.companion_run.push(NetFrame {
            tick,
            header,
            pose,
            twist: frame.twist_ratios,
            camera: Vec::new(),
            audio_floats: Vec::new(),
            audio_words: Vec::new(),
            hud: Vec::new(),
        });
        if ends || game.companion_run.len() == MAX_RUN {
            client.send_companion_frames(std::mem::take(&mut game.companion_run));
        }
        Ok(())
    }
}
