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
        let uuid = uuid::Uuid::new_v4();
        Policy {
            id: uuid.to_string(),
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
        let regex_str = format!("^{id}:(.*)$", id = id);
        let matcher = regex::Regex::new(&regex_str).unwrap();
        matcher.is_match(&resource)
    }

    fn includes_id(&self, id: &String) -> bool {
        self.resources
            .iter()
            .find(|resource| self.is_match_by_id(id, resource))
            .is_some()
    }

    fn add_to_existing(&mut self, id: &String, access: String) {
        let regex_str = format!("^({id}):(.*)$", id = id);
        let matcher = regex::Regex::new(&regex_str).unwrap();
        self.resources = self
            .resources
            .iter()
            .map(|resource| {
                if self.is_match_by_id(id, resource) {
                    String::from(matcher.replace(resource, |caps: &Captures| {
                        format!(
                            "{id}:{original_access},{new_access}",
                            id = id,
                            original_access = &caps[2],
                            new_access = access
                        )
                    }))
                } else {
                    String::from(resource)
                }
            })
            .collect();
    }

    pub fn add(&mut self, id: String, access: String) {
        if self.includes_id(&id) {
            self.add_to_existing(&id, access)
        } else {
            self.resources
                .push(format!("{id}:{access}", id = id, access = access))
        }
    }

    fn remove_id(&mut self, id: String) {
        let new_resources: Vec<String> = self
            .resources
            .iter()
            .filter(|resource| !self.is_match_by_id(&id, resource))
            .map(|resource| String::from(resource))
            .collect();
        self.resources = new_resources;
    }

    pub fn remove(&mut self, id: String, access: Option<String>) {
        if let Some(found_access) = access {
            let regex_str = format!("^({id}):(.*)$", id = id);
            let matcher = regex::Regex::new(&regex_str).unwrap();
            self.resources = self
                .resources
                .iter()
                .map(|resource| {
                    if self.is_match_by_id(&id, resource) {
                        String::from(matcher.replace(resource, |parts: &Captures| {
                            let adjusted_access = (&parts[2])
                                .split(",")
                                .filter(|specific_access| specific_access.ne(&found_access))
                                .fold(String::new(), |a, b| a + b + ",");
                            format!("{}:{}", id, adjusted_access.trim_end_matches(','))
                        }))
                    } else {
                        String::from(resource)
                    }
                })
                .collect()
        } else {
            self.remove_id(id)
        }
    }
}

#[cfg(test)]
mod tests {

    #[cfg(test)]
    mod default {
        use crate::policies::{Policy, PolicyEffect};
        use uuid::Uuid;

        #[test]
        fn it_should_deny_on_default() {
            let uuid = Uuid::new_v4().to_string();
            let access = String::from("get");
            let basic = Policy::default();
            assert_eq!(basic.can_i(uuid, access), false)
        }

        #[test]
        fn it_should_add_new_resource_when_not_found() {
            let uuid = Uuid::new_v4().to_string();
            let access = String::from("get");
            let mut basic = Policy::default();
            assert!(basic.resources.is_empty());
            basic.add(uuid.clone(), access.clone());
            assert_eq!(basic.resources[0], format!("{}:{}", uuid, access));
        }

        #[test]
        fn it_should_modify_existing_resource_when_found() {
            let uuid = Uuid::new_v4().to_string();
            let get_access = String::from("get");
            let post_access = String::from("post");
            let mut basic = Policy::default();
            assert!(basic.resources.is_empty());
            basic.add(uuid.clone(), get_access.clone());
            basic.add(uuid.clone(), post_access.clone());
            assert_eq!(
                basic.resources[0],
                format!("{}:{},{}", uuid, get_access, post_access)
            );
        }

        #[test]
        fn it_should_remove_resource_when_found() {
            let uuid = Uuid::new_v4().to_string();
            let access = String::from("get");
            let access_2 = String::from("post");
            let mut basic = Policy::default();
            assert!(basic.resources.is_empty());
            basic.add(uuid.clone(), access.clone());
            basic.add(uuid.clone(), access_2.clone());
            assert_eq!(
                basic.resources[0],
                format!("{}:{},{}", &uuid, &access, &access_2)
            );
            basic.remove(uuid.clone(), Some(access));
            assert_eq!(basic.resources[0], format!("{}:{}", uuid, access_2));
        }

        #[test]
        fn it_should_modify_amd_remove_existing_access_when_resource_found() {
            let uuid = Uuid::new_v4().to_string();
            let get_access = String::from("get");
            let post_access = String::from("post");
            let mut basic = Policy::default();
            assert!(basic.resources.is_empty());
            basic.add(uuid.clone(), get_access.clone());
            basic.add(uuid.clone(), post_access.clone());
            assert_eq!(
                basic.resources[0],
                format!("{}:{},{}", uuid, get_access, post_access)
            );
        }
    }

    #[cfg(test)]
    mod deny_basic {
        use crate::policies::{Policy, PolicyEffect};
        use uuid::Uuid;

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
    }

    #[cfg(test)]
    mod allow_basic {
        use crate::policies::{Policy, PolicyEffect};
        use uuid::Uuid;

        #[test]
        fn it_should_allow_on_effect_allow() {
            let uuid = Uuid::new_v4().to_string();
            let access = String::from("get");
            let mut policy = Policy::default_allow();
            policy.add(uuid.clone(), access.clone());
            assert_eq!(policy.can_i(uuid, access), true)
        }

        #[test]
        fn it_should_deny_on_allow_effect_and_not_found() {
            let uuid = Uuid::new_v4().to_string();
            let access = String::from("get");
            let policy = Policy::default_allow();
            assert_eq!(policy.can_i(uuid, access), false)
        }
    }
}
