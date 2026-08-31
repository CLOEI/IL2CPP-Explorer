use il2cpp_core::model::{AssemblyId, FieldId, MethodId, PropertyId, TypeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddressTarget {
    Rva(u64),
    Va(u64),
    FileOffset(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum NavigationTarget {
    ProjectOverview,
    Assembly(AssemblyId),
    Type(TypeId),
    Field(FieldId),
    Property(PropertyId),
    Method(MethodId),
    Address(AddressTarget),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplorerTab {
    pub id: u64,
    pub target: NavigationTarget,
    pub title: String,
}

#[derive(Default)]
pub struct TabState {
    pub tabs: Vec<ExplorerTab>,
    pub active: Option<usize>,
    next_id: u64,
}

impl TabState {
    pub fn replace_active(&mut self, target: NavigationTarget, title: String) {
        if let Some(index) = self.active.and_then(|index| self.tabs.get_mut(index)) {
            index.target = target;
            index.title = title;
        } else {
            self.open(target, title);
        }
    }
    pub fn open(&mut self, target: NavigationTarget, title: String) {
        if let Some(index) = self.tabs.iter().position(|tab| tab.target == target) {
            self.active = Some(index);
            return;
        }
        self.next_id += 1;
        self.tabs.push(ExplorerTab {
            id: self.next_id,
            target,
            title,
        });
        self.active = Some(self.tabs.len() - 1);
    }
    pub fn close(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        self.active = if self.tabs.is_empty() {
            None
        } else {
            Some(index.min(self.tabs.len() - 1))
        };
    }
}

pub fn parse_address(value: &str) -> Result<u64, &'static str> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Address is required.");
    }
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    u64::from_str_radix(hex, 16)
        .or_else(|_| value.parse())
        .map_err(|_| "Invalid hexadecimal address.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_hex_formats() {
        assert_eq!(parse_address("0x18F4520"), Ok(0x18F4520));
        assert_eq!(parse_address("18F4520"), Ok(0x18F4520));
        assert!(parse_address("oops").is_err());
    }
    #[test]
    fn tabs_close_to_neighbour() {
        let mut tabs = TabState::default();
        tabs.open(NavigationTarget::ProjectOverview, "Project".into());
        tabs.open(NavigationTarget::Address(AddressTarget::Rva(1)), "1".into());
        tabs.close(1);
        assert_eq!(tabs.active, Some(0));
    }
}
