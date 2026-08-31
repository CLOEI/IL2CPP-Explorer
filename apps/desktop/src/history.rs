use crate::navigation::NavigationTarget;

#[derive(Default)]
pub struct NavigationHistory {
    back: Vec<NavigationTarget>,
    current: Option<NavigationTarget>,
    forward: Vec<NavigationTarget>,
}
impl NavigationHistory {
    pub fn navigate(&mut self, target: NavigationTarget) -> bool {
        if self.current == Some(target) {
            return false;
        }
        if let Some(current) = self.current {
            self.back.push(current);
        }
        self.current = Some(target);
        self.forward.clear();
        true
    }
    pub fn back(&mut self) -> Option<NavigationTarget> {
        let target = self.back.pop()?;
        if let Some(current) = self.current {
            self.forward.push(current);
        }
        self.current = Some(target);
        self.current
    }
    pub fn forward(&mut self) -> Option<NavigationTarget> {
        let target = self.forward.pop()?;
        if let Some(current) = self.current {
            self.back.push(current);
        }
        self.current = Some(target);
        self.current
    }
    pub fn can_back(&self) -> bool {
        !self.back.is_empty()
    }
    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::AddressTarget;
    #[test]
    fn history_back_forward_and_clear() {
        let mut h = NavigationHistory::default();
        h.navigate(NavigationTarget::ProjectOverview);
        h.navigate(NavigationTarget::Address(AddressTarget::Rva(1)));
        assert!(h.can_back());
        assert_eq!(h.back(), Some(NavigationTarget::ProjectOverview));
        assert!(h.can_forward());
        h.navigate(NavigationTarget::Address(AddressTarget::Rva(2)));
        assert!(!h.can_forward());
        assert!(!h.navigate(NavigationTarget::Address(AddressTarget::Rva(2))));
    }
}
