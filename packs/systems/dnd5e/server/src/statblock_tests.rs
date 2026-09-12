use super::*;

fn line(text: &str, size: f64, bold: bool) -> SourceLine {
    SourceLine {
        text: text.to_string(),
        size,
        bold,
        page: 98,
    }
}

/// The red dragon from the Monster Manual, as this repository's own PDF
/// reader actually produces it — spacing losses and all. Not the SRD's
/// idealised layout, which no book ships.
fn red_dragon() -> Vec<SourceLine> {
    vec![
        line("RED DRAGON", 11.6, true),
        line("Gargantuan dragon, chaotic evil", 8.7, false),
        line("Armor Class 22 (natural armor)", 8.5, false),
        line("Hit Points 546 (28d20 + 252)", 8.5, false),
        line("Speed 40ft., climb 40ft., fly 80ft.", 8.5, false),
        line("Challenge 24 (36,500 XP)", 8.5, false),
        line("ACTIONS", 8.4, true),
        line(
            "Bite. Melee Weapon Attack: +17 to hit, reach 15ft., one target.",
            8.7,
            false,
        ),
        line(
            "Claw. Melee Weapon Attack: +17 to hit, reach 10ft.",
            8.7,
            false,
        ),
    ]
}

#[test]
fn a_real_statblock_reads_end_to_end() {
    let found = statblocks(&red_dragon());
    assert_eq!(found.len(), 1, "{found:#?}");
    let dragon = &found[0];
    assert_eq!(dragon.name, "RED DRAGON");
    assert_eq!(dragon.size_category.as_deref(), Some("gargantuan"));
    assert_eq!(dragon.armor_class, Some(22));
    assert_eq!(dragon.hit_points, Some(546));
    assert_eq!(dragon.hit_dice.as_deref(), Some("28d20 + 252"));
    assert_eq!(dragon.challenge.as_deref(), Some("24 (36,500 XP)"));
    assert_eq!(dragon.page, 98);
}

#[test]
fn reach_is_read_per_attack_and_not_from_the_creatures_size() {
    // Spec 047's finding, held here: a tarrasque's bite reaches 10 feet and
    // its tail 20, and an ogre is Large with a 5-foot greatclub. A reader
    // that derived reach from size would give this dragon one number.
    let dragon = &statblocks(&red_dragon())[0];
    let reaches: Vec<Option<f64>> = dragon.actions.iter().map(|a| a.reach_feet).collect();
    assert_eq!(reaches, vec![Some(15.0), Some(10.0)]);
}

#[test]
fn the_spacing_a_real_book_loses_does_not_defeat_the_reader() {
    // "reach 15ft." with the space gone is what the Monster Manual's own
    // text yields. A reader expecting "15 ft." finds nothing in that book.
    assert_eq!(reach_of("reach 15ft., one target"), Some(15.0));
    assert_eq!(reach_of("reach 15 ft., one target"), Some(15.0));
    assert_eq!(reach_of("reach 15 feet"), Some(15.0));
}

#[test]
fn the_word_reach_in_prose_is_not_a_measurement() {
    // "creatures within reach 3 of its allies" is not an attack. Requiring a
    // unit after the number is what separates them.
    assert_eq!(reach_of("can reach 3 creatures at once"), None);
    assert_eq!(reach_of("the dragon's attacks reach far"), None);
}

#[test]
fn two_creatures_on_one_page_are_two_statblocks() {
    let mut lines = red_dragon();
    lines.push(line("GOBLIN", 11.6, true));
    lines.push(line("Small humanoid (goblinoid), neutral evil", 8.7, false));
    lines.push(line("Armor Class 15 (leather armor, shield)", 8.5, false));
    lines.push(line("Hit Points 7 (2d6)", 8.5, false));

    let found = statblocks(&lines);
    assert_eq!(
        found.len(),
        2,
        "{:#?}",
        found.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    assert_eq!(found[1].name, "GOBLIN");
    assert_eq!(found[1].armor_class, Some(15));
    assert_eq!(found[1].size_category.as_deref(), Some("small"));
    assert_eq!(
        found[0].actions.len(),
        2,
        "the dragon's actions must not leak into the goblin"
    );
}

#[test]
fn prose_that_is_not_a_statblock_yields_nothing() {
    let lines = vec![
        line("RED DRAGON", 13.4, true),
        line(
            "The most covetous of the true dragons, red dragons",
            9.2,
            false,
        ),
        line(
            "tirelessly seek to increase their treasure hoards.",
            9.2,
            false,
        ),
    ];
    assert!(
        statblocks(&lines).is_empty(),
        "a creature's description is not a creature"
    );
}

#[test]
fn a_block_with_no_armour_class_is_not_a_statblock() {
    // The anchor is load-bearing: without it a sidebar with a bold heading
    // and a size word would be read as a monster.
    let lines = vec![
        line("LARGE CREATURES", 12.0, true),
        line(
            "Large creatures take up more space on the board.",
            9.0,
            false,
        ),
    ];
    assert!(statblocks(&lines).is_empty());
}

#[test]
fn a_size_word_alone_is_not_a_descriptor() {
    // A table of size categories has "Large" on a line by itself.
    assert_eq!(descriptor_of("Large"), None);
    assert!(descriptor_of("Large beast, unaligned").is_some());
}

#[test]
fn the_labels_books_actually_use_are_all_recognised() {
    // Case and punctuation vary between publishers; the value does not.
    for text in [
        "Armor Class 15",
        "ARMOR CLASS 15",
        "Armor Class: 15",
        "Armour Class 15",
        // A book that emboldens its labels as a run of their own leaves the
        // styling full stop behind: "Armor Class. 15".
        "Armor Class. 15",
        "ARMOR CLASS — 15",
    ] {
        assert_eq!(
            armor_class_of(&line(text, 9.0, false)),
            Some(15),
            "{text} should read as 15"
        );
    }
}

#[test]
fn hit_points_keep_their_dice_and_a_block_without_dice_is_still_read() {
    assert_eq!(
        hit_points_of("Hit Points 546 (28d20 + 252)"),
        Some((546, Some("28d20 + 252".to_string())))
    );
    assert_eq!(hit_points_of("Hit Points 7"), Some((7, None)));
    // Brackets that are not dice are not dice.
    assert_eq!(hit_points_of("Hit Points 30 (see below)"), Some((30, None)));
}

#[test]
fn a_block_whose_name_was_never_found_is_kept_and_flagged() {
    // The defect this exists for: with no name line above it, the reader
    // presented "Medium humanoid (aarakocra), neutral good" as a monster'''s
    // name, with full confidence. The creature is still worth keeping — it
    // has an armour class and hit points — but a person has to look at it.
    let lines = vec![
        line("Medium humanoid (aarakocra), neutral good", 8.7, false),
        line("Armor Class 12", 8.5, false),
        line("Hit Points 13 (3d8)", 8.5, false),
    ];
    let found = statblocks(&lines);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].armor_class, Some(12));
    assert_eq!(
        found[0].confidence.name,
        ReadDefault::Uncertain,
        "a descriptor standing in for a name must never read as certain"
    );
}

#[test]
fn a_damaged_name_is_reported_uncertain_rather_than_trusted() {
    // Spec 048 FR-003: a value a person should look at is worth more than a
    // value quietly presented as fact.
    let mut lines = red_dragon();
    lines[0] = line("R E D D R A G O N O F T H E", 11.6, true);
    let found = statblocks(&lines);
    assert_eq!(found[0].confidence.name, ReadDefault::Uncertain);
    assert_eq!(
        found[0].confidence.armor_class,
        ReadDefault::Clear,
        "an undamaged value beside a damaged one is still clear"
    );
}

#[test]
fn a_creature_from_a_book_with_no_usable_font_is_refused() {
    // Some books embed a subsetted font with no encoding map, and their text
    // decodes into something shaped like words: "Gold dragons have the most
    // love of fey" arrives as "* ROGGUDJRQVKDYHWKHP RVWORYHRIIH". A block
    // read from one would import a creature called that, with a perfectly
    // valid armour class beside it.
    let lines = vec![
        line(
            "* ROGGUDJRQVKDYHWKHP RVWORYHRIIHADP RQJDOOGUDJRQNLQG",
            11.6,
            true,
        ),
        line("Armor Class 18", 8.5, false),
        line("Hit Points 100 (10d10)", 8.5, false),
    ];
    assert!(
        statblocks(&lines).is_empty(),
        "there is nothing here for a person to correct"
    );
}

#[test]
fn an_action_needs_a_name_rather_than_merely_a_full_stop() {
    // Prose continuing from the line above contains full stops and is not an
    // action. Without this every sentence in a statblock becomes one.
    assert!(action_of("much damage on a successful one.").is_none());
    assert!(action_of("Bite. Melee Weapon Attack: +17 to hit.").is_some());
}
