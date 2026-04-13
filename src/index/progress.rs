#[derive(Debug, Clone)]
pub(crate) enum SyncProgressEvent {
    ScanStarted,
    ScanProgress {
        visited_entries: usize,
        discovered_files: usize,
    },
    IndexStarted {
        total_files: usize,
        processed_files: usize,
    },
    IndexProgress {
        processed_files: usize,
        total_files: usize,
        indexed_files: usize,
        skipped_files: usize,
        failed_files: usize,
    },
    Finished {
        total_files: usize,
        processed_files: usize,
        indexed_files: usize,
        skipped_files: usize,
        failed_files: usize,
        partial: bool,
    },
}

pub(crate) trait SyncProgressObserver {
    fn on_event(&mut self, event: SyncProgressEvent);
}
