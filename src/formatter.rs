use std::path::Path;

pub(crate) fn format_text(path: Option<&Path>, source: &str) -> Result<Option<String>, String> {
    let Some(extension) = path
        .and_then(Path::extension)
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
    else {
        return Ok(None);
    };

    match extension.as_str() {
        "toml" => format_toml(source).map(Some),
        "yaml" | "yml" => format_yaml(source).map(Some),
        "json" => format_json(source).map(Some),
        _ => Ok(None),
    }
}

fn format_toml(source: &str) -> Result<String, String> {
    let parsed = taplo::parser::parse(source);
    if let Some(error) = parsed.errors.first() {
        return Err(error.to_string());
    }
    Ok(taplo::formatter::format(source, Default::default()))
}

fn format_yaml(source: &str) -> Result<String, String> {
    pretty_yaml::format_text(source, &Default::default()).map_err(|error| error.to_string())
}

fn format_json(source: &str) -> Result<String, String> {
    let value =
        serde_json::from_str::<serde_json::Value>(source).map_err(|error| error.to_string())?;
    let mut formatted = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    formatted.push('\n');
    Ok(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_formatting_preserves_comments_and_is_idempotent() {
        let source = "# keep this comment\n[server]\nport=8080\n";
        let path = Path::new("settings.toml");

        let formatted = format_text(Some(path), source)
            .expect("valid TOML")
            .expect("supported extension");

        assert!(formatted.contains("# keep this comment"));
        assert!(formatted.contains("port = 8080"));
        assert_eq!(
            format_text(Some(path), &formatted).expect("format twice"),
            Some(formatted)
        );
    }

    #[test]
    fn yaml_formatting_preserves_comments_anchors_and_aliases() {
        let source = "# keep this comment\ndefaults: &defaults\n  enabled: true\nservice:\n  <<: *defaults\n";
        let path = Path::new("settings.yaml");

        let formatted = format_text(Some(path), source)
            .expect("valid YAML")
            .expect("supported extension");

        assert!(formatted.contains("# keep this comment"));
        assert!(formatted.contains("&defaults"));
        assert!(formatted.contains("*defaults"));
        assert_eq!(
            format_text(Some(path), &formatted).expect("format twice"),
            Some(formatted)
        );
    }

    #[test]
    fn json_formatting_preserves_key_order_and_adds_indentation() {
        let formatted = format_text(Some(Path::new("settings.json")), "{\"z\":1,\"a\":2}")
            .expect("valid JSON")
            .expect("supported extension");

        assert_eq!(formatted, "{\n  \"z\": 1,\n  \"a\": 2\n}\n");
    }

    #[test]
    fn invalid_structured_text_is_rejected_without_partial_output() {
        let toml_error =
            format_text(Some(Path::new("broken.toml")), "value = [").expect_err("invalid TOML");
        let yaml_error =
            format_text(Some(Path::new("broken.yaml")), "key: [").expect_err("invalid YAML");

        assert!(!toml_error.is_empty());
        assert!(!yaml_error.is_empty());
    }

    #[test]
    fn unsupported_extensions_are_left_to_the_editor() {
        assert_eq!(
            format_text(Some(Path::new("notes.md")), "# Notes")
                .expect("unsupported is not an error"),
            None
        );
        assert_eq!(
            format_text(None, "value=1").expect("unnamed document"),
            None
        );
    }
}
