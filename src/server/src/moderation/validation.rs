//! Spec 015 (FR-003, FR-006): statutory-element validation for takedown
//! notices and counter-notices. Pure functions, no DB access — checks
//! required-field presence per 17 U.S.C. § 512(c)(3)/(g)(3), not legal
//! advice generation (research.md R3).

/// Names a missing statutory element, returned to the submitter so they
/// know exactly what to fix (FR-003's "MUST inform the submitter when
/// required elements are missing").
pub type MissingElement = String;

pub struct TakedownNoticeFields<'a> {
    pub claimant_name: &'a str,
    pub claimant_contact: &'a str,
    pub copyrighted_work_description: &'a str,
    pub infringing_material_location: &'a str,
    pub good_faith_statement: bool,
    pub accuracy_statement: bool,
    pub signature: &'a str,
}

/// Validates a takedown notice against the statutory required elements
/// (data-model.md's validation rules). Returns the list of missing
/// elements — empty means valid.
pub fn validate_takedown_notice(fields: &TakedownNoticeFields) -> Vec<MissingElement> {
    let mut missing = Vec::new();
    if fields.claimant_name.trim().is_empty() {
        missing.push("claimantName".to_string());
    }
    if fields.claimant_contact.trim().is_empty() {
        missing.push("claimantContact".to_string());
    }
    if fields.copyrighted_work_description.trim().is_empty() {
        missing.push("copyrightedWorkDescription".to_string());
    }
    if fields.infringing_material_location.trim().is_empty() {
        missing.push("infringingMaterialLocation".to_string());
    }
    if !fields.good_faith_statement {
        missing.push("goodFaithStatement".to_string());
    }
    if !fields.accuracy_statement {
        missing.push("accuracyStatement".to_string());
    }
    if fields.signature.trim().is_empty() {
        missing.push("signature".to_string());
    }
    missing
}

pub struct CounterNoticeFields<'a> {
    pub removed_material_description: &'a str,
    pub good_faith_mistake_statement: bool,
    pub consent_to_jurisdiction: bool,
    pub contact_information: &'a str,
    pub signature: &'a str,
}

/// Validates a counter-notice against the statutory required elements
/// (17 U.S.C. § 512(g)(3)).
pub fn validate_counter_notice(fields: &CounterNoticeFields) -> Vec<MissingElement> {
    let mut missing = Vec::new();
    if fields.removed_material_description.trim().is_empty() {
        missing.push("removedMaterialDescription".to_string());
    }
    if !fields.good_faith_mistake_statement {
        missing.push("goodFaithMistakeStatement".to_string());
    }
    if !fields.consent_to_jurisdiction {
        missing.push("consentToJurisdiction".to_string());
    }
    if fields.contact_information.trim().is_empty() {
        missing.push("contactInformation".to_string());
    }
    if fields.signature.trim().is_empty() {
        missing.push("signature".to_string());
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_notice() -> TakedownNoticeFields<'static> {
        TakedownNoticeFields {
            claimant_name: "Acme Corp",
            claimant_contact: "legal@acme.example",
            copyrighted_work_description: "Acme Sourcebook Vol. 1",
            infringing_material_location: "world-actor-123",
            good_faith_statement: true,
            accuracy_statement: true,
            signature: "Jane Claimant",
        }
    }

    #[test]
    fn a_fully_populated_notice_has_no_missing_elements() {
        assert!(validate_takedown_notice(&valid_notice()).is_empty());
    }

    #[test]
    fn missing_accuracy_statement_is_reported() {
        let mut fields = valid_notice();
        fields.accuracy_statement = false;
        let missing = validate_takedown_notice(&fields);
        assert_eq!(missing, vec!["accuracyStatement".to_string()]);
    }

    #[test]
    fn multiple_missing_elements_are_all_reported() {
        let fields = TakedownNoticeFields {
            claimant_name: "",
            claimant_contact: "",
            copyrighted_work_description: "Something",
            infringing_material_location: "world-actor-123",
            good_faith_statement: false,
            accuracy_statement: true,
            signature: "Signed",
        };
        let missing = validate_takedown_notice(&fields);
        assert_eq!(
            missing,
            vec![
                "claimantName".to_string(),
                "claimantContact".to_string(),
                "goodFaithStatement".to_string(),
            ]
        );
    }

    fn valid_counter_notice() -> CounterNoticeFields<'static> {
        CounterNoticeFields {
            removed_material_description: "My homebrew NPC, entirely SRD-derived",
            good_faith_mistake_statement: true,
            consent_to_jurisdiction: true,
            contact_information: "gm@example.com",
            signature: "GM Name",
        }
    }

    #[test]
    fn a_fully_populated_counter_notice_has_no_missing_elements() {
        assert!(validate_counter_notice(&valid_counter_notice()).is_empty());
    }

    #[test]
    fn missing_consent_to_jurisdiction_is_reported() {
        let mut fields = valid_counter_notice();
        fields.consent_to_jurisdiction = false;
        assert_eq!(
            validate_counter_notice(&fields),
            vec!["consentToJurisdiction".to_string()]
        );
    }
}
