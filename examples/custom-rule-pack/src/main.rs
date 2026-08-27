use std::process::ExitCode;

fn main() -> ExitCode {
    match rulewright_custom_rule_pack::registry() {
        Ok(registry) => rulewright::run_with_registry(&registry),
        Err(error) => {
            eprintln!("custom-rulewright: {error}");
            ExitCode::FAILURE
        }
    }
}
