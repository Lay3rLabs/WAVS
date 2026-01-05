//! Rerun visualization for WAVS network packet flow.
//!
//! This module provides visualization of packet flow between WAVS nodes
//! using animated 2D points that move between nodes.

use once_cell::sync::OnceCell;
use parking_lot::{Mutex, RwLock};
use rerun::{Color, LineStrips2D, Points2D, RecordingStream, TextLog};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global recorder instance shared across the application.
static RECORDER: OnceCell<RwLock<Option<RecordingStream>>> = OnceCell::new();

/// Frame counter for animation timing
static FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Track seen operators for dynamic node creation
static SEEN_OPERATORS: OnceCell<Mutex<HashSet<String>>> = OnceCell::new();

// Node IDs for the network graph
pub const NODE_TRIGGER: &str = "trigger_manager";
pub const NODE_DISPATCHER: &str = "dispatcher";
pub const NODE_ENGINE: &str = "engine_manager";
pub const NODE_SUBMISSION: &str = "submission_manager";
pub const NODE_AGGREGATOR: &str = "aggregator";
pub const NODE_CONTRACT: &str = "contract";

// Node positions (x, y)
const POS_TRIGGER: [f32; 2] = [0.0, 0.0];
const POS_DISPATCHER: [f32; 2] = [200.0, 0.0];
const POS_ENGINE: [f32; 2] = [200.0, -150.0];
const POS_SUBMISSION: [f32; 2] = [400.0, 0.0];
const POS_AGGREGATOR: [f32; 2] = [600.0, 0.0];
const POS_CONTRACT: [f32; 2] = [800.0, 0.0];

/// get position for an operator node (positioned in arc above Aggregator)
fn operator_position(operator_id: &str) -> [f32; 2] {
    let operators = SEEN_OPERATORS.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = operators.lock();
    let index = if set.contains(operator_id) {
        set.iter().position(|x| x == operator_id).unwrap()
    } else {
        let idx = set.len();
        set.insert(operator_id.to_string());
        idx
    };
    // position in arc above Aggregator (600, 0)
    let angle = std::f32::consts::PI * (0.25 + 0.5 * index as f32 / 4.0);
    let radius = 120.0;
    [500.0 + radius * angle.cos(), -radius * angle.sin()]
}

fn node_position(node: &str) -> [f32; 2] {
    match node {
        NODE_TRIGGER => POS_TRIGGER,
        NODE_DISPATCHER => POS_DISPATCHER,
        NODE_ENGINE => POS_ENGINE,
        NODE_SUBMISSION => POS_SUBMISSION,
        NODE_AGGREGATOR => POS_AGGREGATOR,
        NODE_CONTRACT => POS_CONTRACT,
        _ if node.starts_with("operator_") => operator_position(&node[9..]), // strip operator_ prefix
        _ => [0.0, 0.0],
    }
}

/// Initialize Rerun visualization with the network topology
/// (start viewer first using: rerun)
pub fn init_rerun(app_name: &str) -> anyhow::Result<()> {
    let rec = rerun::RecordingStreamBuilder::new(app_name)
        .recording_id("wavs-network-viz")
        .connect_grpc()?;

    // draw static network topology
    log_network_topology(&rec)?;

    RECORDER.get_or_init(|| RwLock::new(Some(rec)));
    Ok(())
}

/// Log the static network topology (nodes and connections).
fn log_network_topology(rec: &RecordingStream) -> anyhow::Result<()> {
    let positions = vec![
        POS_TRIGGER,
        POS_DISPATCHER,
        POS_ENGINE,
        POS_SUBMISSION,
        POS_AGGREGATOR,
        POS_CONTRACT,
    ];

    let colors = vec![
        Color::from_rgb(66, 135, 245), // blue - Trigger
        Color::from_rgb(245, 166, 35), // orange - Dispatcher
        Color::from_rgb(126, 211, 33), // green - Engine
        Color::from_rgb(208, 2, 27),   // red - Submission
        Color::from_rgb(144, 19, 254), // purple - Aggregator
        Color::from_rgb(80, 80, 80),   // gray - Contract
    ];

    let labels = vec![
        "TriggerManager",
        "Dispatcher",
        "EngineManager",
        "SubmissionManager",
        "Aggregator",
        "Contract",
    ];

    // record static nodes as large points
    rec.log_static(
        "network/nodes",
        &Points2D::new(positions)
            .with_colors(colors)
            .with_labels(labels)
            .with_radii([15.0]),
    )?;

    // record static connection lines
    let connections: Vec<Vec<[f32; 2]>> = vec![
        vec![POS_TRIGGER, POS_DISPATCHER],
        vec![POS_DISPATCHER, POS_ENGINE],
        vec![POS_ENGINE, POS_DISPATCHER],
        vec![POS_DISPATCHER, POS_SUBMISSION],
        vec![POS_AGGREGATOR, POS_CONTRACT],
    ];

    rec.log_static(
        "network/connections",
        &LineStrips2D::new(connections)
            .with_colors([Color::from_unmultiplied_rgba(100, 100, 100, 80)]),
    )?;

    Ok(())
}

/// Log and create operator node dynamically.
/// Call this before logging packet flow involving an operator.
pub fn log_operator_node(operator_id: &str) {
    let Some(recorder) = RECORDER.get() else {
        return;
    };
    let lock = recorder.read();
    let Some(rec) = lock.as_ref() else {
        return;
    };

    let pos = operator_position(operator_id);
    let label = if operator_id.len() > 8 {
        format!("Op_{}", &operator_id[..8])
    } else {
        format!("Op_{}", operator_id)
    };

    if let Err(e) = rec.log(
        format!("network/operators/{}", operator_id),
        &Points2D::new([pos])
            .with_colors([Color::from_rgb(255, 165, 0)]) // orange for operators
            .with_labels([label])
            .with_radii([12.0]),
    ) {
        tracing::warn!("Failed to log operator node: {}", e);
    }
}

/// Number of animation frames for packet movement
const ANIMATION_FRAMES: u32 = 10;

/// Log a packet flowing between nodes with animation.
///
/// This animates a point moving from source to destination node.
///
/// # Arguments
/// * `from` - Source node ID (use NODE_* constants)
/// * `to` - Destination node ID (use NODE_* constants)
/// * `event_id` - The event ID of the packet
/// * `workflow_id` - The workflow ID
/// * `details` - Optional additional details to log
pub fn log_packet_flow(
    from: &str,
    to: &str,
    event_id: &str,
    workflow_id: &str,
    details: Option<&str>,
) {
    eprintln!("[wavs-rerun] log_packet_flow: {} -> {}", from, to);

    let Some(recorder) = RECORDER.get() else {
        eprintln!("[wavs-rerun] RECORDER not initialized!");
        return;
    };
    let lock = recorder.read();
    let Some(rec) = lock.as_ref() else {
        eprintln!("[wavs-rerun] RecordingStream is None!");
        return;
    };

    {
        let from_pos = node_position(from);
        let to_pos = node_position(to);

        // Get unique packet ID for this flow
        let packet_id = FRAME_COUNTER.fetch_add(ANIMATION_FRAMES as u64, Ordering::SeqCst);
        let entity_path = format!("network/packets/{}", packet_id);

        // Animate the packet moving from source to destination
        for frame in 0..=ANIMATION_FRAMES {
            let t = frame as f32 / ANIMATION_FRAMES as f32;
            let x = from_pos[0] + (to_pos[0] - from_pos[0]) * t;
            let y = from_pos[1] + (to_pos[1] - from_pos[1]) * t;

            rec.set_time_sequence("frame", (packet_id + frame as u64) as i64);

            if let Err(e) = rec.log(
                entity_path.as_str(),
                &Points2D::new([[x, y]])
                    .with_colors([Color::from_rgb(255, 255, 0)]) // Yellow packet
                    .with_radii([8.0]),
            ) {
                tracing::warn!("Failed to log rerun packet: {}", e);
            }
        }

        // Clear the packet after animation (log empty)
        rec.set_time_sequence("frame", (packet_id + ANIMATION_FRAMES as u64 + 1) as i64);
        let _ = rec.log(entity_path.as_str(), &rerun::Clear::flat());

        // Log packet details as text
        let msg = format!(
            "{} -> {}\nevent: {}\nworkflow: {}{}",
            from,
            to,
            event_id,
            workflow_id,
            details.map(|d| format!("\n{}", d)).unwrap_or_default()
        );

        if let Err(e) = rec.log("network/log", &TextLog::new(msg)) {
            tracing::warn!("Failed to log rerun text: {}", e);
        }
    }
}
