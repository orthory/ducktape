//! Host observations and advisory producer reports retain separate provenance.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FrameReports {
    pub(super) local: wire::SanitizeReport,
    pub(super) upstream: wire::SanitizeReport,
}
impl FrameReports {
    pub(super) fn inherit(&mut self, held: Self) {
        self.local.merge(held.local);
        self.upstream.merge(held.upstream);
    }
}
#[derive(Default)]
pub(super) struct DisplayDiagnostics {
    seen: FrameReports,
}
impl DisplayDiagnostics {
    pub(super) fn observe(&mut self, reports: FrameReports) -> [Option<&'static str>; 2] {
        let mut origins = [None, None];
        if reports.local.display_text_truncated && !self.seen.local.display_text_truncated {
            origins[0] = Some("host");
        }
        if reports.upstream.display_text_truncated && !self.seen.upstream.display_text_truncated {
            origins[1] = Some("producer-reported");
        }
        self.seen.inherit(reports);
        origins
    }
}
