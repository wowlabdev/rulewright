use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Violation, atomic, file, path::Path};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub(super) enum BaselineError {
    #[error("baseline filesystem operation failed: {0}")]
    Filesystem(#[from] crate::error::Error),
    #[error("failed to parse baseline {}: {source}", path.display())]
    Parse {
        path: crate::PathBuf,
        source: serde_json::Error,
    },
    #[error("unsupported baseline schema version {0}")]
    Schema(u32),
    #[error("failed to serialize baseline: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Deserialize, Serialize)]
struct BaselineDocument {
    schema_version: u32,
    findings: Vec<BaselineFinding>,
}

#[derive(Clone, Deserialize, Serialize)]
struct BaselineFinding {
    rule: String,
    path: String,
    message: String,
    count: usize,
}

type FindingKey = (String, String, String);

pub(super) fn write(path: &Path, violations: &[Violation]) -> Result<(), BaselineError> {
    let mut counts: BTreeMap<FindingKey, usize> = BTreeMap::new();

    for violation in violations {
        *counts.entry(key(violation)).or_default() += 1;
    }

    let findings = counts
        .into_iter()
        .map(|((rule, path, message), count)| BaselineFinding {
            rule,
            path,
            message,
            count,
        })
        .collect();
    let document = BaselineDocument {
        schema_version: SCHEMA_VERSION,
        findings,
    };
    let mut encoded = serde_json::to_vec_pretty(&document)?;

    encoded.push(b'\n');
    atomic::replace(path, &encoded)?;

    Ok(())
}

// #rw(fn: rust_map_err_pure_wrap) serde_json does not know the baseline path, which this error adds
pub(super) fn filter(
    path: &Path,
    violations: Vec<Violation>,
) -> Result<Vec<Violation>, BaselineError> {
    let contents = file::read_text(path)?;
    let document: BaselineDocument =
        serde_json::from_str(&contents).map_err(|source| BaselineError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

    if document.schema_version != SCHEMA_VERSION {
        return Err(BaselineError::Schema(document.schema_version));
    }

    let mut remaining: BTreeMap<FindingKey, usize> = BTreeMap::new();

    for finding in document.findings {
        let key = (finding.rule, finding.path, finding.message);
        let count = remaining.entry(key).or_default();

        *count = count.saturating_add(finding.count);
    }

    let mut new_findings = Vec::new();

    for violation in violations {
        let allowed = remaining.entry(key(&violation)).or_default();

        if *allowed == 0 {
            new_findings.push(violation);
        } else {
            *allowed -= 1;
        }
    }

    Ok(new_findings)
}

fn key(violation: &Violation) -> FindingKey {
    (
        violation.rule_name().to_owned(),
        violation.rel.clone(),
        violation.message.clone(),
    )
}

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::*;

    #[gtest]
    fn baseline_allows_only_the_recorded_duplicate_count() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let path = directory.path().join("baseline.json");
        let recorded = [
            crate::violation("src/lib.rs", 2, "same finding").with_rule("rust_example"),
            crate::violation("src/lib.rs", 8, "same finding").with_rule("rust_example"),
        ];

        write(&path, &recorded).or_fail()?;
        let current = vec![
            crate::violation("src/lib.rs", 3, "same finding").with_rule("rust_example"),
            crate::violation("src/lib.rs", 9, "same finding").with_rule("rust_example"),
            crate::violation("src/lib.rs", 12, "same finding").with_rule("rust_example"),
        ];
        let new_findings = filter(&path, current).or_fail()?;

        verify_eq!(new_findings.len(), 1)?;

        verify_eq!(new_findings[0].line, 12)
    }

    #[gtest]
    fn baseline_survives_line_movement() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let path = directory.path().join("baseline.json");
        let recorded = [crate::violation("src/lib.rs", 2, "finding").with_rule("rust_example")];

        write(&path, &recorded).or_fail()?;
        let moved = vec![crate::violation("src/lib.rs", 200, "finding").with_rule("rust_example")];

        verify_true!(filter(&path, moved).or_fail()?.is_empty())
    }

    #[gtest]
    fn duplicate_entries_in_a_handwritten_baseline_are_additive() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let path = directory.path().join("baseline.json");

        file::write_text(
            &path,
            r#"{
  "schema_version": 1,
  "findings": [
    { "rule": "rust_example", "path": "src/lib.rs", "message": "finding", "count": 1 },
    { "rule": "rust_example", "path": "src/lib.rs", "message": "finding", "count": 2 }
  ]
}
"#,
        )
        .or_fail()?;
        let current = (0..4)
            .map(|line| crate::violation("src/lib.rs", line, "finding").with_rule("rust_example"))
            .collect();

        verify_eq!(filter(&path, current).or_fail()?.len(), 1)
    }
}
