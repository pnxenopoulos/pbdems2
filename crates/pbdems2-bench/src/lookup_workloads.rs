//! Lookup workloads separating field scans, prefixes, nesting, and array indices.

use std::sync::Arc;

use pbdems2::entity::field_path::FieldPath;
use pbdems2::entity::{
    FlattenedField, FlattenedSerializer, FlattenedSerializerDefinition, Serializer,
    SerializerContainer,
};

use crate::BENCH_PROFILE;

/// A lookup with an independently constructed expected packed key.
pub struct LookupCase {
    /// Stable benchmark label.
    pub name: &'static str,
    /// Schema under test.
    pub serializer: Arc<Serializer>,
    /// Dotted query, allocated before measurement.
    pub path: String,
    /// Expected key, or None for a miss.
    pub expected: Option<u64>,
}

#[derive(Default)]
struct Schema {
    symbols: Vec<String>,
    fields: Vec<FlattenedField>,
    definitions: Vec<FlattenedSerializerDefinition>,
}

impl Schema {
    fn symbol(&mut self, value: &str) -> i32 {
        let index = self.symbols.len() as i32;
        self.symbols.push(value.into());
        index
    }

    fn field(&mut self, name: &str, node: Option<&str>, child: Option<&str>, array: bool) -> i32 {
        let ty = self.symbol(if array {
            "CNetworkUtlVectorBase< CLeaf >"
        } else {
            "bool"
        });
        let name = self.symbol(name);
        let node = node.map(|node| self.symbol(node));
        let child = child.map(|child| self.symbol(child));
        let index = self.fields.len() as i32;
        self.fields.push(
            FlattenedField::new(Some(ty), Some(name))
                .with_send_node_sym(node)
                .with_serializer_name_sym(child),
        );
        index
    }

    fn define(&mut self, name: &str, fields: Vec<i32>) {
        let symbol = self.symbol(name);
        self.definitions
            .push(FlattenedSerializerDefinition::new(Some(symbol), fields));
    }

    fn finish(self, root: &str) -> Arc<Serializer> {
        SerializerContainer::parse(
            FlattenedSerializer::new(self.definitions, self.symbols, self.fields),
            BENCH_PROFILE,
        )
        .expect("valid lookup schema")
        .get(root)
        .expect("root exists")
        .clone()
        .into()
    }
}

fn key(indices: &[u8]) -> u64 {
    let mut path = FieldPath::default();
    path.data[..indices.len()].copy_from_slice(indices);
    path.last = indices.len() - 1;
    path.pack()
}

/// Fixed-width names prevent digit counts from changing comparison work.
pub fn lookup_cases() -> Vec<LookupCase> {
    let mut cases = Vec::new();
    let mut schema = Schema::default();
    let fields = (0..128)
        .map(|i| schema.field(&format!("value_{i:03}"), None, None, false))
        .collect();
    schema.define("CRoot", fields);
    let flat = schema.finish("CRoot");
    for (name, path, expected) in [
        ("flat_first", "value_000", Some(key(&[0]))),
        ("flat_middle", "value_064", Some(key(&[64]))),
        ("flat_last", "value_127", Some(key(&[127]))),
        ("flat_miss_same_length", "value_999", None),
    ] {
        cases.push(LookupCase {
            name,
            serializer: Arc::clone(&flat),
            path: path.into(),
            expected,
        });
    }

    let mut schema = Schema::default();
    let fields = (0..128)
        .map(|i| {
            schema.field(
                "value",
                Some(&format!("network.component_{i:03}")),
                None,
                false,
            )
        })
        .collect();
    schema.define("CRoot", fields);
    let prefixed = schema.finish("CRoot");
    for (name, path, expected) in [
        (
            "prefix_last",
            "network.component_127.value",
            Some(key(&[127])),
        ),
        ("prefix_miss", "network.component_999.value", None),
    ] {
        cases.push(LookupCase {
            name,
            serializer: Arc::clone(&prefixed),
            path: path.into(),
            expected,
        });
    }

    let mut schema = Schema::default();
    for depth in (0..4).rev() {
        let child = (depth < 3).then(|| format!("CLevel{}", depth + 1));
        let fields = (0..16)
            .map(|i| schema.field(&format!("part_{i:02}"), None, child.as_deref(), false))
            .collect();
        schema.define(&format!("CLevel{depth}"), fields);
    }
    cases.push(LookupCase {
        name: "nested_depth_4",
        serializer: schema.finish("CLevel0"),
        path: "part_15.part_15.part_15.part_15".into(),
        expected: Some(key(&[15, 15, 15, 15])),
    });

    let mut schema = Schema::default();
    let fields = (0..16)
        .map(|i| schema.field(&format!("value_{i:02}"), Some("body.data"), None, false))
        .collect();
    schema.define("CLeaf", fields);
    let fields = (0..16)
        .map(|i| {
            schema.field(
                &format!("block_{i:02}"),
                Some("network.state"),
                Some("CLeaf"),
                false,
            )
        })
        .collect();
    schema.define("CRoot", fields);
    cases.push(LookupCase {
        name: "nested_send_nodes",
        serializer: schema.finish("CRoot"),
        path: "network.state.block_15.body.data.value_15".into(),
        expected: Some(key(&[15, 15])),
    });

    let mut schema = Schema::default();
    let leaf = schema.field("value", Some("node"), None, false);
    schema.define("CLeaf", vec![leaf]);
    let array = schema.field("items", None, Some("CLeaf"), true);
    schema.define("CRoot", vec![array]);
    let array = schema.finish("CRoot");
    for (name, path, expected) in [
        ("array_element", "items.17", Some(key(&[0, 17]))),
        (
            "array_nested",
            "items.17.node.value",
            Some(key(&[0, 17, 0])),
        ),
        ("array_invalid_index", "items.bad.node.value", None),
    ] {
        cases.push(LookupCase {
            name,
            serializer: Arc::clone(&array),
            path: path.into(),
            expected,
        });
    }
    cases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_cases_resolve_to_independent_keys() {
        for case in lookup_cases() {
            assert_eq!(
                case.serializer.resolve_field_key(&case.path),
                case.expected,
                "{}",
                case.name
            );
            if let Some(key) = case.expected {
                assert_eq!(
                    case.serializer.field_name_for_key(key).as_deref(),
                    Some(case.path.as_str()),
                    "{}",
                    case.name
                );
            }
        }
    }
}
