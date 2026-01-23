use clap::ValueEnum;
use indicatif::{ProgressBar, ProgressStyle};

fn colorize(token: &str, color: &str) -> String {
    match color {
        "black" | "red" | "green" | "yellow" | "blue" | "magenta" | "cyan" | "white" => {
            format!("{{{}:.{}}}", token, color)
        }
        _ => format!("{{{}}}", token), // fallback: no color
    }
}

#[derive(Debug, Clone)]
pub struct ProgressOptions {
    pub style: ProgressBarStyle,
    pub filled: String,
    pub empty: String,
    pub head: String,
    pub bar_color: String,
    pub message_color: String,
}
impl ProgressOptions {
    pub fn apply(&self, pb: &ProgressBar, total_files: usize) {
        let bar = colorize("wide_bar", &self.bar_color);
        let msg = colorize("msg", &self.message_color);

        let template = match self.style {
            ProgressBarStyle::Default => {
                format!("{} {{percent}}% {} ETA:{{eta_precise}}", msg, bar)
            }
            ProgressBarStyle::Detailed => format!(
                "{} {} {{percent:>3}}% • {{binary_bytes}}/{{binary_total_bytes}} • \
                 {{binary_bytes_per_sec}} • Elapsed: {{elapsed_precise}} • ETA:{{eta_precise}}",
                msg, bar
            ),
        };

        let chars = format!("{}{}{}", self.filled, self.head, self.empty);

        let style = ProgressStyle::default_bar()
            .template(&template)
            .unwrap()
            .progress_chars(&chars);

        pb.set_style(style);

        pb.set_message(match self.style {
            ProgressBarStyle::Detailed => format!("Copying: 0/{} files", total_files),
            _ => "Copying".to_string(),
        });
    }
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ProgressBarStyle {
    #[default]
    Default,
    Detailed,
}

impl Default for ProgressOptions {
    fn default() -> Self {
        ProgressOptions {
            style: ProgressBarStyle::Default,
            filled: String::from("█"),
            empty: String::from("░"),
            head: String::from("░"),
            bar_color: String::from("white"),
            message_color: String::from("white"),
        }
    }
}
