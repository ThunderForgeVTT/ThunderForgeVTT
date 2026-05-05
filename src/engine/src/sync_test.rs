//! Circular Flow Testing & Logging System
//!
//! Provides end-to-end tracing of the circular data flow:
//! 1. Local keyboard input → Movement request
//! 2. Send mutation to server
//! 3. Server validates and persists
//! 4. Server broadcasts via pg_notify
//! 5. WebSocket receives worldEventCreated
//! 6. Client applies server event or rolls back

use bevy::prelude::*;
use crate::components::*;

/// Circular flow stage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowStage {
    /// User input detected
    LocalInput,
    /// Mutation being sent to server
    MutationSent,
    /// Server acknowledged mutation
    ServerReceived,
    /// Database persisted event
    EventPersisted,
    /// PostgreSQL NOTIFY triggered
    NotifyTriggered,
    /// WebSocket received subscription
    SubscriptionReceived,
    /// Client validating server state
    Validating,
    /// Optimistic update confirmed
    Confirmed,
    /// Mutation rejected, rolling back
    RolledBack,
}

/// Trace event for circular flow
#[derive(Debug, Clone)]
pub struct FlowTrace {
    pub stage: FlowStage,
    pub token_id: String,
    pub timestamp: f64,
    pub message: String,
}

/// Resource for tracking circular flows
#[derive(Resource, Default)]
pub struct CircularFlowTracer {
    traces: Vec<FlowTrace>,
    max_traces: usize,
}

impl CircularFlowTracer {
    pub fn new() -> Self {
        Self {
            traces: Vec::new(),
            max_traces: 1000,
        }
    }

    /// Record a trace event
    pub fn trace(&mut self, stage: FlowStage, token_id: String, message: String, time: f64) {
        if self.traces.len() >= self.max_traces {
            self.traces.remove(0);
        }

        let trace = FlowTrace {
            stage,
            token_id,
            timestamp: time,
            message,
        };

        self.traces.push(trace.clone());
        self.log_trace(&trace);
    }

    /// Log a trace event
    fn log_trace(&self, trace: &FlowTrace) {
        eprintln!(
            "[{:.2}] [{}] [{}] Token: {} - {}",
            trace.timestamp,
            stage_name(trace.stage),
            stage_color(trace.stage),
            trace.token_id,
            trace.message
        );
    }

    /// Get all traces
    pub fn get_traces(&self) -> &[FlowTrace] {
        &self.traces
    }

    /// Get traces for a specific token
    pub fn get_token_traces(&self, token_id: &str) -> Vec<FlowTrace> {
        self.traces
            .iter()
            .filter(|t| t.token_id == token_id)
            .cloned()
            .collect()
    }

    /// Print a summary of the circular flow
    pub fn print_summary(&self) {
        eprintln!("\n=== Circular Flow Summary ===");
        eprintln!("Total traces: {}", self.traces.len());

        let mut token_traces: std::collections::HashMap<String, Vec<_>> =
            std::collections::HashMap::new();
        for trace in &self.traces {
            token_traces
                .entry(trace.token_id.clone())
                .or_default()
                .push(trace.clone());
        }

        for (token_id, traces) in token_traces {
            eprintln!("\nToken: {}", token_id);
            for trace in traces {
                eprintln!("  → [{}] {}", stage_name(trace.stage), trace.message);
            }
        }
        eprintln!("============================\n");
    }
}

fn stage_name(stage: FlowStage) -> &'static str {
    match stage {
        FlowStage::LocalInput => "INPUT",
        FlowStage::MutationSent => "SEND",
        FlowStage::ServerReceived => "RECV",
        FlowStage::EventPersisted => "SAVE",
        FlowStage::NotifyTriggered => "NOTIFY",
        FlowStage::SubscriptionReceived => "SUB",
        FlowStage::Validating => "VALIDATE",
        FlowStage::Confirmed => "OK",
        FlowStage::RolledBack => "ROLLBACK",
    }
}

fn stage_color(stage: FlowStage) -> &'static str {
    match stage {
        FlowStage::LocalInput => "🎮",
        FlowStage::MutationSent => "📤",
        FlowStage::ServerReceived => "📥",
        FlowStage::EventPersisted => "💾",
        FlowStage::NotifyTriggered => "🔔",
        FlowStage::SubscriptionReceived => "📡",
        FlowStage::Validating => "🔍",
        FlowStage::Confirmed => "✅",
        FlowStage::RolledBack => "❌",
    }
}

/// System to trace keyboard input
pub fn trace_keyboard_input(
    mut tracer: ResMut<CircularFlowTracer>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    query: Query<&TokenId, With<crate::movement::PlayerControlled>>,
) {
    if keyboard_input.any_pressed([
        KeyCode::KeyW,
        KeyCode::KeyS,
        KeyCode::KeyA,
        KeyCode::KeyD,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
    ]) {
        for token_id in query.iter() {
            tracer.trace(
                FlowStage::LocalInput,
                token_id.0.clone(),
                "Keyboard input detected".to_string(),
                time.elapsed_secs() as f64,
            );
        }
    }
}

/// System to trace mutation sends
pub fn trace_mutation_sent(
    mut tracer: ResMut<CircularFlowTracer>,
    mut query: Query<(&Token, &GridPosition, &TokenId), Changed<GridPosition>>,
    time: Res<Time>,
) {
    for (_token, grid_pos, token_id) in query.iter_mut() {
        tracer.trace(
            FlowStage::MutationSent,
            token_id.0.clone(),
            format!(
                "Mutation sent to server: x={:.1}, y={:.1}, z={:.1}",
                grid_pos.x, grid_pos.y, grid_pos.z
            ),
            time.elapsed_secs() as f64,
        );
    }
}

/// System to trace server events
pub fn trace_server_event(
    _tracer: ResMut<CircularFlowTracer>,
    _time: Res<Time>,
) {
    // Phase 4.3: Actual event reading deferred to Phase 4.4
    // Placeholder for now
}

/// System to trace optimistic update confirmations
pub fn trace_update_confirmation(
    mut tracer: ResMut<CircularFlowTracer>,
    time: Res<Time>,
    mut query: Query<(&Token, &RollbackCache, &TokenId), Changed<RollbackCache>>,
) {
    for (_token, cache, token_id) in query.iter_mut() {
        if !cache.is_pending {
            tracer.trace(
                FlowStage::Confirmed,
                token_id.0.clone(),
                format!(
                    "Optimistic update confirmed: server pos=({:.1}, {:.1})",
                    cache.last_server_position.x, cache.last_server_position.y
                ),
                time.elapsed_secs() as f64,
            );
        }
    }
}

/// System to trace rollbacks
pub fn trace_rollback(
    mut tracer: ResMut<CircularFlowTracer>,
    time: Res<Time>,
    mut query: Query<(&GridPosition, &RollbackCache, &TokenId), Changed<GridPosition>>,
) {
    for (grid_pos, cache, token_id) in query.iter_mut() {
        if grid_pos.distance_to(cache.last_server_position) > 0.1 {
            tracer.trace(
                FlowStage::RolledBack,
                token_id.0.clone(),
                format!(
                    "Rollback triggered: local=({:.1}, {:.1}), server=({:.1}, {:.1})",
                    grid_pos.x,
                    grid_pos.y,
                    cache.last_server_position.x,
                    cache.last_server_position.y
                ),
                time.elapsed_secs() as f64,
            );
        }
    }
}

/// System to print periodic summaries
pub fn print_flow_summary(
    tracer: Res<CircularFlowTracer>,
    mut timer: Local<f32>,
    time: Res<Time>,
) {
    *timer += time.delta_secs();

    if *timer > 10.0 {
        *timer = 0.0;
        tracer.print_summary();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_name() {
        assert_eq!(stage_name(FlowStage::LocalInput), "INPUT");
        assert_eq!(stage_name(FlowStage::MutationSent), "SEND");
        assert_eq!(stage_name(FlowStage::Confirmed), "OK");
    }

    #[test]
    fn test_tracer_creation() {
        let tracer = CircularFlowTracer::new();
        assert!(tracer.get_traces().is_empty());
    }

    #[test]
    fn test_trace_recording() {
        let mut tracer = CircularFlowTracer::new();
        tracer.trace(
            FlowStage::LocalInput,
            "token1".to_string(),
            "Test message".to_string(),
            1.0,
        );

        assert_eq!(tracer.get_traces().len(), 1);
        assert_eq!(tracer.get_traces()[0].stage, FlowStage::LocalInput);
    }

    #[test]
    fn test_get_token_traces() {
        let mut tracer = CircularFlowTracer::new();
        tracer.trace(
            FlowStage::LocalInput,
            "token1".to_string(),
            "Input".to_string(),
            1.0,
        );
        tracer.trace(
            FlowStage::MutationSent,
            "token1".to_string(),
            "Send".to_string(),
            2.0,
        );
        tracer.trace(
            FlowStage::LocalInput,
            "token2".to_string(),
            "Input".to_string(),
            3.0,
        );

        let token1_traces = tracer.get_token_traces("token1");
        assert_eq!(token1_traces.len(), 2);

        let token2_traces = tracer.get_token_traces("token2");
        assert_eq!(token2_traces.len(), 1);
    }
}
