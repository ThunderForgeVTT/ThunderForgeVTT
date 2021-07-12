use std::str::FromStr;

pub enum PolicyEffect {
    Allow,
    Deny,
}

impl ToString for PolicyEffect {
    fn to_string(&self) -> String {
        use PolicyEffect::{Allow, Deny};
        match &self {
            Allow => String::from("Allow"),
            Deny => String::from("Deny"),
        }
    }
}

impl PolicyEffect {
    fn is_allowed(&self) -> bool {
        use PolicyEffect::{Allow, Deny};
        match &self {
            Allow => true,
            Deny => false,
        }
    }
    fn is_deny(&self) -> bool {
        !self.is_allowed()
    }
}

pub struct Policy {
    user_id: Option<String>,
    effect: PolicyEffect,
    resources: Vec<String>,
}

impl Default for Policy {
    fn default() -> Policy {
        Policy {
            user_id: None,
            effect: PolicyEffect::Deny,
            resources: vec![],
        }
    }
}

impl Policy {
    fn looking_for(&self, id: String, access: String, to_be: bool) -> bool {
        use PolicyEffect::{Allow, Deny};
        let regex_str = format!("^{id}:(.*{access}.*)$", id = id, access = access);
        let matcher = regex::Regex::new(&regex_str).unwrap();
        let is_match = self
            .resources
            .iter()
            .find(|resource| matcher.is_match(&resource))
            .is_some();

        match self.effect {
            Allow => is_match && to_be,
            Deny => (is_match && !to_be),
        }
    }

    pub fn can_i(&self, id: String, access: String) -> bool {
        let is_allowed = self.effect.is_allowed();
        self.looking_for(id, access, is_allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::Policy;
    use rocket::serde::uuid::Uuid;

    #[test]
    fn it_should_deny_on_default() {
        let uuid = Uuid::default().to_string();
        let access = String::from("get");
        let basic = Policy::default();
        assert_eq!(basic.can_i(uuid, access), false)
    }
}
