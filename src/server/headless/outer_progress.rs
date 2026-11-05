use std::collections::HashMap;

use crate::ghostty::{ProgressReport, ProgressState};
use crate::layout::PaneId;

/// Aggregates OSC 9;4 reports from every pane into the single value the outer
/// terminal can display.
///
/// The outer terminal has one progress indicator, so concurrent panes
/// alternate: every new report becomes the visible one. A pane that finishes
/// must not clear a pane that is still running, so a removal falls back to the
/// most recently updated remaining pane. Sending the aggregate, and suppressing
/// an unchanged value, is the caller's job.
#[derive(Debug, Default)]
pub(super) struct OuterProgress {
    next_seq: u64,
    active: HashMap<PaneId, (u64, ProgressReport)>,
}

impl OuterProgress {
    /// Records the pane's latest report and returns the aggregate to display.
    pub(super) fn observe(
        &mut self,
        pane_id: PaneId,
        report: ProgressReport,
    ) -> Option<ProgressReport> {
        self.next_seq += 1;
        if report.state == ProgressState::Remove {
            self.active.remove(&pane_id);
        } else {
            self.active.insert(pane_id, (self.next_seq, report));
        }
        self.aggregate()
    }

    /// Drops a pane whose process or tab went away and returns the aggregate.
    pub(super) fn forget_pane(&mut self, pane_id: PaneId) -> Option<ProgressReport> {
        self.active.remove(&pane_id);
        self.aggregate()
    }

    /// Most recently updated active pane. `None` means nothing is running.
    pub(super) fn aggregate(&self) -> Option<ProgressReport> {
        self.active
            .values()
            .max_by_key(|(seq, _)| *seq)
            .map(|(_, report)| *report)
    }

    pub(super) fn panes(&self) -> impl Iterator<Item = PaneId> + '_ {
        self.active.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(progress: u8) -> ProgressReport {
        ProgressReport {
            state: ProgressState::Set,
            progress: Some(progress),
        }
    }

    fn indeterminate() -> ProgressReport {
        ProgressReport {
            state: ProgressState::Indeterminate,
            progress: None,
        }
    }

    fn pane(raw: u32) -> PaneId {
        PaneId::from_raw(raw)
    }

    #[test]
    fn a_single_pane_reports_and_clears() {
        let mut progress = OuterProgress::default();
        assert_eq!(progress.aggregate(), None);
        assert_eq!(progress.observe(pane(1), set(42)), Some(set(42)));
        assert_eq!(progress.observe(pane(1), ProgressReport::REMOVE), None);
        assert_eq!(progress.aggregate(), None);
    }

    #[test]
    fn concurrent_panes_alternate() {
        let mut progress = OuterProgress::default();
        assert_eq!(progress.observe(pane(1), set(30)), Some(set(30)));
        assert_eq!(
            progress.observe(pane(2), indeterminate()),
            Some(indeterminate())
        );
        // The pane that reported last is the visible one, again.
        assert_eq!(progress.observe(pane(1), set(50)), Some(set(50)));
    }

    #[test]
    fn finishing_a_pane_keeps_a_running_pane_visible() {
        let mut progress = OuterProgress::default();
        progress.observe(pane(1), set(50));
        progress.observe(pane(2), indeterminate());
        progress.observe(pane(1), set(50));

        assert_eq!(
            progress.observe(pane(2), ProgressReport::REMOVE),
            Some(set(50))
        );
        assert_eq!(progress.observe(pane(1), ProgressReport::REMOVE), None);
    }

    #[test]
    fn identical_reports_from_another_pane_keep_the_value() {
        let mut progress = OuterProgress::default();
        assert_eq!(progress.observe(pane(1), set(42)), Some(set(42)));
        assert_eq!(progress.observe(pane(2), set(42)), Some(set(42)));
        // Removing the newest reporter falls back to the other identical value.
        assert_eq!(
            progress.observe(pane(2), ProgressReport::REMOVE),
            Some(set(42))
        );
        assert_eq!(progress.observe(pane(1), ProgressReport::REMOVE), None);
    }

    #[test]
    fn forgetting_a_pane_recomputes_the_aggregate() {
        let mut progress = OuterProgress::default();
        progress.observe(pane(1), set(10));
        progress.observe(pane(2), set(20));
        assert_eq!(progress.forget_pane(pane(2)), Some(set(10)));
        assert_eq!(progress.forget_pane(pane(2)), Some(set(10)));
        assert_eq!(progress.forget_pane(pane(1)), None);
    }

    #[test]
    fn panes_lists_only_active_reports() {
        let mut progress = OuterProgress::default();
        progress.observe(pane(1), set(10));
        progress.observe(pane(2), set(20));
        progress.observe(pane(2), ProgressReport::REMOVE);

        let mut panes: Vec<_> = progress.panes().collect();
        panes.sort_by_key(|pane_id| pane_id.raw());
        assert_eq!(panes, vec![pane(1)]);
    }
}
