use std::path::Path;

use anyhow::Result;
use serde_json::Value;

/// Enrich a generated schema with doc comments extracted from WIT source files.
///
/// Walks the parsed WIT package, matches function and type names to schema entries,
/// and adds "description" fields where doc comments exist.
///
/// Per D-07: If parsing fails or no docs found, logs a warning and returns Ok(()).
/// Doc comment enrichment never fails the schema generation.
pub fn enrich_with_docs(schema: &mut Value, wit_path: &Path) -> Result<()> {
    let mut resolve = wit_parser::Resolve::new();

    // Try to parse the WIT source. Use push_dir for directories, push_file for single files.
    let package_id = if wit_path.is_dir() {
        match resolve.push_dir(wit_path) {
            Ok((pkg_id, _source_map)) => pkg_id,
            Err(e) => {
                tracing::warn!(
                    path = %wit_path.display(),
                    error = %e,
                    "Failed to parse WIT directory for doc enrichment, skipping"
                );
                return Ok(());
            }
        }
    } else {
        match resolve.push_file(wit_path) {
            Ok(pkg_id) => pkg_id,
            Err(e) => {
                tracing::warn!(
                    path = %wit_path.display(),
                    error = %e,
                    "Failed to parse WIT file for doc enrichment, skipping"
                );
                return Ok(());
            }
        }
    };

    let package = &resolve.packages[package_id];

    // Enrich exported function descriptions from worlds
    for world_id in package.worlds.values() {
        let world = &resolve.worlds[*world_id];
        for (key, item) in &world.exports {
            match item {
                wit_parser::WorldItem::Function(func) => {
                    if let Some(ref doc_contents) = func.docs.contents {
                        let func_name = match key {
                            wit_parser::WorldKey::Name(n) => n.clone(),
                            wit_parser::WorldKey::Interface(_) => continue,
                        };
                        // Look for the function in schema exports
                        if let Some(export) = schema
                            .get_mut("exports")
                            .and_then(|e| e.get_mut(&func_name))
                        {
                            if let Some(obj) = export.as_object_mut() {
                                obj.insert(
                                    "description".to_string(),
                                    Value::String(doc_contents.trim().to_string()),
                                );
                            }
                        }
                    }
                }
                wit_parser::WorldItem::Interface { id, .. } => {
                    // Check functions inside exported interfaces
                    let iface = &resolve.interfaces[*id];
                    for (func_name, func) in &iface.functions {
                        if let Some(ref doc_contents) = func.docs.contents {
                            // Try both bare name and interface-qualified name
                            let iface_name = iface
                                .name
                                .as_ref()
                                .map(|n| format!("{}/{}", n, func_name))
                                .unwrap_or_else(|| func_name.clone());

                            if let Some(exports) = schema.get_mut("exports") {
                                // Try interface-qualified name first
                                if let Some(export) = exports.get_mut(&iface_name) {
                                    if let Some(obj) = export.as_object_mut() {
                                        obj.insert(
                                            "description".to_string(),
                                            Value::String(doc_contents.trim().to_string()),
                                        );
                                    }
                                }
                                // Also try bare function name
                                if let Some(export) = exports.get_mut(func_name) {
                                    if let Some(obj) = export.as_object_mut() {
                                        obj.insert(
                                            "description".to_string(),
                                            Value::String(doc_contents.trim().to_string()),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Enrich type descriptions in $defs
    for (_type_id, typedef) in resolve.types.iter() {
        if let Some(ref doc_contents) = typedef.docs.contents {
            if let Some(ref name) = typedef.name {
                // Try to find the type in $defs by name
                if let Some(defs) = schema.get_mut("$defs") {
                    if let Some(def) = defs.get_mut(name) {
                        if let Some(obj) = def.as_object_mut() {
                            obj.insert(
                                "description".to_string(),
                                Value::String(doc_contents.trim().to_string()),
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn test_enrich_with_doc_comments_from_fixture() {
        let wit_content = r#"package test:example;

interface types {
    /// A greeting message
    record greeting {
        message: string,
    }
}

world test-world {
    /// Say hello to someone
    export hello: func(name: string) -> string;
}
"#;

        // Write fixture to temp file
        let dir = tempfile::tempdir().unwrap();
        let wit_file = dir.path().join("test.wit");
        let mut f = std::fs::File::create(&wit_file).unwrap();
        f.write_all(wit_content.as_bytes()).unwrap();

        // Build a mock schema that matches the fixture
        let mut schema = json!({
            "world": "test-world",
            "exports": {
                "hello": {
                    "inputSchema": {"type": "string"},
                    "outputSchema": {"type": "string"}
                }
            },
            "$defs": {
                "greeting": {
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    }
                }
            }
        });

        enrich_with_docs(&mut schema, &wit_file).unwrap();

        // Check function description was added
        let hello = schema.get("exports").unwrap().get("hello").unwrap();
        assert_eq!(
            hello.get("description").and_then(|d| d.as_str()),
            Some("Say hello to someone"),
            "function doc comment should be added"
        );

        // Check type description was added
        let greeting = schema.get("$defs").unwrap().get("greeting").unwrap();
        assert_eq!(
            greeting.get("description").and_then(|d| d.as_str()),
            Some("A greeting message"),
            "type doc comment should be added"
        );
    }

    #[test]
    fn test_enrich_with_nonexistent_path_does_not_error() {
        let mut schema = json!({
            "world": "test",
            "exports": {},
            "$defs": {}
        });

        let result = enrich_with_docs(&mut schema, Path::new("/nonexistent/path/test.wit"));
        assert!(
            result.is_ok(),
            "enriching with nonexistent path should not error"
        );
    }

    #[test]
    fn test_enrich_with_no_doc_comments_leaves_schema_unchanged() {
        let wit_content = r#"package test:nodocs;

world test-world {
    export greet: func(name: string) -> string;
}
"#;

        let dir = tempfile::tempdir().unwrap();
        let wit_file = dir.path().join("nodocs.wit");
        let mut f = std::fs::File::create(&wit_file).unwrap();
        f.write_all(wit_content.as_bytes()).unwrap();

        let mut schema = json!({
            "world": "test-world",
            "exports": {
                "greet": {
                    "inputSchema": {"type": "string"},
                    "outputSchema": {"type": "string"}
                }
            },
            "$defs": {}
        });

        let schema_before = schema.clone();
        enrich_with_docs(&mut schema, &wit_file).unwrap();

        assert_eq!(
            schema, schema_before,
            "schema without doc comments should remain unchanged"
        );
    }
}
