//! Tests for the interaction seam.
//!
//! Split out of `interaction.rs` only because the two together exceed the
//! repository's 1000-line file limit (`scripts/check-file-length.sh`); they
//! are the same module, attached with `#[path]`.

use super::*;

fn decl(id: &str, subjects: &[SubjectKind], config: Vec<ConfigField>) -> EffectDeclaration {
    EffectDeclaration {
        id: id.to_string(),
        label: id.to_string(),
        description: String::from("does a thing"),
        subject_kinds: subjects.to_vec(),
        config,
    }
}

fn reference_field(key: &str, of: &str, required: bool) -> ConfigField {
    ConfigField {
        key: key.to_string(),
        label: key.to_string(),
        kind: ConfigFieldKind::Reference { of: of.to_string() },
        required,
    }
}

// ---------------------------------------------------------------------------
// Registry assembly
// ---------------------------------------------------------------------------

#[test]
fn a_duplicate_id_fails_at_assembly_not_at_first_use() {
    // Two contributors, each unaware of the other, claiming one name.
    let first = vec![decl("thing.do", &[SubjectKind::Prop], vec![])];
    let second = vec![decl("thing.do", &[SubjectKind::Door], vec![])];

    let result = EffectRegistry::assemble([first, second]);

    // The point is *when* this fails. Assembly is startup. If the collision
    // instead surfaced when a Game Master happened to author one of the two,
    // it would be found mid-session by the people least able to fix it.
    assert_eq!(
        result,
        Err(RegistryError::DuplicateId {
            id: String::from("thing.do")
        })
    );
}

#[test]
fn an_unnamespaced_id_is_refused_because_it_defeats_collision_detection() {
    let result = EffectRegistry::assemble([vec![decl("toggle", &[SubjectKind::Prop], vec![])]]);
    assert_eq!(
        result,
        Err(RegistryError::UnnamespacedId {
            id: String::from("toggle")
        })
    );
}

#[test]
fn an_empty_contribution_set_assembles_into_an_empty_registry() {
    // A build with no subsystems must offer nothing, not fail (FR-039). This
    // is the state the seam is in before its first contributor exists, and a
    // seam that cannot be in it is not a seam.
    let registry = EffectRegistry::assemble(Vec::<Vec<EffectDeclaration>>::new())
        .expect("no contributors is a legitimate build");
    assert!(registry.is_empty());
    assert_eq!(registry.all().count(), 0);
    assert!(!registry.contains("anything.at_all"));
}

#[test]
fn contributions_from_several_subsystems_are_the_union_of_what_is_compiled_in() {
    let registry = EffectRegistry::assemble([
        vec![decl("alpha.one", &[SubjectKind::Prop], vec![])],
        vec![
            decl("beta.one", &[SubjectKind::Door], vec![]),
            decl("beta.two", &[SubjectKind::Prop, SubjectKind::Region], vec![]),
        ],
    ])
    .expect("distinct ids");

    assert_eq!(registry.len(), 3);
    // id order, so the authoring form does not reshuffle between builds.
    let ids: Vec<&str> = registry.all().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, vec!["alpha.one", "beta.one", "beta.two"]);

    let for_door: Vec<&str> = registry
        .for_subject(SubjectKind::Door)
        .map(|d| d.id.as_str())
        .collect();
    assert_eq!(for_door, vec!["beta.one"]);
}

#[test]
fn a_declaration_knows_its_own_namespace() {
    let d = decl("door.set_state", &[SubjectKind::Door], vec![]);
    assert_eq!(d.namespace(), "door");
}

// ---------------------------------------------------------------------------
// Authoring validation
// ---------------------------------------------------------------------------

fn prop_draft() -> InteractiveDraft {
    InteractiveDraft {
        subject_kind: SubjectKind::Prop,
        subject_ref: Some(String::from("token-1")),
        geometry: None,
        effect_id: None,
        effect_config: serde_json::Value::Null,
        trigger: Trigger::Click,
        activation: Activation::Anyone,
        fire_mode: FireMode::Always,
    }
}

fn region_draft() -> InteractiveDraft {
    InteractiveDraft {
        subject_kind: SubjectKind::Region,
        subject_ref: None,
        geometry: Some(RegionGeometry::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        effect_id: None,
        effect_config: serde_json::Value::Null,
        trigger: Trigger::Enter,
        activation: Activation::Anyone,
        fire_mode: FireMode::Always,
    }
}

#[test]
fn scenery_with_no_effect_is_valid() {
    // An interactive with no effect is legitimate — a GM placing a table has
    // not misconfigured anything (US1 scenario 3).
    let registry = EffectRegistry::default();
    assert_eq!(validate_draft(&prop_draft(), &registry), Ok(()));
    assert_eq!(validate_draft(&region_draft(), &registry), Ok(()));
}

#[test]
fn a_region_carrying_a_subject_reference_is_rejected_rather_than_tolerated() {
    let registry = EffectRegistry::default();
    let mut draft = region_draft();
    draft.subject_ref = Some(String::from("token-1"));

    let errors = validate_draft(&draft, &registry).expect_err("a region has no subject");
    assert!(errors.contains(&AuthoringError::SubjectShape {
        expected: SubjectKind::Region
    }));
}

#[test]
fn a_door_carrying_no_subject_reference_is_rejected() {
    let registry = EffectRegistry::default();
    let mut draft = prop_draft();
    draft.subject_kind = SubjectKind::Door;
    draft.subject_ref = None;

    let errors = validate_draft(&draft, &registry).expect_err("a door is a wall");
    assert!(errors.contains(&AuthoringError::SubjectShape {
        expected: SubjectKind::Door
    }));
    // And it is missing nothing else — geometry is correctly absent.
    assert!(!errors.iter().any(|e| matches!(
        e,
        AuthoringError::GeometryShape { .. }
    )));
}

#[test]
fn a_prop_carrying_geometry_is_rejected() {
    let registry = EffectRegistry::default();
    let mut draft = prop_draft();
    draft.geometry = Some(RegionGeometry::Rect {
        x: 0.0,
        y: 0.0,
        width: 4.0,
        height: 4.0,
    });

    let errors = validate_draft(&draft, &registry).expect_err("a book is not an area");
    assert!(errors.contains(&AuthoringError::GeometryShape {
        expected: SubjectKind::Prop
    }));
}

#[test]
fn a_region_enclosing_no_area_is_rejected() {
    let registry = EffectRegistry::default();
    let mut draft = region_draft();
    draft.geometry = Some(RegionGeometry::Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 10.0,
    });

    let errors = validate_draft(&draft, &registry).expect_err("zero width encloses nothing");
    assert!(errors.contains(&AuthoringError::DegenerateGeometry));
}

#[test]
fn only_a_region_may_trigger_on_entry() {
    let registry = EffectRegistry::default();
    let mut draft = prop_draft();
    draft.trigger = Trigger::Enter;

    let errors = validate_draft(&draft, &registry).expect_err("a book cannot be crossed");
    assert!(errors.contains(&AuthoringError::EnterNeedsRegion {
        subject_kind: SubjectKind::Prop
    }));
}

#[test]
fn an_effect_no_contributor_declares_is_refused_at_authoring_time() {
    let registry = EffectRegistry::default();
    let mut draft = prop_draft();
    draft.effect_id = Some(String::from("audio.play"));

    let errors = validate_draft(&draft, &registry).expect_err("no audio subsystem exists");
    assert!(errors.contains(&AuthoringError::UnknownEffect {
        id: String::from("audio.play")
    }));
}

#[test]
fn an_effect_is_refused_on_a_subject_it_does_not_attach_to() {
    let registry = EffectRegistry::assemble([vec![decl(
        "thing.door_only",
        &[SubjectKind::Door],
        vec![],
    )]])
    .expect("one declaration");

    let mut draft = prop_draft();
    draft.effect_id = Some(String::from("thing.door_only"));

    let errors = validate_draft(&draft, &registry).expect_err("prop is not a door");
    assert!(errors.contains(&AuthoringError::WrongSubjectForEffect {
        id: String::from("thing.door_only"),
        subject_kind: SubjectKind::Prop
    }));
}

#[test]
fn every_problem_is_reported_at_once_rather_than_one_per_attempt() {
    let registry = EffectRegistry::default();
    let mut draft = region_draft();
    draft.subject_ref = Some(String::from("token-1"));
    draft.geometry = None;
    draft.effect_id = Some(String::from("nothing.here"));

    let errors = validate_draft(&draft, &registry).expect_err("three separate problems");
    // A Game Master filling in a form deserves the whole list.
    assert!(errors.len() >= 3, "expected several, got {errors:?}");
}

// ---------------------------------------------------------------------------
// Config validation
// ---------------------------------------------------------------------------

#[test]
fn a_required_field_left_empty_is_refused() {
    let declaration = decl(
        "thing.do",
        &[SubjectKind::Prop],
        vec![reference_field("target", "wall", true)],
    );
    let errors = validate_config(&declaration, &serde_json::json!({}));
    assert_eq!(
        errors,
        vec![AuthoringError::MissingConfigField {
            key: String::from("target")
        }]
    );
}

#[test]
fn an_optional_field_left_empty_is_fine() {
    let declaration = decl(
        "thing.do",
        &[SubjectKind::Prop],
        vec![reference_field("target", "wall", false)],
    );
    assert!(validate_config(&declaration, &serde_json::json!({})).is_empty());
    assert!(validate_config(&declaration, &serde_json::Value::Null).is_empty());
}

#[test]
fn a_field_nothing_declared_is_refused_rather_than_stored_and_ignored() {
    // Storing it would be the silent-drift failure this whole design exists to
    // avoid: a GM configures something, it is kept, and nothing ever reads it.
    let declaration = decl("thing.do", &[SubjectKind::Prop], vec![]);
    let errors = validate_config(&declaration, &serde_json::json!({ "colour": "red" }));
    assert_eq!(
        errors,
        vec![AuthoringError::UnknownConfigField {
            key: String::from("colour")
        }]
    );
}

#[test]
fn a_choice_only_accepts_what_it_declared() {
    let declaration = decl(
        "thing.do",
        &[SubjectKind::Prop],
        vec![ConfigField {
            key: String::from("state"),
            label: String::from("State"),
            kind: ConfigFieldKind::Choice {
                options: vec![
                    ChoiceOption {
                        value: String::from("open"),
                        label: String::from("Open"),
                    },
                    ChoiceOption {
                        value: String::from("closed"),
                        label: String::from("Closed"),
                    },
                ],
            },
            required: true,
        }],
    );

    assert!(validate_config(&declaration, &serde_json::json!({ "state": "open" })).is_empty());
    assert_eq!(
        validate_config(&declaration, &serde_json::json!({ "state": "ajar" })),
        vec![AuthoringError::InvalidConfigField {
            key: String::from("state")
        }]
    );
    assert_eq!(
        validate_config(&declaration, &serde_json::json!({ "state": 3 })),
        vec![AuthoringError::InvalidConfigField {
            key: String::from("state")
        }]
    );
}

#[test]
fn a_reference_list_accepts_several_and_refuses_a_non_reference_among_them() {
    let declaration = decl(
        "thing.do",
        &[SubjectKind::Prop],
        vec![ConfigField {
            key: String::from("lights"),
            label: String::from("Lights"),
            kind: ConfigFieldKind::ReferenceList {
                of: String::from("light"),
            },
            required: true,
        }],
    );

    assert!(
        validate_config(&declaration, &serde_json::json!({ "lights": ["a", "b"] })).is_empty()
    );
    assert_eq!(
        validate_config(&declaration, &serde_json::json!({ "lights": ["a", 7] })),
        vec![AuthoringError::InvalidConfigField {
            key: String::from("lights")
        }]
    );
}

// ---------------------------------------------------------------------------
// Activation — the truth table
// ---------------------------------------------------------------------------

fn context() -> ActivationContext {
    ActivationContext {
        actor_is_gm: false,
        has_effect: true,
        effect_available: true,
        subject_locked: false,
        activation: Activation::Anyone,
        fire_mode: FireMode::Always,
        has_fired: false,
    }
}

#[test]
fn an_ordinary_activation_performs() {
    assert_eq!(resolve_activation(context()), ActivationOutcome::Performed);
}

#[test]
fn no_effect_reads_as_scenery_rather_than_as_anything_going_wrong() {
    let mut c = context();
    c.has_effect = false;
    // Even with everything else that could refuse it also true — there is
    // nothing to refuse.
    c.subject_locked = true;
    c.activation = Activation::GmOnly;
    assert_eq!(resolve_activation(c), ActivationOutcome::NoEffect);
}

#[test]
fn an_absent_subsystem_is_unavailable_rather_than_refused_or_broken() {
    let mut c = context();
    c.effect_available = false;
    assert_eq!(resolve_activation(c), ActivationOutcome::Unavailable);
}

#[test]
fn a_player_is_refused_a_gm_only_interactive_and_the_gm_is_not() {
    let mut c = context();
    c.activation = Activation::GmOnly;
    assert_eq!(
        resolve_activation(c),
        ActivationOutcome::Refused {
            reason: RefusalReason::GmOnly
        }
    );

    c.actor_is_gm = true;
    assert_eq!(resolve_activation(c), ActivationOutcome::Performed);
}

#[test]
fn a_locked_subject_refuses_a_player_and_accepts_the_gm() {
    // FR-013. The GM locked it; the GM can still open it.
    let mut c = context();
    c.subject_locked = true;
    assert_eq!(
        resolve_activation(c),
        ActivationOutcome::Refused {
            reason: RefusalReason::Locked
        }
    );

    c.actor_is_gm = true;
    assert_eq!(resolve_activation(c), ActivationOutcome::Performed);
}

#[test]
fn a_locked_subject_does_not_queue_a_request() {
    // Queueing would put a decision in front of the Game Master that their own
    // lock has already made.
    let mut c = context();
    c.subject_locked = true;
    c.activation = Activation::RequiresApproval;
    assert_eq!(
        resolve_activation(c),
        ActivationOutcome::Refused {
            reason: RefusalReason::Locked
        }
    );
}

#[test]
fn a_once_interactive_that_has_fired_is_refused_for_everybody_including_the_gm() {
    let mut c = context();
    c.fire_mode = FireMode::Once;
    c.has_fired = true;
    assert_eq!(
        resolve_activation(c),
        ActivationOutcome::Refused {
            reason: RefusalReason::AlreadyFired
        }
    );

    // The GM's route back is `resetInteractive`, not a privileged re-fire —
    // otherwise "once" would mean "once, unless".
    c.actor_is_gm = true;
    assert_eq!(
        resolve_activation(c),
        ActivationOutcome::Refused {
            reason: RefusalReason::AlreadyFired
        }
    );
}

#[test]
fn a_once_interactive_that_has_not_fired_performs() {
    let mut c = context();
    c.fire_mode = FireMode::Once;
    assert_eq!(resolve_activation(c), ActivationOutcome::Performed);
}

#[test]
fn approval_is_requested_for_a_player_and_skipped_for_the_gm() {
    let mut c = context();
    c.activation = Activation::RequiresApproval;
    assert_eq!(resolve_activation(c), ActivationOutcome::Requested);

    // A GM's own activation does not queue — they are the person the queue
    // exists to ask.
    c.actor_is_gm = true;
    assert_eq!(resolve_activation(c), ActivationOutcome::Performed);
}

#[test]
fn permission_is_reported_before_fire_state() {
    // Both are true. Telling a player they were never allowed is more useful
    // than telling them they are too late for something they could not do.
    let mut c = context();
    c.activation = Activation::GmOnly;
    c.fire_mode = FireMode::Once;
    c.has_fired = true;
    assert_eq!(
        resolve_activation(c),
        ActivationOutcome::Refused {
            reason: RefusalReason::GmOnly
        }
    );
}

#[test]
fn every_combination_resolves_to_exactly_one_outcome() {
    // The exhaustive sweep. Not checking *which* outcome — the tests above do
    // that — but that the table is total: no combination panics, and none
    // falls through to a default that happens to be permissive.
    let mut performed = 0;
    for actor_is_gm in [false, true] {
        for has_effect in [false, true] {
            for effect_available in [false, true] {
                for subject_locked in [false, true] {
                    for activation in [
                        Activation::Anyone,
                        Activation::GmOnly,
                        Activation::RequiresApproval,
                    ] {
                        for fire_mode in [FireMode::Always, FireMode::Once] {
                            for has_fired in [false, true] {
                                let outcome = resolve_activation(ActivationContext {
                                    actor_is_gm,
                                    has_effect,
                                    effect_available,
                                    subject_locked,
                                    activation,
                                    fire_mode,
                                    has_fired,
                                });
                                if outcome == ActivationOutcome::Performed {
                                    performed += 1;
                                    // Nothing performs that a rule forbids.
                                    assert!(has_effect && effect_available);
                                    assert!(!subject_locked || actor_is_gm);
                                    assert!(activation != Activation::GmOnly || actor_is_gm);
                                    assert!(
                                        activation != Activation::RequiresApproval || actor_is_gm
                                    );
                                    assert!(fire_mode != FireMode::Once || !has_fired);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(performed > 0, "the table cannot refuse everything");
}
