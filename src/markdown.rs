//! Minimal Markdown builders used by machine-readable Rulewright reports.

use std::fmt::Display;

/// Wrap text in Markdown code delimiters.
#[must_use]
pub(crate) fn code(text: impl Display) -> String {
    format!("`{text}`")
}

/// Wrap text in Markdown bold delimiters.
#[must_use]
pub(crate) fn bold(text: impl Display) -> String {
    format!("**{text}**")
}

/// Wrap text in Markdown italic delimiters.
#[must_use]
pub(crate) fn italic(text: impl Display) -> String {
    format!("*{text}*")
}

/// Builder for multi-line Markdown documents.
#[derive(Debug, Default)]
pub(crate) struct Doc(Vec<String>);

impl Doc {
    /// Create an empty document.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self(Vec::new())
    }

    /// Append one line.
    #[must_use]
    pub(crate) fn line(mut self, text: &str) -> Self {
        self.0.push(text.to_owned());

        self
    }

    /// Append an empty line.
    #[must_use]
    pub(crate) fn blank(mut self) -> Self {
        self.0.push(String::new());

        self
    }

    /// Append a level-one heading.
    #[must_use]
    pub(crate) fn h1(self, text: &str) -> Self {
        self.heading(1, text)
    }

    /// Append a level-two heading.
    #[must_use]
    pub(crate) fn h2(self, text: &str) -> Self {
        self.heading(2, text)
    }

    /// Append a level-three heading.
    #[must_use]
    pub(crate) fn h3(self, text: &str) -> Self {
        self.heading(3, text)
    }

    /// Append a fenced code block.
    #[must_use]
    pub(crate) fn code_block(mut self, language: &str, contents: &str) -> Self {
        self.0.push(format!("```{language}\n{contents}\n```"));

        self
    }

    /// Append a pre-rendered Markdown fragment.
    #[must_use]
    pub(crate) fn raw(mut self, text: String) -> Self {
        self.0.push(text);

        self
    }

    /// Append a block quote.
    #[must_use]
    pub(crate) fn quote(mut self, text: &str) -> Self {
        self.0.push(
            text.lines()
                .map(|line| format!("> {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );

        self
    }

    /// Append a bullet with a bold key.
    #[must_use]
    pub(crate) fn kv_bullet(mut self, key: &str, value: &str) -> Self {
        self.0.push(format!("- **{key}:** {value}"));

        self
    }

    /// Append an inline-code definition item.
    #[must_use]
    pub(crate) fn def(mut self, term: &str, description: &str) -> Self {
        self.0.push(format!("- `{term}` — {description}"));

        self
    }

    /// Render the complete document.
    #[must_use]
    pub(crate) fn build(self) -> String {
        self.0.join("\n")
    }

    fn heading(mut self, level: usize, text: &str) -> Self {
        self.0.push(format!("{} {text}", "#".repeat(level)));

        self
    }
}

/// Markdown table builder.
#[derive(Debug, Default)]
pub(crate) struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    /// Create an empty table.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            headers: Vec::new(),
            rows: Vec::new(),
        }
    }

    /// Set the table headers.
    #[must_use]
    pub(crate) fn headers<I, T>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Display,
    {
        self.headers = headers.into_iter().map(|value| value.to_string()).collect();

        self
    }

    /// Append a row.
    #[must_use]
    pub(crate) fn row<I, T>(mut self, row: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Display,
    {
        self.rows
            .push(row.into_iter().map(|value| value.to_string()).collect());

        self
    }

    /// Render the table.
    #[must_use]
    pub(crate) fn build_markdown(self) -> String {
        let mut lines = Vec::new();

        if !self.headers.is_empty() {
            lines.push(format!(
                "| {} |",
                self.headers
                    .iter()
                    .map(|cell| escape_table_cell(cell))
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
            lines.push(format!(
                "| {} |",
                self.headers
                    .iter()
                    .map(|_| "---")
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }

        lines.extend(self.rows.into_iter().map(|row| {
            format!(
                "| {} |",
                row.iter()
                    .map(|cell| escape_table_cell(cell))
                    .collect::<Vec<_>>()
                    .join(" | ")
            )
        }));

        lines.join("\n")
    }
}

fn escape_table_cell(cell: &str) -> String {
    cell.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', "<br>")
}

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::*;

    #[gtest]
    fn tables_escape_cell_content_that_changes_markdown_structure() -> Result<()> {
        let table = Table::new()
            .headers(["Rule | name", "Description"])
            .row([r"path\name", "first line\nsecond | line"])
            .build_markdown();

        verify_eq!(
            table,
            "| Rule \\| name | Description |\n\
             | --- | --- |\n\
             | path\\\\name | first line<br>second \\| line |"
        )?;

        Ok(())
    }
}
