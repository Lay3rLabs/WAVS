#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zindex {
    Modal,
    DropdownOptions,
}

impl Zindex {
    pub fn value(&self) -> &'static str {
        match self {
            Self::Modal => "1000",
            Self::DropdownOptions => "900",
        }
    }
}
