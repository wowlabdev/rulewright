//! Compact terminal presentation for the Rulewright CLI.

use console::style;
use tabled::Tabled;

/// Print a section heading to standard error.
pub fn header(text: &str) {
    eprintln!("{}", style(format!("▸ {text}")).bold().cyan());
}

/// Print a subsection heading to standard error.
pub fn subheader(text: &str) {
    eprintln!("{}", style(text).bold().underlined());
}

/// Print a key-value detail.
pub fn kv(key: &str, value: &str) {
    eprintln!(
        "  {} {}",
        style(format!("{key}:")).dim(),
        style(value).bold()
    );
}

/// Print a successful outcome.
pub fn success(message: &str) {
    eprintln!("{} {message}", style("✓").green().bold());
}

/// Print an error outcome.
pub fn error(message: &str) {
    eprintln!("{} {message}", style("✗").red().bold());
}

/// Print a warning outcome.
pub fn warning(message: &str) {
    eprintln!("{} {message}", style("!").yellow().bold());
}

/// Print secondary information.
pub fn detail(text: &str) {
    eprintln!("  {}", style(text).dim());
}

/// Print a blank line.
pub fn blank() {
    eprintln!();
}

/// Print a visual separator.
pub fn separator() {
    eprintln!("{}", style("─".repeat(50)).dim());
}

/// Render tabular data to standard error.
pub fn table<I, T>(data: I)
where
    I: IntoIterator<Item = T>,
    T: Tabled,
{
    eprintln!("{}", tabled::Table::new(data));
}

/// Print the CLI name and version unless quiet output was requested.
pub fn banner(name: &str, version: &str) {
    eprintln!(
        "{} {}\n",
        style(name).bold().cyan(),
        style(format!("v{version}")).dim()
    );
}
