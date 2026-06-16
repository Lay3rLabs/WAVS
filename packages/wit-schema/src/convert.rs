use std::collections::{BTreeMap, HashMap};

use serde_json::{json, Value};
use wasmtime::component::types::{self, Type};

/// Compute a structural fingerprint for a type, used for $defs deduplication (D-06).
/// Returns None for primitive types that don't need deduplication.
fn type_fingerprint(ty: &Type) -> Option<String> {
    match ty {
        Type::Record(record) => {
            let fields: Vec<String> = record.fields().map(|f| f.name.to_string()).collect();
            Some(format!("record:{}", fields.join("|")))
        }
        Type::Variant(variant) => {
            let cases: Vec<String> = variant.cases().map(|c| c.name.to_string()).collect();
            Some(format!("variant:{}", cases.join("|")))
        }
        Type::Enum(enum_ty) => {
            let names: Vec<String> = enum_ty.names().map(|n| n.to_string()).collect();
            Some(format!("enum:{}", names.join("|")))
        }
        Type::Flags(flags) => {
            let names: Vec<String> = flags.names().map(|n| n.to_string()).collect();
            Some(format!("flags:{}", names.join("|")))
        }
        _ => None,
    }
}

/// Generate a def name from a fingerprint.
fn def_name_from_fingerprint(fingerprint: &str) -> String {
    // Strip the type prefix and use field/case names
    let parts: Vec<&str> = fingerprint.splitn(2, ':').collect();
    if parts.len() == 2 {
        parts[1].replace('|', "_")
    } else {
        fingerprint.replace('|', "_")
    }
}

/// Convert a WIT type to its JSON Schema representation.
///
/// `defs` accumulates shared type definitions for the `$defs` section.
/// `seen_types` tracks structural fingerprints for deduplication (D-06).
/// `param_name` is an optional hint for naming $defs entries.
pub fn type_to_schema(
    ty: &Type,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    type_to_schema_inner(ty, defs, seen_types, None)
}

/// Convert a WIT type to JSON Schema with an optional parameter name hint for $defs naming.
pub fn type_to_schema_named(
    ty: &Type,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
    param_name: Option<&str>,
) -> Value {
    type_to_schema_inner(ty, defs, seen_types, param_name)
}

fn type_to_schema_inner(
    ty: &Type,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
    param_name: Option<&str>,
) -> Value {
    // Check for $defs deduplication on complex types (D-06)
    if let Some(fingerprint) = type_fingerprint(ty) {
        let count = seen_types.entry(fingerprint.clone()).or_insert(0);
        *count += 1;

        if *count > 1 {
            // This type has been seen before -- use or create a $ref
            let def_name = if let Some(name) = param_name {
                name.to_string()
            } else {
                def_name_from_fingerprint(&fingerprint)
            };

            if !defs.contains_key(&def_name) {
                // First time moving to $defs -- generate the schema and store it
                let schema = convert_type_direct(ty, defs, seen_types);
                defs.insert(def_name.clone(), schema);
            }

            return json!({"$ref": format!("#/$defs/{}", def_name)});
        }
    }

    convert_type_direct(ty, defs, seen_types)
}

/// Convert a type directly without deduplication checks (used internally).
fn convert_type_direct(
    ty: &Type,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    match ty {
        Type::Bool => json!({"type": "boolean"}),
        Type::U8 | Type::U16 | Type::U32 => json!({"type": "integer", "minimum": 0}),
        Type::S8 | Type::S16 | Type::S32 => json!({"type": "integer"}),
        Type::U64 | Type::S64 => json!({"type": "integer"}),
        Type::Float32 | Type::Float64 => json!({"type": "number"}),
        Type::Char => json!({"type": "string", "maxLength": 1}),
        Type::String => json!({"type": "string"}),
        Type::List(list) => list_to_schema(list, defs, seen_types),
        Type::Record(record) => record_to_schema(record, defs, seen_types),
        Type::Variant(variant) => variant_to_schema(variant, defs, seen_types),
        Type::Enum(enum_ty) => enum_to_schema(enum_ty),
        Type::Option(opt) => option_to_schema(opt, defs, seen_types),
        Type::Result(result) => result_to_schema(result, defs, seen_types),
        Type::Tuple(tuple) => tuple_to_schema(tuple, defs, seen_types),
        Type::Flags(flags) => flags_to_schema(flags),
        // Resource types (Own, Borrow) and others -- not expected in WAVS components
        _ => json!({}),
    }
}

/// Handle list types, with special case for list<u8> (D-03/Pitfall 4).
fn list_to_schema(
    list: &types::List,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    // Special case: list<u8> represents bytes
    if matches!(list.ty(), Type::U8) {
        json!({"type": "string", "contentEncoding": "base64"})
    } else {
        json!({
            "type": "array",
            "items": type_to_schema_inner(&list.ty(), defs, seen_types, None)
        })
    }
}

/// Check if a record is the WAVS u128 type (D-03).
/// u128 is defined as: record u128 { value: tuple<u64, u64> }
fn is_u128_record(record: &types::Record) -> bool {
    let fields: Vec<_> = record.fields().collect();
    if fields.len() != 1 {
        return false;
    }
    let field = &fields[0];
    if field.name != "value" {
        return false;
    }
    if let Type::Tuple(tuple) = &field.ty {
        let types: Vec<_> = tuple.types().collect();
        types.len() == 2 && matches!(types[0], Type::U64) && matches!(types[1], Type::U64)
    } else {
        false
    }
}

/// Convert a record type to JSON Schema (D-01).
/// Checks for u128 special case first (D-03).
fn record_to_schema(
    record: &types::Record,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    // u128 special case (D-03)
    if is_u128_record(record) {
        return json!({
            "type": "string",
            "pattern": "^[0-9]+$",
            "description": "128-bit unsigned integer"
        });
    }

    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for field in record.fields() {
        properties.insert(
            field.name.to_string(),
            type_to_schema_inner(&field.ty, defs, seen_types, Some(field.name)),
        );
        required.push(json!(field.name));
    }

    json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": required,
        "additionalProperties": false
    })
}

/// Convert a variant type to JSON Schema with externally tagged representation (D-01).
fn variant_to_schema(
    variant: &types::Variant,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    let mut one_of = Vec::new();

    for case in variant.cases() {
        let payload_schema = if let Some(ref payload_ty) = case.ty {
            type_to_schema_inner(payload_ty, defs, seen_types, Some(case.name))
        } else {
            // No-payload variant case -- value is an empty object
            json!({"type": "object", "maxProperties": 0})
        };

        let mut props = serde_json::Map::new();
        props.insert(case.name.to_string(), payload_schema);

        one_of.push(json!({
            "type": "object",
            "properties": Value::Object(props),
            "required": [case.name],
            "additionalProperties": false
        }));
    }

    json!({"oneOf": one_of})
}

/// Convert an enum type to JSON Schema (D-02).
fn enum_to_schema(enum_ty: &types::Enum) -> Value {
    let names: Vec<Value> = enum_ty.names().map(|n| json!(n)).collect();
    json!({"type": "string", "enum": names})
}

/// Convert an option type to JSON Schema (nullable).
fn option_to_schema(
    opt: &types::OptionType,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    json!({
        "anyOf": [
            type_to_schema_inner(&opt.ty(), defs, seen_types, None),
            {"type": "null"}
        ]
    })
}

/// Convert a result type to JSON Schema (full representation for inputs).
fn result_to_schema(
    result: &types::ResultType,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    let ok_schema = result
        .ok()
        .map(|ty| type_to_schema_inner(&ty, defs, seen_types, None))
        .unwrap_or_else(|| json!({"type": "object", "maxProperties": 0}));
    let err_schema = result
        .err()
        .map(|ty| type_to_schema_inner(&ty, defs, seen_types, None))
        .unwrap_or_else(|| json!({"type": "object", "maxProperties": 0}));

    let mut ok_props = serde_json::Map::new();
    ok_props.insert("ok".to_string(), ok_schema);

    let mut err_props = serde_json::Map::new();
    err_props.insert("err".to_string(), err_schema);

    json!({
        "oneOf": [
            {
                "type": "object",
                "properties": Value::Object(ok_props),
                "required": ["ok"],
                "additionalProperties": false
            },
            {
                "type": "object",
                "properties": Value::Object(err_props),
                "required": ["err"],
                "additionalProperties": false
            }
        ]
    })
}

/// Convert a result type for output schemas, simplifying result<T, string> cases.
///
/// When the error type is `string`, returns just the ok type schema with a description
/// noting the error possibility. Otherwise returns the full oneOf representation.
pub fn result_to_output_schema(
    result: &types::ResultType,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    // Check if the error type is string (common WAVS pattern)
    let err_is_string = result
        .err()
        .map(|ty| matches!(ty, Type::String))
        .unwrap_or(false);

    if err_is_string {
        // Simplify: return the ok type as the primary schema
        if let Some(ok_ty) = result.ok() {
            let mut schema = type_to_schema_inner(&ok_ty, defs, seen_types, None);
            // Add description noting the error type
            if let Some(obj) = schema.as_object_mut() {
                obj.insert(
                    "description".to_string(),
                    json!("On error, returns a string error message"),
                );
            }
            schema
        } else {
            // result<_, string> -- no ok type
            json!({
                "type": "object",
                "maxProperties": 0,
                "description": "On error, returns a string error message"
            })
        }
    } else {
        // Full representation for non-string errors
        result_to_schema(result, defs, seen_types)
    }
}

/// Convert a tuple type to JSON Schema.
fn tuple_to_schema(
    tuple: &types::Tuple,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    let items: Vec<Value> = tuple
        .types()
        .map(|ty| type_to_schema_inner(&ty, defs, seen_types, None))
        .collect();
    let len = items.len();
    json!({
        "type": "array",
        "prefixItems": items,
        "minItems": len,
        "maxItems": len
    })
}

/// Convert a flags type to JSON Schema.
fn flags_to_schema(flags: &types::Flags) -> Value {
    let names: Vec<Value> = flags.names().map(|n| json!(n)).collect();
    json!({
        "type": "array",
        "items": {"type": "string", "enum": names},
        "uniqueItems": true
    })
}
