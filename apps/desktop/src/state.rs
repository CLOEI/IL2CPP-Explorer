use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use il2cpp_core::analysis::Il2CppProject;
use il2cpp_core::model::{AssemblyId, FieldId, MethodId, TypeId};
use il2cpp_disasm::FunctionInspection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodTab {
    Overview,
    CSharp,
    Disassembly,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchResult {
    Type(TypeId),
    Method(MethodId),
    Field(FieldId),
}

#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub result: SearchResult,
    pub label: String,
    pub kind: &'static str,
}

pub enum LoadState {
    Empty,
    Loading {
        receiver: Receiver<Result<ProjectData, String>>,
        binary: PathBuf,
        metadata: PathBuf,
    },
    Loaded(ProjectData),
    Failed {
        message: String,
        binary: PathBuf,
        metadata: PathBuf,
    },
}

pub struct ProjectData {
    pub project: Arc<Il2CppProject>,
    pub binary_path: PathBuf,
    pub metadata_path: PathBuf,
    pub navigation: NavigationIndex,
    pub search_entries: Vec<SearchMatch>,
    pub csharp_types: HashMap<TypeId, String>,
    pub csharp_methods: HashMap<MethodId, String>,
    pub disassembly: HashMap<MethodId, Result<FunctionInspection, String>>,
}

#[derive(Default)]
pub struct NavigationIndex {
    pub assemblies: Vec<AssemblyNode>,
}

pub struct AssemblyNode {
    pub id: AssemblyId,
    pub name: String,
    pub namespaces: NamespaceNode,
}

#[derive(Default)]
pub struct NamespaceNode {
    pub children: BTreeMap<String, NamespaceNode>,
    pub types: Vec<TypeId>,
}

impl NavigationIndex {
    pub fn build(project: &Il2CppProject) -> Self {
        let metadata = project.metadata();
        let mut assemblies = Vec::with_capacity(metadata.assemblies.len());
        for assembly in &metadata.assemblies {
            let image = &metadata.images[assembly.image.0];
            let mut namespaces = NamespaceNode::default();
            for type_id in &image.types {
                let ty = &metadata.types[type_id.0];
                if ty.nested_in.is_some() {
                    continue;
                }
                let mut node = &mut namespaces;
                for segment in ty
                    .namespace
                    .split('.')
                    .filter(|segment| !segment.is_empty())
                {
                    node = node.children.entry(segment.to_owned()).or_default();
                }
                node.types.push(*type_id);
            }
            sort_namespace_types(&mut namespaces, project);
            assemblies.push(AssemblyNode {
                id: assembly.id,
                name: assembly.name.clone(),
                namespaces,
            });
        }
        assemblies.sort_by(|left, right| left.name.cmp(&right.name));
        Self { assemblies }
    }
}

fn sort_namespace_types(node: &mut NamespaceNode, project: &Il2CppProject) {
    node.types.sort_by(|left, right| {
        project.metadata().types[left.0]
            .name
            .cmp(&project.metadata().types[right.0].name)
    });
    for child in node.children.values_mut() {
        sort_namespace_types(child, project);
    }
}

impl ProjectData {
    pub fn new(project: Arc<Il2CppProject>, binary_path: PathBuf, metadata_path: PathBuf) -> Self {
        let navigation = NavigationIndex::build(&project);
        let search_entries = build_search_entries(&project);
        Self {
            project,
            binary_path,
            metadata_path,
            navigation,
            search_entries,
            csharp_types: HashMap::new(),
            csharp_methods: HashMap::new(),
            disassembly: HashMap::new(),
        }
    }
}

pub fn search(entries: &[SearchMatch], query: &str) -> (Vec<SearchMatch>, bool) {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return (Vec::new(), false);
    }
    let matches = entries
        .iter()
        .filter(|entry| entry.label.to_lowercase().contains(&query))
        .cloned()
        .collect::<Vec<_>>();
    let limited = matches.len() > 100;
    (matches.into_iter().take(100).collect(), limited)
}

fn build_search_entries(project: &Il2CppProject) -> Vec<SearchMatch> {
    let metadata = project.metadata();
    let mut entries =
        Vec::with_capacity(metadata.types.len() + metadata.methods.len() + metadata.fields.len());
    for ty in &metadata.types {
        entries.push(SearchMatch {
            result: SearchResult::Type(ty.id),
            label: type_name(project, ty.id),
            kind: "Type",
        });
    }
    for method in &metadata.methods {
        entries.push(SearchMatch {
            result: SearchResult::Method(method.id),
            label: format!(
                "{}::{}",
                type_name(project, method.declaring_type),
                method.name
            ),
            kind: "Method",
        });
    }
    for field in &metadata.fields {
        entries.push(SearchMatch {
            result: SearchResult::Field(field.id),
            label: format!(
                "{}::{}",
                type_name(project, field.declaring_type),
                field.name
            ),
            kind: "Field",
        });
    }
    entries
}

pub fn type_name(project: &Il2CppProject, type_id: TypeId) -> String {
    let ty = &project.metadata().types[type_id.0];
    if ty.namespace.is_empty() {
        ty.name.clone()
    } else {
        format!("{}.{}", ty.namespace, ty.name)
    }
}

pub fn format_hex(value: Option<u64>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| format!("0x{value:08X}"))
}

pub fn format_token(value: u32) -> String {
    format!("0x{value:08X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_addresses_consistently() {
        assert_eq!(format_hex(Some(0x18f_4520)), "0x018F4520");
        assert_eq!(format_hex(None), "-");
        assert_eq!(format_token(0x0600_1234), "0x06001234");
    }

    #[test]
    fn search_is_case_insensitive_and_limited() {
        let entries = vec![SearchMatch {
            result: SearchResult::Type(TypeId(0)),
            label: "Game.Player.PlayerController".to_owned(),
            kind: "Type",
        }];
        let (matches, limited) = search(&entries, "playercontroller");
        assert_eq!(matches.len(), 1);
        assert!(!limited);
    }
}
