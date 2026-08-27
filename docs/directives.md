# Suppression directives

Sometimes a rule is wrong for one particular line. Rulewright suppressions are Rust comments beginning with `#rw`, but they are not unexplained escape hatches: every directive needs at least one target and a real reason.

```rust
// #rw(rust_panic) executable boundary intentionally aborts on invalid bootstrap state
panic!("invalid bootstrap state");
```

Supported scopes:

```rust
// #rw(rule) reason
// #rw(rule_one, rule_two) reason
// #rw(file: rule) reason
// #rw(block: rule) reason
// #rw(fn: rule) reason
// #rw(*) reason
```

- The default scope covers the next source line.
- `file:` covers the complete file.
- `block:` covers the following logical block until its closing boundary.
- `fn:` covers the following function.
- `*` covers every registered rule in the selected scope and cannot be mixed with named targets.

Unknown rules, empty targets, malformed scopes, and missing reasons are findings under `rust_rulewright_directives`. Text that merely looks like a directive inside a string or another non-comment token does nothing.

`rulewright --suppressions` reports active suppressions. `rulewright clean --dry-run` previews stale targets; `rulewright clean` atomically removes or rewrites only targets that no longer cover a finding.

## Alignment marker

`// #rw:aligned` is not a suppression. It opts the immediately following table-like block into the `rust_aligned` rule:

```rust
// #rw:aligned
Parser    => "parser";
Formatter => "formatter";
Writer    => "writer";
```

For blocks with at least two rows, the rule keeps `=>`, corresponding commas, and trailing `//` comments in consistent columns. The region ends at a blank line, a common closing delimiter, or another alignment marker, so the marker should sit directly above the small block it describes. Without the marker, Rulewright leaves ordinary source spacing alone. `rulewright --fix` can add the missing spaces when a marked block drifts out of alignment.

Inside an array or slice, the marker also keeps each simple tuple row on one line:

```rust
#[rustfmt::skip]
let cases = [
    // #rw:aligned
    (SHORT,     "first"),
    (LONG_NAME, "second"),
];
```

Rulewright can collapse a wrapped row when every tuple field is a literal or a path. Calls, blocks, nested tuples, and other expressions are reported but deliberately left unchanged because flattening them would be unsafe. Rustfmt normally removes manual column padding, so put `#[rustfmt::skip]` on the containing item when the aligned layout should survive `cargo fmt`.

## Sorted-region marker

`// #rw:sorted(asc)` and `// #rw:sorted(desc)` opt the immediately following contiguous lines into `rust_sorted`:

```rust
// #rw:sorted(asc)
use alpha::Value;
use beta::Value;
use gamma::Value;
```

The direction is explicit because registries and precedence tables sometimes read more naturally in descending order. Comments keep their positions while sortable lines move around them, and the region ends at a blank line, a common closing delimiter, or another `#rw:` marker. `rulewright --fix` sorts a marked region without touching unmarked source.
