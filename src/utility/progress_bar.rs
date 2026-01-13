use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug, Clone, Copy, Default)]
pub enum ProgressBarStyle {
    #[default]
    Default,
    Minimal,
}

impl ProgressBarStyle {
    pub fn apply(&self, pb: &ProgressBar, total_files: usize) {
        let style = match self {
            ProgressBarStyle::Minimal => ProgressStyle::default_bar()
                .template("{msg} {percent}% |{wide_bar}| ETA:{eta_precise}")
                .unwrap()
                .progress_chars("▓░ "),

            ProgressBarStyle::Default => ProgressStyle::default_bar()
                .template(
                    "{msg} [{wide_bar}] {percent:>3}% | \
                         {binary_bytes}/{binary_total_bytes} | ETA:{eta_precise}",
                )
                .unwrap()
                .progress_chars("=> "),
        };

        pb.set_style(style);
        if matches!(self, ProgressBarStyle::Default) {
            pb.set_message(format!("Copying: 0/{} files", total_files));
        } else {
            pb.set_message("Copying");
        }
    }
}
