//! Lighting, as a contributor to the interaction seam.
//!
//! Spec 030, US3. A lever on a wall turns the lamps in a room on or off for
//! everyone at the table. The declaration lives in
//! `thunderforge_canvas_core::lighting`; this moves the rows.
//!
//! # A light is off when its intensity is zero
//!
//! Which means switching one off would destroy the only record of how bright
//! it had been, and a lever pulled twice would leave the room at a brightness
//! nobody chose. So the prior value is stashed in the light's existing
//! metadata and restored on the way back — bookkeeping for one feature, not a
//! new property of a light, and therefore not a new column.
//!
//! # A deleted light does not make a switch dead
//!
//! A Game Master who deletes one of five lamps has a switch that should still
//! work on the other four, and should be *told* about the fifth. Reporting the
//! missing one while performing the rest is the behaviour US3's third scenario
//! asks for; silently doing nothing is what makes a GM think the whole switch
//! is broken.

use diesel::prelude::*;
use uuid::Uuid;

use thunderforge_canvas_core::lighting::{
    PRIOR_INTENSITY_KEY, TOGGLE, intensity_to_restore, lights_of, requested_on,
};

/// Whether this module declared the effect.
pub fn handles(effect_id: &str) -> bool {
    effect_id == TOGGLE
}

/// What one `light.toggle` did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Switched {
    /// Lights that changed.
    pub changed: Vec<Uuid>,
    /// Lights the configuration named that are no longer in the scene.
    ///
    /// Reported rather than pruned. Pruning would edit a Game Master's
    /// authored configuration as a side effect of a player's click, and they
    /// might be about to put the lamp back.
    pub missing: Vec<String>,
}

/// Perform a `light.toggle`, authoritatively.
pub fn perform(
    conn: &mut PgConnection,
    effect_id: &str,
    config: &serde_json::Value,
    scene_id: Uuid,
) -> Result<Switched, diesel::result::Error> {
    if !handles(effect_id) {
        return Ok(Switched::default());
    }

    let mut result = Switched::default();
    let named = lights_of(config);
    if named.is_empty() {
        return Ok(result);
    }

    conn.transaction(|conn| {
        use crate::schema::light_sources as l;

        for reference in named {
            let Ok(light_id) = Uuid::parse_str(reference) else {
                result.missing.push(reference.to_string());
                continue;
            };

            // Scoped to the interactive's own scene. A stored configuration
            // must not reach a lamp in another scene, whatever it claims.
            //
            // Locked, because `toggle` reads the current state and writes the
            // other one: two players hitting one switch at the same moment
            // must resolve to one answer rather than interleaving into a state
            // neither asked for.
            let existing: Option<(f64, Option<serde_json::Value>)> = l::table
                .filter(l::light_id.eq(light_id))
                .filter(l::scene_id.eq(scene_id))
                .select((l::intensity, l::metadata))
                .for_update()
                .first(conn)
                .optional()?;

            let Some((intensity, metadata)) = existing else {
                result.missing.push(reference.to_string());
                continue;
            };

            let currently_on = intensity > 0.0;
            let Some(want_on) = requested_on(config, currently_on) else {
                // A mode nothing recognises. Reported as nothing done rather
                // than as an arbitrary choice.
                continue;
            };
            if want_on == currently_on {
                continue;
            }

            let now = chrono::Utc::now().naive_utc();
            if want_on {
                let restored = intensity_to_restore(metadata.as_ref());
                diesel::update(l::table.filter(l::light_id.eq(light_id)))
                    .set((l::intensity.eq(f64::from(restored)), l::updated_at.eq(now)))
                    .execute(conn)?;
            } else {
                let mut next = match metadata {
                    Some(serde_json::Value::Object(map)) => map,
                    _ => serde_json::Map::new(),
                };
                next.insert(
                    PRIOR_INTENSITY_KEY.to_string(),
                    serde_json::json!(intensity),
                );
                diesel::update(l::table.filter(l::light_id.eq(light_id)))
                    .set((
                        l::intensity.eq(0.0f64),
                        l::metadata.eq(serde_json::Value::Object(next)),
                        l::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }
            result.changed.push(light_id);
        }
        Ok::<(), diesel::result::Error>(())
    })?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_module_claims_exactly_what_it_declared() {
        // If these disagree, an authored effect validates and then nothing
        // performs it — the silent failure the seam exists to avoid, arriving
        // from the one direction the registry cannot catch.
        for declaration in thunderforge_canvas_core::lighting::interaction_effects() {
            assert!(
                handles(&declaration.id),
                "{} is declared but not performed",
                declaration.id
            );
        }
    }

    #[test]
    fn nothing_else_is_claimed() {
        assert!(!handles(thunderforge_canvas_core::wall::SET_STATE));
        assert!(!handles("audio.play"));
    }
}
