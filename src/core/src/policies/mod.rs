use uuid::Uuid;
use regex::Captures;

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
    id: String,
    effect: PolicyEffect,
    resources: Vec<String>,
}

impl Default for Policy {
    fn default() -> Policy {
        Policy {
            id: Uuid::new_v4().to_string(),
            effect: PolicyEffect::Deny,
            resources: vec![],
        }
    }
}

impl Policy {
    fn default_allow() -> Policy {
        let mut policy = Policy::default();
        policy.effect = PolicyEffect::Allow;
        policy
    }
    fn default_deny() -> Policy {
        let mut policy = Policy::default();
        policy.effect = PolicyEffect::Deny;
        policy
    }
    fn looking_for(&self, id: String, access: String, to_be: bool) -> bool {
        let regex_str = format!("^{id}:(.*{access}.*)$", id = id, access = access);
        let matcher = regex::Regex::new(&regex_str).unwrap();
        let is_match = self
            .resources
            .iter()
            .find(|resource| matcher.is_match(&resource))
            .is_some();
        is_match && to_be
    }

    pub fn can_i(&self, id: String, access: String) -> bool {
        let is_allowed = self.effect.is_allowed();
        self.looking_for(id, access, is_allowed)
    }

    fn is_match_by_id(&self, id: &String, resource: &String) -> bool {
        let regex_str = format!("^{id}:.*)$", id = id);
        let matcher = regex::Regex::new(&regex_str).unwrap();
        matcher.is_match(&resource)
    }

    fn includes_id(&self, id: &String) -> bool {

        self
            .resources
            .iter()
            .find(|resource| self.is_match_by_id(id, resource))
            .is_some()
    }

    fn add_to_existing(&mut self, id: &String, access: String) {
        let regex_str = format!("^({id}):(.*)$", id = id);
        let matcher = regex::Regex::new(&regex_str).unwrap();
        self.resources = self.resources.iter().map(|resource| {
            if self.is_match_by_id(id, resource) {
                String::from(matcher.replace(resource, |caps: &Captures| {
                    format!(
                        "{id}:{original_access},{new_access}",
                        id=id,
                        original_access=&caps[2],
                        new_access=access
                    )
                }))
            } else {
                String::from(resource)
            }
        }).collect();
    }

    // fn remove_from_existing(&self, id: &String, access: String) {
    //
    // }

    pub fn add(&mut self, id: String, access: String)
    {
        if self.includes_id(&id) {
            self.add_to_existing(&id, access)
        } else {
            self.resources.push(format!(
                "{id}:{access}",
                id=id,
                access=access
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Policy;
    use uuid::Uuid;
    use crate::policies::PolicyEffect;

    #[test]
    fn it_should_deny_on_default() {
        let uuid = Uuid::new_v4().to_string();
        let access = String::from("get");
        let basic = Policy::default();
        assert_eq!(basic.can_i(uuid, access), false)
    }

    #[test]
    fn it_should_deny_on_deny_effect_and_not_found() {
        let uuid = Uuid::new_v4().to_string();
        let access = String::from("get");
        let policy = Policy::default_deny();
        assert_eq!(policy.can_i(uuid, access), false)
    }

    #[test]
    fn it_should_deny_on_effect_deny() {
        let uuid = Uuid::new_v4().to_string();
        let access = String::from("get");
        let mut policy = Policy::default_deny();
        policy.add(uuid.clone(), access.clone());
        assert_eq!(policy.can_i(uuid, access), false)
    }

    #[test]
    fn it_should_deny_on_allow_effect_and_not_found() {
        let uuid = Uuid::new_v4().to_string();
        let access = String::from("get");
        let policy = Policy::default_allow();
        assert_eq!(policy.can_i(uuid, access), false)
    }

    #[test]
    fn it_should_allow_on_effect_allow() {
        let uuid = Uuid::new_v4().to_string();
        let access = String::from("get");
        let mut policy = Policy::default_allow();
        policy.add(uuid.clone(), access.clone());
        assert_eq!(policy.can_i(uuid, access), true)
    }
}
