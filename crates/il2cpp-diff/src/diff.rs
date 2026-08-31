use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::Result;
use il2cpp_core::analysis::{Il2CppProject, TypeResolver};
use il2cpp_core::model::{MethodId, TypeRef};

use crate::identity::{IdentityCache, MethodMatchKey, TypeIdentity, TypeIdentityRef};
use crate::native::{fingerprint, native_diff};
use crate::{
    AssemblyDiff, DiffStatus, DiffSummary, FieldDiff, MethodDiff, ProjectDiff, PropertyDiff,
    TypeDiff,
};

#[derive(Debug, Clone, Copy)]
pub struct DiffOptions {
    pub compare_native_bodies: bool,
    pub calculate_similarity: bool,
}
impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            compare_native_bodies: true,
            calculate_similarity: true,
        }
    }
}

/// Reusable diff engine. Matching indexes once, then compares maps in deterministic order.
pub struct DiffEngine<'a> {
    old: &'a Il2CppProject,
    new: &'a Il2CppProject,
    options: DiffOptions,
}

impl<'a> DiffEngine<'a> {
    pub fn new(old: &'a Il2CppProject, new: &'a Il2CppProject) -> Self {
        Self {
            old,
            new,
            options: DiffOptions::default(),
        }
    }
    pub const fn with_options(mut self, options: DiffOptions) -> Self {
        self.options = options;
        self
    }

    pub fn compare(&self) -> Result<ProjectDiff> {
        let old_resolver = resolver(self.old);
        let new_resolver = resolver(self.new);
        let old_cache = IdentityCache::build(self.old.metadata(), &old_resolver);
        let new_cache = IdentityCache::build(self.new.metadata(), &new_resolver);
        let assemblies = assembly_diffs(self.old, self.new);
        let mut old_types = BTreeMap::new();
        let mut new_types = BTreeMap::new();
        for (index, identity) in old_cache.types.iter().cloned().enumerate() {
            old_types.insert(identity, index);
        }
        for (index, identity) in new_cache.types.iter().cloned().enumerate() {
            new_types.insert(identity, index);
        }
        let keys = old_types
            .keys()
            .chain(new_types.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut types = keys
            .into_iter()
            .map(|identity| {
                self.type_diff(
                    &identity,
                    old_types.get(&identity).copied(),
                    new_types.get(&identity).copied(),
                    &old_cache,
                    &new_cache,
                    &old_resolver,
                    &new_resolver,
                )
            })
            .collect::<Vec<_>>();
        types.sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut summary = DiffSummary::default();
        for assembly in &assemblies {
            match assembly.status {
                DiffStatus::Added => summary.assemblies_added += 1,
                DiffStatus::Removed => summary.assemblies_removed += 1,
                _ => {}
            }
        }
        for ty in &types {
            match ty.status {
                DiffStatus::Added => summary.types_added += 1,
                DiffStatus::Removed => summary.types_removed += 1,
                DiffStatus::Changed | DiffStatus::Moved => summary.types_changed += 1,
                DiffStatus::Unchanged => {}
            }
            for method in &ty.methods {
                match method.status {
                    DiffStatus::Added => summary.methods_added += 1,
                    DiffStatus::Removed => summary.methods_removed += 1,
                    DiffStatus::Changed => summary.methods_changed += 1,
                    DiffStatus::Moved => summary.methods_moved += 1,
                    DiffStatus::Unchanged => {}
                }
            }
        }
        Ok(ProjectDiff {
            assemblies,
            types,
            summary,
            native_available: self.old.native_methods().is_some()
                && self.new.native_methods().is_some(),
        })
    }

    #[allow(clippy::too_many_arguments)] // paired-project caches and resolvers stay borrow-local.
    fn type_diff(
        &self,
        identity: &TypeIdentity,
        old_id: Option<usize>,
        new_id: Option<usize>,
        old_cache: &IdentityCache,
        new_cache: &IdentityCache,
        old_resolver: &TypeResolver<'_>,
        new_resolver: &TypeResolver<'_>,
    ) -> TypeDiff {
        let old_type = old_id.map(|id| &self.old.metadata().types[id]);
        let new_type = new_id.map(|id| &self.new.metadata().types[id]);
        let old_base = old_type
            .and_then(|ty| ty.parent.and_then(|index| old_resolver.resolve(index).ok()))
            .map(|value| normalized_ref(old_cache, value));
        let new_base = new_type
            .and_then(|ty| ty.parent.and_then(|index| new_resolver.resolve(index).ok()))
            .map(|value| normalized_ref(new_cache, value));
        let old_interfaces = old_type.map_or_else(Vec::new, |ty| {
            ty.interfaces
                .iter()
                .filter_map(|index| old_resolver.resolve(*index).ok())
                .map(|value| normalized_ref(old_cache, value))
                .collect()
        });
        let new_interfaces = new_type.map_or_else(Vec::new, |ty| {
            ty.interfaces
                .iter()
                .filter_map(|index| new_resolver.resolve(*index).ok())
                .map(|value| normalized_ref(new_cache, value))
                .collect()
        });
        let fields = field_diffs(
            self.old,
            self.new,
            old_type.map(|v| v.id.0),
            new_type.map(|v| v.id.0),
            old_cache,
            new_cache,
        );
        let properties = property_diffs(
            self.old,
            self.new,
            old_type.map(|v| v.id.0),
            new_type.map(|v| v.id.0),
            old_cache,
            new_cache,
        );
        let methods = method_diffs(
            self,
            old_type.map(|v| v.id.0),
            new_type.map(|v| v.id.0),
            old_cache,
            new_cache,
        );
        let status = match (old_type, new_type) {
            (None, Some(_)) => DiffStatus::Added,
            (Some(_), None) => DiffStatus::Removed,
            (Some(old), Some(new))
                if old_base != new_base
                    || old_interfaces != new_interfaces
                    || old.flags != new.flags
                    || old.bitfield != new.bitfield
                    || fields.iter().any(|item| item.status.is_changed())
                    || properties.iter().any(|item| item.status.is_changed())
                    || methods.iter().any(|item| item.status.is_changed()) =>
            {
                DiffStatus::Changed
            }
            _ => DiffStatus::Unchanged,
        };
        TypeDiff {
            identity: identity.clone(),
            status,
            old_base,
            new_base,
            old_interfaces,
            new_interfaces,
            old_flags: old_type.map(|v| v.flags),
            new_flags: new_type.map(|v| v.flags),
            fields,
            properties,
            methods,
        }
    }
}

fn resolver(project: &Il2CppProject) -> TypeResolver<'_> {
    project.runtime_metadata().map_or_else(
        || TypeResolver::metadata_only(project.metadata()),
        |runtime| TypeResolver::with_runtime(project.metadata(), project.binary(), runtime),
    )
}

fn assembly_diffs(old: &Il2CppProject, new: &Il2CppProject) -> Vec<AssemblyDiff> {
    let old_names = old
        .metadata()
        .assemblies
        .iter()
        .map(|item| item.name.clone())
        .collect::<BTreeSet<_>>();
    let new_names = new
        .metadata()
        .assemblies
        .iter()
        .map(|item| item.name.clone())
        .collect::<BTreeSet<_>>();
    old_names
        .union(&new_names)
        .map(|name| AssemblyDiff {
            name: name.clone(),
            old_present: old_names.contains(name),
            new_present: new_names.contains(name),
            status: match (old_names.contains(name), new_names.contains(name)) {
                (true, true) => DiffStatus::Unchanged,
                (true, false) => DiffStatus::Removed,
                (false, true) => DiffStatus::Added,
                (false, false) => unreachable!(),
            },
        })
        .collect()
}

fn field_diffs(
    old: &Il2CppProject,
    new: &Il2CppProject,
    old_type: Option<usize>,
    new_type: Option<usize>,
    old_cache: &IdentityCache,
    new_cache: &IdentityCache,
) -> Vec<FieldDiff> {
    let mut old_fields = BTreeMap::new();
    let mut new_fields = BTreeMap::new();
    if let Some(id) = old_type {
        for field in &old.metadata().types[id].fields {
            old_fields.insert(old.metadata().fields[field.0].name.clone(), field.0);
        }
    }
    if let Some(id) = new_type {
        for field in &new.metadata().types[id].fields {
            new_fields.insert(new.metadata().fields[field.0].name.clone(), field.0);
        }
    }
    old_fields
        .keys()
        .chain(new_fields.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|name| {
            let old_field = old_fields.get(&name).map(|id| &old.metadata().fields[*id]);
            let new_field = new_fields.get(&name).map(|id| &new.metadata().fields[*id]);
            let old_ty = old_field.map(|field| old_cache.field_types[field.id.0].clone());
            let new_ty = new_field.map(|field| new_cache.field_types[field.id.0].clone());
            let status = match (old_field, new_field) {
                (None, Some(_)) => DiffStatus::Added,
                (Some(_), None) => DiffStatus::Removed,
                (Some(old), Some(new)) if old_ty != new_ty || old.offset != new.offset => {
                    DiffStatus::Changed
                }
                _ => DiffStatus::Unchanged,
            };
            FieldDiff {
                name,
                status,
                old_type: old_ty,
                new_type: new_ty,
                old_offset: old_field.and_then(|v| v.offset),
                new_offset: new_field.and_then(|v| v.offset),
                old_static: None,
                new_static: None,
            }
        })
        .collect()
}

fn property_diffs(
    old: &Il2CppProject,
    new: &Il2CppProject,
    old_type: Option<usize>,
    new_type: Option<usize>,
    old_cache: &IdentityCache,
    new_cache: &IdentityCache,
) -> Vec<PropertyDiff> {
    let mut old_values = BTreeMap::new();
    let mut new_values = BTreeMap::new();
    if let Some(id) = old_type {
        for property in &old.metadata().types[id].properties {
            old_values.insert(
                old.metadata().properties[property.0].name.clone(),
                property.0,
            );
        }
    }
    if let Some(id) = new_type {
        for property in &new.metadata().types[id].properties {
            new_values.insert(
                new.metadata().properties[property.0].name.clone(),
                property.0,
            );
        }
    }
    old_values
        .keys()
        .chain(new_values.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|name| {
            let left = old_values
                .get(&name)
                .map(|id| &old.metadata().properties[*id]);
            let right = new_values
                .get(&name)
                .map(|id| &new.metadata().properties[*id]);
            let getter = |value: Option<&il2cpp_core::model::Property>, cache: &IdentityCache| {
                value
                    .and_then(|v| v.getter)
                    .and_then(|id| cache.methods.get(id.0))
                    .map(ToString::to_string)
            };
            let setter = |value: Option<&il2cpp_core::model::Property>, cache: &IdentityCache| {
                value
                    .and_then(|v| v.setter)
                    .and_then(|id| cache.methods.get(id.0))
                    .map(ToString::to_string)
            };
            let old_getter = getter(left, old_cache);
            let new_getter = getter(right, new_cache);
            let old_setter = setter(left, old_cache);
            let new_setter = setter(right, new_cache);
            let status = match (left, right) {
                (None, Some(_)) => DiffStatus::Added,
                (Some(_), None) => DiffStatus::Removed,
                (Some(left), Some(right))
                    if old_getter != new_getter
                        || old_setter != new_setter
                        || left.attributes != right.attributes =>
                {
                    DiffStatus::Changed
                }
                _ => DiffStatus::Unchanged,
            };
            PropertyDiff {
                name,
                status,
                old_getter,
                new_getter,
                old_setter,
                new_setter,
                old_attributes: left.map(|v| v.attributes),
                new_attributes: right.map(|v| v.attributes),
            }
        })
        .collect()
}

fn method_diffs(
    engine: &DiffEngine<'_>,
    old_type: Option<usize>,
    new_type: Option<usize>,
    old_cache: &IdentityCache,
    new_cache: &IdentityCache,
) -> Vec<MethodDiff> {
    let old_ids = old_type.map_or_else(Vec::new, |id| {
        engine.old.metadata().types[id]
            .methods
            .iter()
            .map(|item| item.0)
            .collect::<Vec<_>>()
    });
    let new_ids = new_type.map_or_else(Vec::new, |id| {
        engine.new.metadata().types[id]
            .methods
            .iter()
            .map(|item| item.0)
            .collect::<Vec<_>>()
    });
    let all_old = old_ids.clone();
    let all_new = new_ids.clone();
    let mut old_by_identity = HashMap::new();
    let mut new_by_identity = HashMap::new();
    for id in &all_old {
        old_by_identity
            .entry(old_cache.methods[*id].clone())
            .or_insert_with(Vec::new)
            .push(*id);
    }
    for id in &all_new {
        new_by_identity
            .entry(new_cache.methods[*id].clone())
            .or_insert_with(Vec::new)
            .push(*id);
    }
    let mut old_by_key = HashMap::<MethodMatchKey, Vec<usize>>::new();
    let mut new_by_key = HashMap::<MethodMatchKey, Vec<usize>>::new();
    for id in old_ids {
        old_by_key
            .entry(old_cache.method_keys[id].clone())
            .or_default()
            .push(id);
    }
    for id in new_ids {
        new_by_key
            .entry(new_cache.method_keys[id].clone())
            .or_default()
            .push(id);
    }
    let mut pairs = Vec::new();
    let mut used_old = HashSet::new();
    let mut used_new = HashSet::new();
    // Exact full identities resolve overloaded conversion operators before signature-change matching.
    for identity in old_by_identity
        .keys()
        .chain(new_by_identity.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let left = old_by_identity.get(&identity).cloned().unwrap_or_default();
        let right = new_by_identity.get(&identity).cloned().unwrap_or_default();
        if left.len() == 1 && right.len() == 1 {
            used_old.insert(left[0]);
            used_new.insert(right[0]);
            pairs.push((Some(left[0]), Some(right[0])));
        }
    }
    for key in old_by_key
        .keys()
        .chain(new_by_key.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let left = old_by_key
            .get(&key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|id| !used_old.contains(id))
            .collect::<Vec<_>>();
        let right = new_by_key
            .get(&key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|id| !used_new.contains(id))
            .collect::<Vec<_>>();
        if left.len() == 1 && right.len() == 1 {
            used_old.insert(left[0]);
            used_new.insert(right[0]);
            pairs.push((Some(left[0]), Some(right[0])));
        }
    }
    // Conservative secondary match: only unambiguous same-name/static/generic candidate per side.
    let mut secondary_old = HashMap::<(String, usize, bool), Vec<usize>>::new();
    let mut secondary_new = HashMap::<(String, usize, bool), Vec<usize>>::new();
    for (_, ids) in old_by_key {
        for id in ids.into_iter().filter(|id| !used_old.contains(id)) {
            let item = &old_cache.methods[id];
            secondary_old
                .entry((item.name.clone(), item.generic_arity, item.is_static))
                .or_default()
                .push(id);
        }
    }
    for (_, ids) in new_by_key {
        for id in ids.into_iter().filter(|id| !used_new.contains(id)) {
            let item = &new_cache.methods[id];
            secondary_new
                .entry((item.name.clone(), item.generic_arity, item.is_static))
                .or_default()
                .push(id);
        }
    }
    for key in secondary_old
        .keys()
        .chain(secondary_new.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let left = secondary_old.get(&key).cloned().unwrap_or_default();
        let right = secondary_new.get(&key).cloned().unwrap_or_default();
        if left.len() == 1 && right.len() == 1 {
            used_old.insert(left[0]);
            used_new.insert(right[0]);
            pairs.push((Some(left[0]), Some(right[0])));
        }
    }
    for id in all_old.into_iter().filter(|id| !used_old.contains(id)) {
        pairs.push((Some(id), None));
    }
    for id in all_new.into_iter().filter(|id| !used_new.contains(id)) {
        pairs.push((None, Some(id)));
    }
    let mut result = pairs
        .into_iter()
        .map(|(left, right)| method_diff(engine, left, right, old_cache, new_cache))
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.identity.cmp(&right.identity));
    result
}

fn method_diff(
    engine: &DiffEngine<'_>,
    old: Option<usize>,
    new: Option<usize>,
    old_cache: &IdentityCache,
    new_cache: &IdentityCache,
) -> MethodDiff {
    let old_method = old.map(|id| &engine.old.metadata().methods[id]);
    let new_method = new.map(|id| &engine.new.metadata().methods[id]);
    let identity = new
        .or(old)
        .map(|id| {
            if new.is_some() {
                new_cache.methods[id].clone()
            } else {
                old_cache.methods[id].clone()
            }
        })
        .expect("method pair contains an item");
    let old_address = old_method.and_then(|value| {
        engine
            .old
            .native_methods()
            .and_then(|methods| methods.address_of(value.id))
    });
    let new_address = new_method.and_then(|value| {
        engine
            .new
            .native_methods()
            .and_then(|methods| methods.address_of(value.id))
    });
    let old_rva = old_address.map(|value| value.relative_address);
    let new_rva = new_address.map(|value| value.relative_address);
    let metadata_changed = old_method.zip(new_method).is_some_and(|(left, right)| {
        old_cache.methods[left.id.0] != new_cache.methods[right.id.0]
            || left.flags != right.flags
            || left.implementation_flags != right.implementation_flags
    });
    let native = if engine.options.compare_native_bodies && old.is_some() && new.is_some() {
        native_diff(
            old.and_then(|id| fingerprint(engine.old, MethodId(id), &old_cache.methods)),
            new.and_then(|id| fingerprint(engine.new, MethodId(id), &new_cache.methods)),
            engine.options.calculate_similarity,
        )
    } else {
        None
    };
    let status = match (old_method, new_method) {
        (None, Some(_)) => DiffStatus::Added,
        (Some(_), None) => DiffStatus::Removed,
        (Some(_), Some(_))
            if metadata_changed
                || native
                    .as_ref()
                    .is_some_and(|item| item.equivalent == Some(false)) =>
        {
            DiffStatus::Changed
        }
        (Some(_), Some(_))
            if old_rva != new_rva
                && native
                    .as_ref()
                    .is_some_and(|item| item.equivalent == Some(true)) =>
        {
            DiffStatus::Moved
        }
        _ => DiffStatus::Unchanged,
    };
    MethodDiff {
        identity,
        status,
        old_rva,
        new_rva,
        old_va: old_address.map(|v| v.virtual_address),
        new_va: new_address.map(|v| v.virtual_address),
        old_token: old_method.map(|v| v.token),
        new_token: new_method.map(|v| v.token),
        native,
    }
}

fn normalized_ref(cache: &IdentityCache, value: TypeRef) -> TypeIdentityRef {
    match value {
        TypeRef::Type(id) => cache
            .types
            .get(id.0)
            .cloned()
            .map(TypeIdentityRef::Named)
            .unwrap_or_else(|| TypeIdentityRef::Unknown("type".to_owned())),
        TypeRef::Unknown {
            type_index: Some(index),
            raw_type: None,
        } => cache
            .type_indexes
            .get(&index)
            .cloned()
            .map(TypeIdentityRef::Named)
            .unwrap_or_else(|| TypeIdentityRef::Unknown(format!("unknown:{index}:none"))),
        _ => TypeIdentityRef::Unknown(format!("{value:?}")),
    }
}
