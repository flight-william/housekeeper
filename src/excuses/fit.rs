//! Which entry excuses which finding, when a finding fits more than one.

use super::{Date, Excuse};
use crate::finding::{Finding, Place};
use std::cmp::Reverse;
use std::collections::HashMap;

/// The live entry each finding is given to, if any, by finding index. Each finding's
/// entries are tried most specific first. When all are full, a finding an entry already
/// holds moves to another entry that fits it, if that frees a place (an augmenting
/// path), so as many findings as possible are excused and the order of the list never
/// decides which: an earlier, wider entry cannot use up a later entry's findings.
pub(super) fn fit(findings: &[Finding], excuses: &[Excuse], today: Date) -> Vec<Option<usize>> {
    let mut by_place: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
    for (e, x) in excuses.iter().enumerate() {
        by_place
            .entry((x.rule.as_str(), x.file.as_str()))
            .or_default()
            .push(e);
    }
    let fits = findings
        .iter()
        .map(|f| {
            let same = match &f.place {
                Place::File(file) => by_place.get(&(f.rule.as_str(), file.as_str())),
                Place::Repo => None,
            };
            let mut fits: Vec<usize> = same
                .into_iter()
                .flatten()
                .copied()
                .filter(|&e| excuses[e].deadline >= today && excuses[e].matches(f))
                .collect();
            fits.sort_by_key(|&e| Reverse(excuses[e].specificity()));
            fits
        })
        .collect();
    let mut fit = Fit {
        fits,
        room: excuses.iter().map(|e| e.count as usize).collect(),
        given: vec![None; findings.len()],
        taken: vec![Vec::new(); excuses.len()],
        seen: vec![0; excuses.len()],
        round: 0,
    };
    for i in 0..findings.len() {
        fit.round += 1;
        fit.reach(i);
    }
    fit.given
}

struct Fit {
    /// The entries each finding fits, most specific first.
    fits: Vec<Vec<usize>>,
    /// How many findings each entry may hold (its count).
    room: Vec<usize>,
    /// The entry each finding holds a place in.
    given: Vec<Option<usize>>,
    /// The findings each entry holds.
    taken: Vec<Vec<usize>>,
    /// The round in which each entry was last tried, so a round tries it once.
    seen: Vec<usize>,
    round: usize,
}

impl Fit {
    /// Find finding `i` a place: a free one first, else by moving a holder on.
    fn reach(&mut self, i: usize) -> bool {
        for k in 0..self.fits[i].len() {
            let e = self.fits[i][k];
            if self.seen[e] != self.round && self.taken[e].len() < self.room[e] {
                self.seen[e] = self.round;
                self.take(i, e);
                return true;
            }
        }
        for k in 0..self.fits[i].len() {
            let e = self.fits[i][k];
            if self.seen[e] == self.round {
                continue;
            }
            self.seen[e] = self.round;
            for h in 0..self.taken[e].len() {
                if self.reach(self.taken[e][h]) {
                    self.taken[e].swap_remove(h);
                    self.take(i, e);
                    return true;
                }
            }
        }
        false
    }

    fn take(&mut self, i: usize, e: usize) {
        self.given[i] = Some(e);
        self.taken[e].push(i);
    }
}
