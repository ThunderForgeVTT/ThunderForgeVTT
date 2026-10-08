//! Spec 083 FR-015: a busy table never leaves the board behind.

use super::*;

#[test]
fn a_full_queue_skips_the_oldest_waiting_never_the_playing() {
    let mut q = ThrowQueue::default();
    assert_eq!(q.push("A"), None);
    for t in ["B", "C", "D", "E"] {
        assert_eq!(q.push(t), None);
    }
    assert_eq!(q.push("F"), Some("B"));
    assert_eq!(q.playing, Some("A"));
    assert_eq!(q.waiting, ["C", "D", "E", "F"]);
    assert_eq!(q.push("G"), Some("C"));
    assert_eq!(q.playing, Some("A"));
}

#[test]
fn landing_starts_the_next_in_arrival_order() {
    let mut q = ThrowQueue::default();
    for t in ["A", "B", "C"] {
        q.push(t);
    }
    q.landed();
    assert_eq!(q.playing, Some("B"));
    assert_eq!(q.fading, vec!["A"]);
    q.landed();
    assert_eq!(q.playing, Some("C"));
    q.landed();
    assert_eq!(q.playing, None);
    assert_eq!(q.fading, vec!["A", "B", "C"]);
}

#[test]
fn fading_throws_leave_on_their_own() {
    let mut q = ThrowQueue::default();
    q.push("A");
    q.push("B");
    q.landed();
    assert_eq!(q.fade_done(|t| *t == "A"), vec!["A"]);
    assert!(q.fading.is_empty());
    assert_eq!(q.playing, Some("B"));
    q.landed();
    q.fade_done(|_| true);
    assert!(q.is_empty());
}
