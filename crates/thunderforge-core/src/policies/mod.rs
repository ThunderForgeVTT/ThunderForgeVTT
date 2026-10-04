use regex::Captures;
use std::fmt::{Display, Formatter, Result};

pub enum PolicyEffect {
    Allow,
    Deny,
}

impl Display for PolicyEffect {
    fn fmt(&self, f: &mut Formatter) -> Result {
        use PolicyEffect::{Allow, Deny};
        match &self {
            Allow => write!(f, "allow"),
            Deny => write!(f, "deny"),
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
    // fn is_deny(&self) -> bool {
    //     !self.is_allowed()
    // }
}

pub struct Policy {
    // id: String,
    effect: PolicyEffect,
    resources: Vec<String>,
}

impl Default for Policy {
    fn default() -> Policy {
        // let uuid = uuid::Uuid::new_v4();
        Policy {
            // id: uuid.to_string(),
            effect: PolicyEffect::Deny,
            resources: vec![],
        }
    }
}

impl Policy {
    // fn default_allow() -> Policy {
    //     let mut policy = Policy::default();
    //     policy.effect = PolicyEffect::Allow;
    //     policy
    // }
    // fn default_deny() -> Policy {
    //     let mut policy = Policy::default();
    //     policy.effect = PolicyEffect::Deny;
    //     policy
    // }
    fn looking_for(&self, id: String, access: String, to_be: bool) -> bool {
        let regex_str = format!("^{id}:(.*{access}.*)$", id = id, access = access);
        let matcher = regex::Regex::new(&regex_str).unwrap();
        let is_match = self
            .resources
            .iter()
            .find(|resource| matcher.is_match(resource))
            .is_some();
        is_match && to_be
    }

    pub fn can_i(&self, id: String, access: String) -> bool {
        let is_allowed = self.effect.is_allowed();
        self.looking_for(id, access, is_allowed)
    }

    fn is_match_by_id(&self, id: &String, resource: &str) -> bool {
        let regex_str = format!("^{id}:(.*)$", id = id);
        let matcher = regex::Regex::new(&regex_str).unwrap();
        matcher.is_match(resource)
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
            .map(String::from)
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
                            let adjusted_access = parts[2]
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
            assert!(!basic.can_i(uuid, access))
        }

        /// The effect itself, which the test above cannot see.
        ///
        /// `it_should_deny_on_default` uses a policy with **no resources**, so
        /// `can_i` returns false through "nothing matched" whatever the effect
        /// is. A mutation audit on 2026-09-02 changed `Default` to `Allow` and
        /// every policy test stayed green: the authorisation primitive could be
        /// wholly inverted without a single failure.
        ///
        /// These two supply a resource that *does* match, so the only thing
        /// left deciding the answer is the effect.
        #[test]
        fn an_allowing_policy_permits_a_resource_it_lists() {
            let uuid = Uuid::new_v4().to_string();
            let access = String::from("get");
            let mut allowing = Policy {
                effect: PolicyEffect::Allow,
                resources: vec![],
            };
            allowing.add(uuid.clone(), access.clone());

            assert!(
                allowing.can_i(uuid, access),
                "an allow policy listing this resource must permit it, or the \
                 effect is not being read at all"
            );
        }

        #[test]
        fn a_denying_policy_refuses_a_resource_it_lists() {
            let uuid = Uuid::new_v4().to_string();
            let access = String::from("get");
            let mut denying = Policy {
                effect: PolicyEffect::Deny,
                resources: vec![],
            };
            denying.add(uuid.clone(), access.clone());

            assert!(
                !denying.can_i(uuid, access),
                "listing a resource must not permit it under a deny effect"
            );
        }

        /// And the default is the denying one — asserted against the effect
        /// rather than against an empty resource list.
        #[test]
        fn the_default_effect_is_deny() {
            let uuid = Uuid::new_v4().to_string();
            let access = String::from("get");
            let mut default = Policy::default();
            default.add(uuid.clone(), access.clone());

            assert!(
                !default.can_i(uuid, access),
                "a default policy that lists a resource must still refuse it"
            );
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

        /// Removing a resource entirely, which nothing covered.
        ///
        /// A mutation audit on 2026-09-02 gutted `Policy::remove` to a no-op.
        /// Only one test noticed — and the test *named*
        /// `it_should_modify_amd_remove_existing_access_when_resource_found`
        /// was not it: despite its name it never called `remove`, and was
        /// behaviourally identical to the modify test beside it. It is now
        /// named for what it does, and this covers the branch its name had
        /// been claiming.
        #[test]
        fn it_should_remove_a_resource_entirely_when_no_access_is_named() {
            let uuid = Uuid::new_v4().to_string();
            let mut basic = Policy::default();
            basic.add(uuid.clone(), String::from("get"));
            basic.add(uuid.clone(), String::from("put"));
            assert_eq!(basic.resources.len(), 1);

            basic.remove(uuid.clone(), None);

            assert!(
                basic.resources.is_empty(),
                "removing with no access named must drop the resource itself, \
                 not one of its verbs"
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
                format!("{}:{},{}", uuid, access, access_2)
            );
            basic.remove(uuid.clone(), Some(access));
            assert_eq!(basic.resources[0], format!("{}:{}", uuid, access_2));
        }

        #[test]
        fn it_should_modify_existing_access_when_resource_found() {
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

    // #[cfg(test)]
    // mod deny_basic {
    //     use crate::policies::{Policy, PolicyEffect};
    //     use uuid::Uuid;
    //
    //     #[test]
    //     fn it_should_deny_on_deny_effect_and_not_found() {
    //         let uuid = Uuid::new_v4().to_string();
    //         let access = String::from("get");
    //         let policy = Policy::default_deny();
    //         assert_eq!(policy.can_i(uuid, access), false)
    //     }
    //
    //     #[test]
    //     fn it_should_deny_on_effect_deny() {
    //         let uuid = Uuid::new_v4().to_string();
    //         let access = String::from("get");
    //         let mut policy = Policy::default_deny();
    //         policy.add(uuid.clone(), access.clone());
    //         assert_eq!(policy.can_i(uuid, access), false)
    //     }
    // }

    // #[cfg(test)]
    // mod allow_basic {
    //     use crate::policies::{Policy, PolicyEffect};
    //     use uuid::Uuid;
    //
    //     #[test]
    //     fn it_should_allow_on_effect_allow() {
    //         let uuid = Uuid::new_v4().to_string();
    //         let access = String::from("get");
    //         let mut policy = Policy::default_allow();
    //         policy.add(uuid.clone(), access.clone());
    //         assert_eq!(policy.can_i(uuid, access), true)
    //     }
    //
    //     #[test]
    //     fn it_should_deny_on_allow_effect_and_not_found() {
    //         let uuid = Uuid::new_v4().to_string();
    //         let access = String::from("get");
    //         let policy = Policy::default_allow();
    //         assert_eq!(policy.can_i(uuid, access), false)
    //     }
    // }
}
