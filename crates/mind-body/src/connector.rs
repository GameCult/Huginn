//! The plan windows as one line for a Mind: the one rendering both organs put in front of their
//! sessions, from the windows a credential lease granted (`lease_client`).

use model_connector::PlanQuota;

/// The plan windows as one line for a Mind: each window's percent used, its
/// status, when it resets and how old the reading is. Rendered from the readout
/// and nothing else, so no number in it is ever a paraphrase; the guidance that
/// follows it states the swarm's aim and that nothing enforces it.
pub fn render_plan(windows: &[PlanQuota], now_ms: u64) -> String {
    let guidance = "The swarm aims to have used about 90% of the weekly allowance at reset; nothing enforces it; you and the other Minds set the tempo.";
    if windows.is_empty() {
        return format!("Plan usage: unobserved. {guidance}");
    }
    let rendered: Vec<String> = windows
        .iter()
        .map(|plan| match plan.utilization_permille {
            Some(permille) => format!(
                "{} {}.{}% {}, resets at unix ms {}, read {}s ago",
                plan.window,
                permille / 10,
                permille % 10,
                plan.status,
                plan.resets_at_unix_ms,
                now_ms.saturating_sub(plan.observed_at_unix_ms) / 1000
            ),
            None => format!("{} expired, resets at unix ms {}", plan.window, plan.resets_at_unix_ms),
        })
        .collect();
    format!("Plan usage: {}. {guidance}", rendered.join("; "))
}
