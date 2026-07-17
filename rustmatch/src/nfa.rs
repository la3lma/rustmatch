//! Dense Thompson-style NFA compilation and immutable pattern database.

use std::collections::HashMap;

use crate::hir::{Assertion, Hir, HirPattern};
use crate::predicate::SymbolPredicate;
use crate::{Error, PatternId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StateId(u32);

impl StateId {
    fn for_index(index: usize) -> Result<Self, Error> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| Error::PatternSetTooLarge)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PredicateId(u32);

impl PredicateId {
    fn for_index(index: usize) -> Result<Self, Error> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| Error::PatternSetTooLarge)
    }

    const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EdgeKind {
    Epsilon,
    Assertion(Assertion),
    Symbol(u16),
    Predicate(PredicateId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Edge {
    pub(crate) kind: EdgeKind,
    pub(crate) target: StateId,
}

#[derive(Debug)]
struct State {
    edge_start: usize,
    edge_len: usize,
    terminal_start: usize,
    terminal_len: usize,
}

#[derive(Debug)]
pub(crate) struct PatternDatabase {
    root: StateId,
    states: Box<[State]>,
    edges: Box<[Edge]>,
    predicates: Box<[SymbolPredicate]>,
    terminal_ordinals: Box<[usize]>,
    pattern_ids: Box<[PatternId]>,
    uses_assertions: bool,
}

impl PatternDatabase {
    pub(crate) const fn root(&self) -> StateId {
        self.root
    }

    pub(crate) const fn state_count(&self) -> usize {
        self.states.len()
    }

    pub(crate) const fn pattern_count(&self) -> usize {
        self.pattern_ids.len()
    }

    pub(crate) const fn uses_assertions(&self) -> bool {
        self.uses_assertions
    }

    pub(crate) fn edges_from(&self, state: StateId) -> &[Edge] {
        let state = &self.states[state.index()];
        &self.edges[state.edge_start..state.edge_start + state.edge_len]
    }

    pub(crate) fn terminals_at(&self, state: StateId) -> &[usize] {
        let state = &self.states[state.index()];
        &self.terminal_ordinals[state.terminal_start..state.terminal_start + state.terminal_len]
    }

    pub(crate) fn pattern_id(&self, ordinal: usize) -> PatternId {
        self.pattern_ids[ordinal]
    }

    #[inline]
    pub(crate) fn edge_matches(&self, kind: EdgeKind, symbol: u16) -> bool {
        match kind {
            EdgeKind::Epsilon | EdgeKind::Assertion(_) => false,
            EdgeKind::Symbol(expected) => expected == symbol,
            EdgeKind::Predicate(predicate) => {
                matches_predicate(&self.predicates[predicate.index()], symbol)
            }
        }
    }
}

#[inline(never)]
fn matches_predicate(predicate: &SymbolPredicate, symbol: u16) -> bool {
    predicate.matches(symbol)
}

pub(crate) fn compile(patterns: &[HirPattern]) -> Result<PatternDatabase, Error> {
    let mut pending_edges: Vec<Vec<Edge>> = vec![Vec::new()];
    let mut pending_terminals: Vec<Vec<usize>> = vec![Vec::new()];
    let root = StateId::for_index(0)?;
    let mut pattern_ids = Vec::with_capacity(patterns.len());
    let mut predicates = Vec::new();
    let mut predicate_ids = HashMap::new();
    let mut uses_assertions = false;

    for (ordinal, pattern) in patterns.iter().enumerate() {
        let first = add_state(&mut pending_edges, &mut pending_terminals)?;
        let terminal = add_state(&mut pending_edges, &mut pending_terminals)?;
        pending_edges[root.index()].push(Edge {
            kind: EdgeKind::Epsilon,
            target: first,
        });
        compile_expression(
            pattern.expression(),
            first,
            terminal,
            &mut pending_edges,
            &mut pending_terminals,
            &mut predicates,
            &mut predicate_ids,
        )?;
        pending_terminals[terminal.index()].push(ordinal);
        pattern_ids.push(pattern.pattern_id());
        uses_assertions |= pattern.expression().uses_assertions();
    }

    let mut states = Vec::with_capacity(pending_edges.len());
    let edge_count = pending_edges.iter().map(Vec::len).sum();
    let terminal_count = pending_terminals.iter().map(Vec::len).sum();
    let mut edges = Vec::with_capacity(edge_count);
    let mut terminal_ordinals = Vec::with_capacity(terminal_count);
    for (state_edges, state_terminals) in pending_edges.into_iter().zip(pending_terminals) {
        let edge_start = edges.len();
        let edge_len = state_edges.len();
        edges.extend(state_edges);
        let terminal_start = terminal_ordinals.len();
        let terminal_len = state_terminals.len();
        terminal_ordinals.extend(state_terminals);
        states.push(State {
            edge_start,
            edge_len,
            terminal_start,
            terminal_len,
        });
    }

    Ok(PatternDatabase {
        root,
        states: states.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
        predicates: predicates.into_boxed_slice(),
        terminal_ordinals: terminal_ordinals.into_boxed_slice(),
        pattern_ids: pattern_ids.into_boxed_slice(),
        uses_assertions,
    })
}

fn compile_expression(
    expression: &Hir,
    start: StateId,
    end: StateId,
    edges: &mut Vec<Vec<Edge>>,
    terminals: &mut Vec<Vec<usize>>,
    predicates: &mut Vec<SymbolPredicate>,
    predicate_ids: &mut HashMap<SymbolPredicate, PredicateId>,
) -> Result<(), Error> {
    match expression {
        Hir::Never => {}
        Hir::Epsilon => edges[start.index()].push(Edge {
            kind: EdgeKind::Epsilon,
            target: end,
        }),
        Hir::Assertion(assertion) => edges[start.index()].push(Edge {
            kind: EdgeKind::Assertion(*assertion),
            target: end,
        }),
        Hir::Symbol(symbol) => edges[start.index()].push(Edge {
            kind: EdgeKind::Symbol(*symbol),
            target: end,
        }),
        Hir::Predicate(predicate) => edges[start.index()].push(Edge {
            kind: EdgeKind::Predicate(intern_predicate(
                predicates,
                predicate_ids,
                predicate.clone(),
            )?),
            target: end,
        }),
        Hir::Sequence(expressions) => {
            let mut current = start;
            for (index, item) in expressions.iter().enumerate() {
                let next = if index + 1 == expressions.len() {
                    end
                } else {
                    add_state(edges, terminals)?
                };
                compile_expression(
                    item,
                    current,
                    next,
                    edges,
                    terminals,
                    predicates,
                    predicate_ids,
                )?;
                current = next;
            }
        }
        Hir::Alternation(expressions) => {
            for branch in expressions {
                compile_expression(
                    branch,
                    start,
                    end,
                    edges,
                    terminals,
                    predicates,
                    predicate_ids,
                )?;
            }
        }
        Hir::Repeat {
            expression,
            min,
            max,
        } => compile_repetition(
            expression,
            *min,
            *max,
            start,
            end,
            edges,
            terminals,
            predicates,
            predicate_ids,
        )?,
    }
    Ok(())
}

// Keeping the complete mutable compilation context explicit here avoids hidden
// aliases while the compiler is still small; an arena context can replace it
// once additional edge families make that simplification worthwhile.
#[allow(clippy::too_many_arguments)]
fn compile_repetition(
    expression: &Hir,
    min: u16,
    max: Option<u16>,
    start: StateId,
    end: StateId,
    edges: &mut Vec<Vec<Edge>>,
    terminals: &mut Vec<Vec<usize>>,
    predicates: &mut Vec<SymbolPredicate>,
    predicate_ids: &mut HashMap<SymbolPredicate, PredicateId>,
) -> Result<(), Error> {
    let mut current = start;
    for required in 0..min {
        let next = if required + 1 == min && max == Some(min) {
            end
        } else {
            add_state(edges, terminals)?
        };
        compile_expression(
            expression,
            current,
            next,
            edges,
            terminals,
            predicates,
            predicate_ids,
        )?;
        current = next;
    }

    match max {
        Some(limit) if limit == min => {
            if min == 0 {
                add_epsilon(edges, start, end);
            }
        }
        Some(limit) => {
            for optional in min..limit {
                add_epsilon(edges, current, end);
                let next = if optional + 1 == limit {
                    end
                } else {
                    add_state(edges, terminals)?
                };
                compile_expression(
                    expression,
                    current,
                    next,
                    edges,
                    terminals,
                    predicates,
                    predicate_ids,
                )?;
                current = next;
            }
        }
        None => {
            add_epsilon(edges, current, end);
            let loop_end = add_state(edges, terminals)?;
            compile_expression(
                expression,
                current,
                loop_end,
                edges,
                terminals,
                predicates,
                predicate_ids,
            )?;
            add_epsilon(edges, loop_end, current);
        }
    }
    Ok(())
}

fn add_epsilon(edges: &mut [Vec<Edge>], start: StateId, end: StateId) {
    edges[start.index()].push(Edge {
        kind: EdgeKind::Epsilon,
        target: end,
    });
}

fn intern_predicate(
    predicates: &mut Vec<SymbolPredicate>,
    predicate_ids: &mut HashMap<SymbolPredicate, PredicateId>,
    predicate: SymbolPredicate,
) -> Result<PredicateId, Error> {
    if let Some(&id) = predicate_ids.get(&predicate) {
        Ok(id)
    } else {
        let id = PredicateId::for_index(predicates.len())?;
        predicate_ids.insert(predicate.clone(), id);
        predicates.push(predicate);
        Ok(id)
    }
}

fn add_state(
    edges: &mut Vec<Vec<Edge>>,
    terminals: &mut Vec<Vec<usize>>,
) -> Result<StateId, Error> {
    let id = StateId::for_index(edges.len())?;
    edges.push(Vec::new());
    terminals.push(Vec::new());
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::{EdgeKind, compile};
    use crate::PatternId;
    use crate::parser::parse;

    #[test]
    fn compiled_literal_has_dense_valid_shared_nfa() -> Result<(), crate::Error> {
        // Prepare
        let patterns = [parse(PatternId::new(1), "cat")?];

        // Test
        let database = compile(&patterns)?;

        // Assert
        assert_eq!(database.root.index(), 0);
        assert_eq!(database.state_count(), 5);
        assert_eq!(database.pattern_count(), 1);
        assert_eq!(database.edges_from(database.root).len(), 1);
        assert_eq!(
            database.edges_from(database.root)[0].kind,
            EdgeKind::Epsilon
        );
        for state_index in 0..database.state_count() {
            let state = super::StateId::for_index(state_index)?;
            for edge in database.edges_from(state) {
                assert!(edge.target.index() < database.state_count());
            }
            for &ordinal in database.terminals_at(state) {
                assert!(ordinal < database.pattern_count());
            }
        }
        let terminal_count = (0..database.state_count())
            .map(super::StateId::for_index)
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .map(|&state| database.terminals_at(state).len())
            .sum::<usize>();
        assert_eq!(terminal_count, 1);
        Ok(())
    }

    #[test]
    fn equal_dot_predicates_are_interned_once() -> Result<(), crate::Error> {
        // Prepare
        let patterns = [
            parse(PatternId::new(1), ".")?,
            parse(PatternId::new(2), "a.")?,
        ];

        // Test
        let database = compile(&patterns)?;
        let predicate_edges: Vec<_> = database
            .edges
            .iter()
            .filter_map(|edge| match edge.kind {
                EdgeKind::Predicate(id) => Some(id),
                EdgeKind::Epsilon | EdgeKind::Assertion(_) | EdgeKind::Symbol(_) => None,
            })
            .collect();

        // Assert
        assert_eq!(database.predicates.len(), 1);
        assert_eq!(predicate_edges.len(), 2);
        assert!(predicate_edges.windows(2).all(|ids| ids[0] == ids[1]));
        Ok(())
    }

    #[test]
    fn assertion_metadata_selects_the_contextual_scan_only_when_needed() -> Result<(), crate::Error>
    {
        // Prepare
        let literal = [parse(PatternId::new(1), "a")?];
        let anchored = [parse(PatternId::new(2), "^a")?];

        // Test
        let literal_database = compile(&literal)?;
        let anchored_database = compile(&anchored)?;

        // Assert
        assert!(!literal_database.uses_assertions());
        assert!(anchored_database.uses_assertions());
        assert!(
            anchored_database
                .edges
                .iter()
                .any(|edge| matches!(edge.kind, EdgeKind::Assertion(_)))
        );
        Ok(())
    }

    #[test]
    fn alternation_branches_and_converges_with_dense_valid_edges() -> Result<(), crate::Error> {
        // Prepare
        let patterns = [parse(PatternId::new(1), "a|(bc|d)")?];

        // Test
        let database = compile(&patterns)?;
        let first = database.edges_from(database.root)[0].target;
        let branch_edges = database.edges_from(first);

        // Assert
        assert_eq!(branch_edges.len(), 3);
        assert!(
            branch_edges
                .iter()
                .all(|edge| edge.kind != EdgeKind::Epsilon)
        );
        for state_index in 0..database.state_count() {
            let state = super::StateId::for_index(state_index)?;
            assert!(
                database
                    .edges_from(state)
                    .iter()
                    .all(|edge| edge.target.index() < database.state_count())
            );
        }
        assert_eq!(
            (0..database.state_count())
                .map(super::StateId::for_index)
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .map(|&state| database.terminals_at(state).len())
                .sum::<usize>(),
            1
        );
        Ok(())
    }

    #[test]
    fn open_repetition_compiles_a_reachable_consuming_cycle() -> Result<(), crate::Error> {
        // Prepare
        let patterns = [parse(PatternId::new(1), "(ab)+")?];

        // Test
        let database = compile(&patterns)?;
        let has_epsilon_back_edge = (0..database.state_count()).any(|source| {
            let state = super::StateId::for_index(source).expect("bounded test database");
            database
                .edges_from(state)
                .iter()
                .any(|edge| matches!(edge.kind, EdgeKind::Epsilon) && edge.target.index() <= source)
        });

        // Assert
        assert!(has_epsilon_back_edge);
        assert!(
            database
                .edges
                .iter()
                .any(|edge| edge.kind == EdgeKind::Symbol(u16::from(b'a')))
        );
        assert!(
            database
                .edges
                .iter()
                .any(|edge| edge.kind == EdgeKind::Symbol(u16::from(b'b')))
        );
        Ok(())
    }
}
