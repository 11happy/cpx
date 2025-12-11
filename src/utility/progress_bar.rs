use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug, Clone, Copy)]
pub enum ProgressBarStyle {
    Default,
    Minimal,
    Detailed,
}

impl ProgressBarStyle {
    pub fn apply(&self, pb: &ProgressBar) {
        let style = match self {
             ProgressBarStyle::Default => {
                ProgressStyle::default_bar()
                    .template(" {spinner} {msg} • {binary_bytes}/{binary_total_bytes} • ETA: {eta}\n[{wide_bar}]")
                    .unwrap()
                    .progress_chars("━━╾─")
            }
            ProgressBarStyle::Minimal => {
                ProgressStyle::default_bar()
                    .template("{spinner} {msg} • {percent}%\n[{wide_bar}]")
                    .unwrap()
                    .progress_chars("•-")
            }
            ProgressBarStyle::Detailed => {
                ProgressStyle::default_bar()
                    .template("{spinner} {msg} • {binary_bytes}/{binary_total_bytes} • {binary_bytes_per_sec} • Elapsed: {elapsed_precise} • ETA: {eta_precise}\n[{wide_bar}]")
                    .unwrap()
                    .progress_chars("=>-")
            }
        };
        pb.set_style(style);
    }
}

impl Default for ProgressBarStyle {
    fn default() -> Self {
        ProgressBarStyle::Default
    }
}
