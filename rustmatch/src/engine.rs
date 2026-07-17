//! Forward NFA scan with scan-local, reusable scratch storage.

use crate::nfa::{EdgeKind, PatternDatabase, StateId};
use crate::{Error, Match, Utf16Span, Utf16Text};

pub(crate) fn scan(
    database: &PatternDatabase,
    input: &Utf16Text,
    mut sink: impl FnMut(Match),
) -> Result<(), Error> {
    validate_input(input)?;
    let units = input.units();
    let mut scratch = Scratch::new(database);

    for start in 0..units.len() {
        scratch.reset_start();
        extend_epsilon_closure(
            database,
            database.root(),
            &mut scratch.active,
            &mut scratch.active_seen,
            &mut scratch.stack,
        );

        for (position, &symbol) in units.iter().enumerate().skip(start) {
            scratch.reset_next();
            for &state in &scratch.active {
                for edge in database.edges_from(state) {
                    if database.edge_matches(edge.kind, symbol) {
                        extend_epsilon_closure(
                            database,
                            edge.target,
                            &mut scratch.next,
                            &mut scratch.next_seen,
                            &mut scratch.stack,
                        );
                    }
                }
            }

            let end = position
                .checked_add(1)
                .ok_or(Error::InputTooLarge)
                .and_then(position_utf16)?;
            for &state in &scratch.next {
                for &ordinal in database.terminals_at(state) {
                    scratch.best_end[ordinal] = Some(end);
                }
            }
            std::mem::swap(&mut scratch.active, &mut scratch.next);
            std::mem::swap(&mut scratch.active_seen, &mut scratch.next_seen);
            if scratch.active.is_empty() {
                break;
            }
        }

        let start_utf16 = position_utf16(start)?;
        for (ordinal, end) in scratch.best_end.iter().copied().enumerate() {
            if let Some(end_utf16) = end {
                sink(Match::new(
                    database.pattern_id(ordinal),
                    Utf16Span::from_bounds(start_utf16, end_utf16),
                ));
            }
        }
    }
    Ok(())
}

fn validate_input(input: &Utf16Text) -> Result<(), Error> {
    for (index, &unit) in input.units().iter().enumerate() {
        if unit > 0x7f {
            return Err(Error::UnsupportedInput {
                position_utf16: position_utf16(index)?,
                code_unit: unit,
            });
        }
    }
    Ok(())
}

fn position_utf16(index: usize) -> Result<u64, Error> {
    u64::try_from(index).map_err(|_| Error::InputTooLarge)
}

fn extend_epsilon_closure(
    database: &PatternDatabase,
    seed: StateId,
    output: &mut Vec<StateId>,
    visited: &mut [bool],
    stack: &mut Vec<StateId>,
) {
    if visited[seed.index()] {
        return;
    }
    visited[seed.index()] = true;
    stack.push(seed);
    while let Some(state) = stack.pop() {
        output.push(state);
        for edge in database.edges_from(state) {
            if edge.kind == EdgeKind::Epsilon && !visited[edge.target.index()] {
                visited[edge.target.index()] = true;
                stack.push(edge.target);
            }
        }
    }
}

struct Scratch {
    active: Vec<StateId>,
    next: Vec<StateId>,
    stack: Vec<StateId>,
    active_seen: Vec<bool>,
    next_seen: Vec<bool>,
    best_end: Vec<Option<u64>>,
}

impl Scratch {
    fn new(database: &PatternDatabase) -> Self {
        let state_count = database.state_count();
        Self {
            active: Vec::with_capacity(state_count),
            next: Vec::with_capacity(state_count),
            stack: Vec::with_capacity(state_count),
            active_seen: vec![false; state_count],
            next_seen: vec![false; state_count],
            best_end: vec![None; database.pattern_count()],
        }
    }

    fn reset_start(&mut self) {
        self.active.clear();
        self.active_seen.fill(false);
        self.best_end.fill(None);
        self.stack.clear();
    }

    fn reset_next(&mut self) {
        self.next.clear();
        self.next_seen.fill(false);
        self.stack.clear();
    }
}
