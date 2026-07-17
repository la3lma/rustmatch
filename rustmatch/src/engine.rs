//! Forward NFA scan with scan-local, reusable scratch storage.

use crate::hir::Assertion;
use crate::nfa::{EdgeKind, PatternDatabase, StateId};
use crate::{Error, Match, Utf16Span, Utf16Text};

pub(crate) fn scan(
    database: &PatternDatabase,
    input: &Utf16Text,
    sink: impl FnMut(Match),
) -> Result<(), Error> {
    if database.uses_assertions() {
        scan_with_assertions(database, input, sink)
    } else {
        scan_without_assertions(database, input, sink)
    }
}

// Keep this loop structurally identical to the pre-assertion engine. Context
// support is pay-for-use and must not enlarge the ordinary transition path.
fn scan_without_assertions(
    database: &PatternDatabase,
    input: &Utf16Text,
    mut sink: impl FnMut(Match),
) -> Result<(), Error> {
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

fn scan_with_assertions(
    database: &PatternDatabase,
    input: &Utf16Text,
    mut sink: impl FnMut(Match),
) -> Result<(), Error> {
    let units = input.units();
    let mut scratch = Scratch::new(database);

    for start in 0..units.len() {
        scratch.reset_start();
        extend_assertion_closure(
            database,
            database.root(),
            AssertionPosition::before(units, start),
            &mut scratch.active,
            &mut scratch.active_seen,
            &mut scratch.stack,
        );

        for (position, &symbol) in units.iter().enumerate().skip(start) {
            scratch.reset_next();
            let next_position = position.checked_add(1).ok_or(Error::InputTooLarge)?;
            for &state in &scratch.active {
                for edge in database.edges_from(state) {
                    if database.edge_matches(edge.kind, symbol) {
                        extend_assertion_closure(
                            database,
                            edge.target,
                            AssertionPosition::after(units, next_position),
                            &mut scratch.next,
                            &mut scratch.next_seen,
                            &mut scratch.stack,
                        );
                    }
                }
            }
            let end = position_utf16(next_position)?;
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

fn position_utf16(index: usize) -> Result<u64, Error> {
    u64::try_from(index).map_err(|_| Error::InputTooLarge)
}

#[derive(Clone, Copy)]
struct AssertionPosition<'a> {
    input: &'a [u16],
    index: usize,
    after_current: bool,
}

impl<'a> AssertionPosition<'a> {
    const fn before(input: &'a [u16], index: usize) -> Self {
        Self {
            input,
            index,
            after_current: false,
        }
    }

    const fn after(input: &'a [u16], index: usize) -> Self {
        Self {
            input,
            index,
            after_current: true,
        }
    }
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

fn extend_assertion_closure(
    database: &PatternDatabase,
    seed: StateId,
    position: AssertionPosition<'_>,
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
            let follows_without_consuming = match edge.kind {
                EdgeKind::Epsilon => true,
                EdgeKind::Assertion(assertion) => assertion_matches(
                    assertion,
                    position.input,
                    position.index,
                    position.after_current,
                ),
                EdgeKind::Symbol(_) | EdgeKind::Predicate(_) => false,
            };
            if follows_without_consuming && !visited[edge.target.index()] {
                visited[edge.target.index()] = true;
                stack.push(edge.target);
            }
        }
    }
}

fn assertion_matches(
    assertion: Assertion,
    input: &[u16],
    position: usize,
    after_current: bool,
) -> bool {
    match assertion {
        Assertion::LineStart => position == 0 || input.get(position - 1) == Some(&u16::from(b'\n')),
        Assertion::LineEnd => {
            after_current
                && (position == input.len() || input.get(position) == Some(&u16::from(b'\n')))
        }
        Assertion::WordBoundary => is_word_boundary(input, position),
        Assertion::NonWordBoundary => !is_word_boundary(input, position),
    }
}

fn is_word_boundary(input: &[u16], position: usize) -> bool {
    let before = position
        .checked_sub(1)
        .and_then(|index| input.get(index))
        .is_some_and(|&symbol| is_ascii_word(symbol));
    let after = input
        .get(position)
        .is_some_and(|&symbol| is_ascii_word(symbol));
    before != after
}

const fn is_ascii_word(symbol: u16) -> bool {
    matches!(symbol, 0x30..=0x39 | 0x41..=0x5a | 0x5f | 0x61..=0x7a)
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{assertion_matches, extend_epsilon_closure, is_ascii_word, scan};
    use crate::hir::Assertion;
    use crate::hir::Hir;
    use crate::nfa;
    use crate::parser::parse;
    use crate::{PatternId, Utf16Text};

    #[test]
    fn epsilon_closure_reaches_empty_branch_without_duplicate_states() -> Result<(), crate::Error> {
        // Prepare
        let patterns = [parse(PatternId::new(1), "|a")?];
        let database = nfa::compile(&patterns)?;
        let mut states = Vec::new();
        let mut visited = vec![false; database.state_count()];
        let mut stack = Vec::new();

        // Test
        extend_epsilon_closure(
            &database,
            database.root(),
            &mut states,
            &mut visited,
            &mut stack,
        );

        // Assert
        let unique: BTreeSet<_> = states.iter().map(|state| state.index()).collect();
        assert_eq!(states.len(), unique.len());
        assert!(
            states
                .iter()
                .any(|&state| !database.terminals_at(state).is_empty())
        );
        Ok(())
    }

    #[test]
    fn assertion_context_exhausts_ascii_boundaries_and_line_edges() {
        // Prepare
        let symbols = std::iter::once(None)
            .chain((0_u16..=127).map(Some))
            .collect::<Vec<_>>();

        // Test / Assert
        for &left in &symbols {
            for &right in &symbols {
                let (input, position) = match (left, right) {
                    (None, None) => (Vec::new(), 0),
                    (None, Some(right)) => (vec![right], 0),
                    (Some(left), None) => (vec![left], 1),
                    (Some(left), Some(right)) => (vec![left, right], 1),
                };
                let expected = left.is_some_and(is_ascii_word) != right.is_some_and(is_ascii_word);
                assert_eq!(
                    assertion_matches(Assertion::WordBoundary, &input, position, false),
                    expected,
                    "left={left:?}, right={right:?}"
                );
                assert_eq!(
                    assertion_matches(Assertion::NonWordBoundary, &input, position, true),
                    !expected,
                    "left={left:?}, right={right:?}"
                );
            }
        }

        assert!(assertion_matches(Assertion::LineStart, &[0x61], 0, false));
        assert!(assertion_matches(
            Assertion::LineStart,
            &[0x0a, 0x61],
            1,
            true
        ));
        assert!(!assertion_matches(
            Assertion::LineStart,
            &[0x78, 0x61],
            1,
            false
        ));
        assert!(assertion_matches(Assertion::LineEnd, &[0x61], 1, true));
        assert!(assertion_matches(
            Assertion::LineEnd,
            &[0x61, 0x0a],
            1,
            true
        ));
        assert!(!assertion_matches(Assertion::LineEnd, &[0x0a], 0, false));
    }

    #[test]
    fn bounded_generated_composition_agrees_with_tiny_hir_interpreter() -> Result<(), crate::Error>
    {
        // Prepare
        let patterns = generated_patterns();
        let inputs = generated_inputs();

        // Test / Assert
        for (ordinal, source) in patterns.iter().enumerate() {
            let pattern_id = PatternId::new(u32::try_from(ordinal).expect("bounded pattern count"));
            let parsed = parse(pattern_id, source)?;
            let database = nfa::compile(std::slice::from_ref(&parsed))?;
            for input in &inputs {
                let text = Utf16Text::from(input.as_str());
                let mut actual = Vec::new();
                scan(&database, &text, |event| {
                    actual.push((event.span().start(), event.span().end()));
                })?;

                let mut expected = Vec::new();
                for start in 0..text.units().len() {
                    let longest = interpret(parsed.expression(), text.units(), start)
                        .into_iter()
                        .filter(|&end| end > start)
                        .max();
                    if let Some(end) = longest {
                        expected.push((
                            u64::try_from(start).expect("bounded input"),
                            u64::try_from(end).expect("bounded input"),
                        ));
                    }
                }
                assert_eq!(actual, expected, "pattern {source:?}, input {input:?}");
            }
        }
        Ok(())
    }

    fn interpret(expression: &Hir, input: &[u16], start: usize) -> BTreeSet<usize> {
        interpret_from(expression, input, (start, false))
            .into_iter()
            .map(|(position, _)| position)
            .collect()
    }

    fn interpret_from(
        expression: &Hir,
        input: &[u16],
        state: (usize, bool),
    ) -> BTreeSet<(usize, bool)> {
        let (position, after_current) = state;
        match expression {
            Hir::Never => BTreeSet::new(),
            Hir::Epsilon => BTreeSet::from([state]),
            Hir::Assertion(assertion) => {
                if assertion_matches(*assertion, input, position, after_current) {
                    BTreeSet::from([state])
                } else {
                    BTreeSet::new()
                }
            }
            Hir::Symbol(expected) => input
                .get(position)
                .filter(|&&actual| actual == *expected)
                .map_or_else(BTreeSet::new, |_| BTreeSet::from([(position + 1, true)])),
            Hir::Predicate(predicate) => input
                .get(position)
                .filter(|&&actual| predicate.matches(actual))
                .map_or_else(BTreeSet::new, |_| BTreeSet::from([(position + 1, true)])),
            Hir::Sequence(expressions) => {
                let mut states = BTreeSet::from([state]);
                for item in expressions {
                    states = states
                        .into_iter()
                        .flat_map(|state| interpret_from(item, input, state))
                        .collect();
                }
                states
            }
            Hir::Alternation(expressions) => expressions
                .iter()
                .flat_map(|branch| interpret_from(branch, input, state))
                .collect(),
            Hir::Repeat {
                expression,
                min,
                max,
            } => interpret_repetition(expression, *min, *max, input, state),
        }
    }

    fn interpret_repetition(
        expression: &Hir,
        min: u16,
        max: Option<u16>,
        input: &[u16],
        start: (usize, bool),
    ) -> BTreeSet<(usize, bool)> {
        let mut accepted = BTreeSet::new();
        let mut positions = BTreeSet::from([start]);
        if min == 0 {
            accepted.insert(start);
        }
        let limit = max.map_or_else(
            || {
                usize::from(min)
                    .saturating_add(input.len())
                    .saturating_add(1)
            },
            usize::from,
        );
        for count in 1..=limit {
            positions = positions
                .into_iter()
                .flat_map(|state| interpret_from(expression, input, state))
                .collect();
            if positions.is_empty() {
                break;
            }
            if count >= usize::from(min) {
                accepted.extend(&positions);
            }
        }
        accepted
    }

    fn generated_patterns() -> Vec<String> {
        let atoms = ["a", "b", ".", "[ab]"];
        let mut patterns = atoms.iter().map(ToString::to_string).collect::<Vec<_>>();
        for left in atoms {
            for right in atoms {
                patterns.push(format!("{left}{right}"));
                patterns.push(format!("{left}|{right}"));
                patterns.push(format!("x({left}|{right})"));
                patterns.push(format!("x(|{left}){right}"));
                patterns.push(format!("{left}?{right}"));
                patterns.push(format!("{left}+"));
                patterns.push(format!("({left}|{right}){{1,2}}"));
            }
        }
        patterns.extend(["^a", "a$", r"\ba\b", r"\Ba", r"a\B", "(^a|b$)"].map(ToString::to_string));
        patterns
    }

    fn generated_inputs() -> Vec<String> {
        let alphabet = ['a', 'b', '\n'];
        let mut inputs = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..3 {
            frontier = frontier
                .into_iter()
                .flat_map(|prefix| {
                    alphabet.map(move |symbol| {
                        let mut input = prefix.clone();
                        input.push(symbol);
                        input
                    })
                })
                .collect();
            inputs.extend(frontier.iter().cloned());
        }
        inputs
    }
}
