#![allow(dead_code, unused_variables)]

use std::cell::RefCell;
use std::collections::{HashSet, HashMap};
use std::fmt;
use std::iter;
use std::mem;
use std::rc::Rc;
use std::slice;

use regex_syntax::Parser;
use regex_syntax::hir::{self, Hir, HirKind};

use error::Result;

const DFA_DEAD: DFAStateID = 0;
const ALPHABET_SIZE: usize = 256;

type DFAStateID = u32;

struct DFA {
    /// The set of DFA states and their transitions. Transitions point to
    /// indices in this list.
    states: Vec<DFAState>,
    /// The initial start state. This is either `0` for an empty DFA with a
    /// single dead state or `1` for the first DFA state built.
    start: DFAStateID,
}

struct DFAState {
    is_match: bool,
    transitions: Box<[DFAStateID]>,
}

impl DFA {
    fn empty() -> DFA {
        let dead = Rc::new(DFABuilderState::dead());
        let mut cache = HashMap::new();
        cache.insert(dead.clone(), DFA_DEAD);
        DFA {
            states: vec![DFAState::empty()],
            start: DFA_DEAD,
        }
    }

    fn from_nfa(nfa: &NFA) -> DFA {
        DFABuilder::new(nfa).build()
    }

    fn set_transition(&mut self, from: DFAStateID, b: u8, to: DFAStateID) {
        self.states[from as usize].transitions[b as usize] = to;
    }

    fn find(&self, bytes: &[u8]) -> Option<usize> {
        let mut state = self.start;
        let mut last_match = None;
        for (i, &b) in bytes.iter().enumerate() {
            state = self.states[state as usize].transitions[b as usize];
            if state == DFA_DEAD {
                return last_match;
            } else if self.states[state as usize].is_match {
                last_match = Some(i + 1);
            }
        }
        last_match
    }
}

impl DFAState {
    fn empty() -> DFAState {
        DFAState {
            is_match: false,
            transitions: vec![DFA_DEAD; ALPHABET_SIZE].into_boxed_slice(),
        }
    }

    fn sparse_transitions(&self) -> Vec<(u8, u8, DFAStateID)> {
        let mut ranges = vec![];
        let mut cur = None;
        for (i, &next_id) in self.transitions.iter().enumerate() {
            let b = i as u8;
            let (prev_start, prev_end, prev_next) = match cur {
                Some(range) => range,
                None => {
                    cur = Some((b, b, next_id));
                    continue;
                }
            };
            if prev_next == next_id {
                cur = Some((prev_start, b, prev_next));
            } else {
                ranges.push((prev_start, prev_end, prev_next));
                cur = Some((b, b, next_id));
            }
        }
        ranges.push(cur.unwrap());
        ranges
    }
}

struct DFABuilder<'a> {
    /// The NFA we're converting into a DFA.
    nfa: &'a NFA,
    /// The DFA we're building.
    dfa: DFA,
    /// Each DFA state being built is defined as an *ordered* set of NFA
    /// states.
    ///
    /// This is never empty. The first state is always a dummy state such that
    /// DFAStateID == 0 corresponds to a dead state.
    builder_states: Vec<Rc<DFABuilderState>>,
    /// A cache of DFA states that already exist and can be easily looked up
    /// via ordered sets of NFA states.
    cache: HashMap<Rc<DFABuilderState>, DFAStateID>,
    /// A stack of NFA states to visit, for depth first visiting.
    stack: Vec<NFAStateID>,
    /// Scratch space for storing an ordered sequence of NFA states, for
    /// amortizing allocation.
    scratch_nfa_states: Vec<NFAStateID>,
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct DFABuilderState {
    is_match: bool,
    nfa_states: Vec<NFAStateID>,
}

impl<'a> DFABuilder<'a> {
    fn new(nfa: &'a NFA) -> DFABuilder<'a> {
        let dead = Rc::new(DFABuilderState::dead());
        let mut cache = HashMap::new();
        cache.insert(dead.clone(), DFA_DEAD);

        DFABuilder {
            nfa: nfa,
            dfa: DFA::empty(),
            builder_states: vec![dead],
            cache: cache,
            stack: vec![],
            scratch_nfa_states: vec![],
        }
    }

    fn build(mut self) -> DFA {
        let mut sparse = self.new_sparse_set();
        let mut uncompiled = vec![self.add_start(&mut sparse)];
        let mut queued: HashSet<DFAStateID> = HashSet::new();
        while let Some(dfa_id) = uncompiled.pop() {
            for b in 0..=255 {
                let next_dfa_id = self.cached_state(dfa_id, b, &mut sparse);
                self.dfa.set_transition(dfa_id, b, next_dfa_id);
                if !queued.contains(&next_dfa_id) {
                    uncompiled.push(next_dfa_id);
                    queued.insert(next_dfa_id);
                }
            }
        }
        // for (i, s) in self.builder_states.iter().enumerate() {
            // let status = if s.is_match { '*' } else { ' ' };
            // println!("{}{:04}: {:?}", status, i, s.nfa_states);
        // }
        self.dfa
    }

    fn cached_state(
        &mut self,
        dfa_id: DFAStateID,
        b: u8,
        sparse: &mut SparseSet,
    ) -> DFAStateID {
        sparse.clear();
        self.next(dfa_id, b, sparse);
        let state = self.new_state(sparse);
        if let Some(&cached_id) = self.cache.get(&state) {
            mem::replace(&mut self.scratch_nfa_states, state.nfa_states);
            return cached_id;
        }
        self.add_state(state)
    }

    fn next(
        &mut self,
        dfa_id: DFAStateID,
        b: u8,
        next_nfa_states: &mut SparseSet,
    ) {
        next_nfa_states.clear();
        for i in 0..self.builder_states[dfa_id as usize].nfa_states.len() {
            let nfa_id = self.builder_states[dfa_id as usize].nfa_states[i];
            match self.nfa.states.borrow()[nfa_id as usize] {
                NFAState::Range { start, end, next } => {
                    if start <= b && b <= end {
                        self.epsilon_closure(next, next_nfa_states);
                    }
                }
                | NFAState::Empty { .. }
                | NFAState::Union { .. }
                | NFAState::Match => {}
            }
        }
    }

    fn epsilon_closure(&mut self, start: NFAStateID, set: &mut SparseSet) {
        if !self.nfa.states.borrow()[start as usize].is_epsilon() {
            set.insert(start);
            return;
        }

        self.stack.push(start);
        while let Some(mut id) = self.stack.pop() {
            loop {
                if set.contains(id) {
                    break;
                }
                set.insert(id);
                match self.nfa.states.borrow()[id as usize] {
                    NFAState::Empty { next } => {
                        id = next;
                    }
                    NFAState::Union { ref alternates, .. } => {
                        id = match alternates.get(0) {
                            None => break,
                            Some(&id) => id,
                        };
                        self.stack.extend(alternates[1..].iter().rev());
                    }
                    NFAState::Range { .. } | NFAState::Match => break,
                }
            }
        }
    }

    fn add_start(&mut self, sparse: &mut SparseSet) -> DFAStateID {
        self.epsilon_closure(0, sparse);
        let state = self.new_state(&sparse);
        let id = self.add_state(state);
        self.dfa.start = id;
        id
    }

    fn add_state(&mut self, state: DFABuilderState) -> DFAStateID {
        let id = self.dfa.states.len() as DFAStateID;
        self.dfa.states.push(DFAState {
            is_match: state.is_match,
            ..DFAState::empty()
        });

        let rstate = Rc::new(state);
        self.builder_states.push(rstate.clone());
        self.cache.insert(rstate, id);
        id
    }

    fn new_state(&mut self, set: &SparseSet) -> DFABuilderState {
        let mut state = DFABuilderState {
            is_match: false,
            nfa_states: mem::replace(&mut self.scratch_nfa_states, vec![]),
        };
        state.nfa_states.clear();

        for &id in set {
            match self.nfa.states.borrow()[id as usize] {
                NFAState::Range { .. } => {
                    state.nfa_states.push(id);
                }
                NFAState::Match => {
                    state.is_match = true;
                    break;
                }
                NFAState::Empty { .. } | NFAState::Union { .. } => {}
            }
        }
        state
    }

    fn new_sparse_set(&self) -> SparseSet {
        SparseSet::new(self.nfa.states.borrow().len())
    }
}

impl DFABuilderState {
    fn dead() -> DFABuilderState {
        DFABuilderState { nfa_states: vec![], is_match: false }
    }
}

#[derive(Debug)]
struct Minimizer<'a> {
    dfa: &'a mut DFA,
    in_transitions: Vec<Vec<Vec<DFAStateID>>>,
    partitions: Vec<StateSet>,
    waiting: Vec<StateSet>,
    // waiting_set: BTreeSet<StateSet>,
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct StateSet(Rc<RefCell<Vec<DFAStateID>>>);

impl<'a> Minimizer<'a> {
    fn new(dfa: &'a mut DFA) -> Minimizer<'a> {
        let in_transitions = Minimizer::incoming_transitions(dfa);
        let partitions = Minimizer::initial_partitions(dfa);
        let waiting = vec![partitions[0].clone()];
        // let mut waiting_set = BTreeSet::new();
        // waiting_set.insert(partitions[0].clone());

        // Minimizer { dfa, in_transitions, partitions, waiting, waiting_set }
        Minimizer { dfa, in_transitions, partitions, waiting }
    }

    fn run(mut self) {
        let mut incoming = StateSet::empty();

        while let Some(set) = self.waiting.pop() {
            for b in 0..=255 {
                self.find_incoming_to(b, &set, &mut incoming);

                let mut newparts = vec![];
                for p in 0..self.partitions.len() {
                    let x = self.partitions[p].intersection(&incoming);
                    if x.is_empty() {
                        newparts.push(self.partitions[p].clone());
                        continue;
                    }

                    let y = self.partitions[p].subtract(&incoming);
                    if y.is_empty() {
                        newparts.push(self.partitions[p].clone());
                        continue;
                    }

                    newparts.push(x.clone());
                    newparts.push(y.clone());
                    match self.waiting.iter().position(|s| s == &self.partitions[p]) {
                        Some(i) => {
                            self.waiting[i] = x;
                            self.waiting.push(y);
                        }
                        None => {
                            if x.len() <= y.len() {
                                self.waiting.push(x);
                            } else {
                                self.waiting.push(y);
                            }
                        }
                    }
                }
                self.partitions = newparts;
            }
        }

        let mut state_to_part = vec![DFA_DEAD; self.dfa.states.len()];
        for p in &self.partitions {
            p.iter(|id| state_to_part[id as usize] = p.first());
        }

        let mut minimal_ids = vec![DFA_DEAD; self.dfa.states.len()];
        let mut new_id = 0;
        for (id, state) in self.dfa.states.iter().enumerate() {
            if state_to_part[id] == id as DFAStateID {
                minimal_ids[id] = new_id;
                new_id += 1;
            }
        }
        let minimal_count = new_id as usize;

        for id in 0..self.dfa.states.len() {
            if state_to_part[id] != id as DFAStateID {
                continue;
            }
            for next in self.dfa.states[id].transitions.iter_mut() {
                *next = minimal_ids[state_to_part[*next as usize] as usize];
            }
            self.dfa.states.swap(id, minimal_ids[id] as usize);
        }
        self.dfa.states.truncate(minimal_count);
    }

    fn find_incoming_to(
        &self,
        b: u8,
        set: &StateSet,
        incoming: &mut StateSet,
    ) {
        incoming.clear();
        set.iter(|id| {
            for &inid in &self.in_transitions[id as usize][b as usize] {
                incoming.add(inid);
            }
        });
        incoming.canonicalize();
    }

    fn initial_partitions(dfa: &DFA) -> Vec<StateSet> {
        let mut is_match = StateSet::empty();
        let mut no_match = StateSet::empty();
        for (i, state) in dfa.states.iter().enumerate() {
            let id = i as DFAStateID;
            if state.is_match {
                is_match.add(id);
            } else {
                no_match.add(id);
            }
        }
        assert!(!is_match.is_empty(), "must have at least one matching state");

        let mut sets = vec![is_match];
        if !no_match.is_empty() {
            sets.push(no_match);
        }
        sets.sort_by_key(|s| s.len());
        sets
    }

    fn incoming_transitions(dfa: &DFA) -> Vec<Vec<Vec<DFAStateID>>> {
        let mut incoming = vec![];
        for state in dfa.states.iter() {
            incoming.push(vec![vec![]; ALPHABET_SIZE]);
        }
        for (i, state) in dfa.states.iter().enumerate() {
            let id = i as DFAStateID;
            for (b, &next) in state.transitions.iter().enumerate() {
                incoming[next as usize][b].push(id);
            }
        }
        incoming
    }
}

impl StateSet {
    fn empty() -> StateSet {
        StateSet(Rc::new(RefCell::new(vec![])))
    }

    fn add(&mut self, id: DFAStateID) {
        self.0.borrow_mut().push(id);
    }

    fn first(&self) -> DFAStateID {
        self.0.borrow()[0]
    }

    fn canonicalize(&mut self) {
        self.0.borrow_mut().sort();
        self.0.borrow_mut().dedup();
    }

    fn clear(&mut self) {
        self.0.borrow_mut().clear();
    }

    fn len(&self) -> usize {
        self.0.borrow().len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn deep_clone(&self) -> StateSet {
        let ids = self.0.borrow().iter().cloned().collect();
        StateSet(Rc::new(RefCell::new(ids)))
    }

    fn iter(&self, mut f: impl FnMut(DFAStateID)) {
        for &id in self.0.borrow().iter() {
            f(id);
        }
    }

    fn intersection(&self, other: &StateSet) -> StateSet {
        if self.is_empty() || other.is_empty() {
            return StateSet::empty();
        }

        let mut result = StateSet::empty();
        let (seta, setb) = (self.0.borrow(), other.0.borrow());
        let (mut ita, mut itb) = (seta.iter().cloned(), setb.iter().cloned());
        let (mut a, mut b) = (ita.next().unwrap(), itb.next().unwrap());
        loop {
            if a == b {
                result.add(a);
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
                b = match itb.next() {
                    None => break,
                    Some(b) => b,
                };
            } else if a < b {
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
            } else {
                b = match itb.next() {
                    None => break,
                    Some(b) => b,
                };
            }
        }
        result
    }

    fn subtract(&self, other: &StateSet) -> StateSet {
        if self.is_empty() || other.is_empty() {
            return self.deep_clone();
        }

        let mut result = StateSet::empty();
        let (seta, setb) = (self.0.borrow(), other.0.borrow());
        let (mut ita, mut itb) = (seta.iter().cloned(), setb.iter().cloned());
        let (mut a, mut b) = (ita.next().unwrap(), itb.next().unwrap());
        loop {
            if a == b {
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
                b = match itb.next() {
                    None => { result.add(a); break; }
                    Some(b) => b,
                };
            } else if a < b {
                result.add(a);
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
            } else {
                b = match itb.next() {
                    None => { result.add(a); break; }
                    Some(b) => b,
                };
            }
        }
        for a in ita {
            result.add(a);
        }
        result
    }
}

#[derive(Debug)]
struct NFA {
    states: RefCell<Vec<NFAState>>,
}

type NFAStateID = u32;

#[derive(Debug)]
enum NFAState {
    Empty { next: NFAStateID },
    Range { start: u8, end: u8, next: NFAStateID },
    Union { alternates: Vec<NFAStateID>, reverse: bool },
    Match,
}

#[derive(Debug)]
struct ThompsonRef {
    start: NFAStateID,
    end: NFAStateID,
}

impl NFA {
    fn empty() -> NFA {
        NFA { states: RefCell::new(vec![]) }
    }

    fn from_pattern(pattern: &str) -> Result<NFA> {
        NFA::from_hir(&Parser::new().parse(pattern)?)
    }

    fn from_hir(expr: &Hir) -> Result<NFA> {
        let nfa = NFA::empty();
        let start = nfa.add_empty();
        let compiled = nfa.compile(expr)?;
        let match_id = nfa.add_match();
        nfa.patch(start, compiled.start);
        nfa.patch(compiled.end, match_id);
        Ok(nfa)
    }

    fn compile(&self, expr: &Hir) -> Result<ThompsonRef> {
        match expr.kind() {
            HirKind::Empty => {
                let id = self.add_empty();
                Ok(ThompsonRef { start: id, end: id })
            }
            HirKind::Literal(hir::Literal::Unicode(ch)) => {
                let mut buf = [0; 4];
                let it = ch
                    .encode_utf8(&mut buf)
                    .as_bytes()
                    .iter()
                    .map(|&b| Ok(self.compile_range(b, b)));
                self.compile_concat(it)
            }
            HirKind::Literal(hir::Literal::Byte(b)) => {
                Ok(self.compile_range(*b, *b))
            }
            HirKind::Class(hir::Class::Bytes(ref cls)) => {
                let it = cls
                    .iter()
                    .map(|rng| Ok(self.compile_range(rng.start(), rng.end())));
                self.compile_alternation(it)
            }
            HirKind::Class(hir::Class::Unicode(ref cls)) => {
                self.compile_unicode_class(cls)
            }
            HirKind::Repetition(ref rep) => {
                self.compile_repetition(rep)
            }
            HirKind::Group(ref group) => {
                self.compile(&*group.hir)
            }
            HirKind::Concat(ref exprs) => {
                self.compile_concat(exprs.iter().map(|e| self.compile(e)))
            }
            HirKind::Alternation(ref exprs) => {
                self.compile_alternation(exprs.iter().map(|e| self.compile(e)))
            }
            HirKind::Anchor(_) => {
                return err!("anchors are not supported");
            }
            HirKind::WordBoundary(_) => {
                return err!("word boundaries are not supported");
            }
        }
    }

    fn compile_concat<I>(
        &self,
        mut it: I,
    ) -> Result<ThompsonRef>
    where I: Iterator<Item=Result<ThompsonRef>>
    {
        let ThompsonRef { start, mut end } = match it.next() {
            Some(result) => result?,
            None => return Ok(self.compile_empty()),
        };
        for result in it {
            let compiled = result?;
            self.patch(end, compiled.start);
            end = compiled.end;
        }
        Ok(ThompsonRef { start, end })
    }

    fn compile_alternation<I>(
        &self,
        it: I,
    ) -> Result<ThompsonRef>
    where I: Iterator<Item=Result<ThompsonRef>>
    {
        let union = self.add_union();

        let mut alternate_ends = vec![];
        for result in it {
            let compiled = result?;
            self.patch(union, compiled.start);
            alternate_ends.push(compiled.end);
        }
        assert!(!alternate_ends.is_empty(), "alternations must be non-empty");

        let empty = self.add_empty();
        for id in alternate_ends {
            self.patch(id, empty);
        }
        Ok(ThompsonRef { start: union, end: empty })
    }

    fn compile_repetition(
        &self,
        rep: &hir::Repetition,
    ) -> Result<ThompsonRef> {
        match rep.kind {
            hir::RepetitionKind::ZeroOrOne => {
                self.compile_zero_or_one(&rep.hir, rep.greedy)
            }
            hir::RepetitionKind::ZeroOrMore => {
                self.compile_at_least(&rep.hir, rep.greedy, 0)
            }
            hir::RepetitionKind::OneOrMore => {
                self.compile_at_least(&rep.hir, rep.greedy, 1)
            }
            hir::RepetitionKind::Range(ref rng) => {
                match *rng {
                    hir::RepetitionRange::Exactly(count) => {
                        self.compile_exactly(&rep.hir, count)
                    }
                    hir::RepetitionRange::AtLeast(m) => {
                        self.compile_at_least(&rep.hir, rep.greedy, m)
                    }
                    hir::RepetitionRange::Bounded(min, max) => {
                        self.compile_bounded(&rep.hir, rep.greedy, min, max)
                    }
                }
            }
        }
    }

    fn compile_bounded(
        &self,
        expr: &Hir,
        greedy: bool,
        min: u32,
        max: u32,
    ) -> Result<ThompsonRef> {
        let prefix = self.compile_exactly(expr, min)?;
        if min == max {
            return Ok(prefix);
        }

        let suffix = self.compile_concat(
            (min..max).map(|_| self.compile_zero_or_one(expr, greedy))
        )?;
        self.patch(prefix.end, suffix.start);
        Ok(ThompsonRef {
            start: prefix.start,
            end: suffix.end,
        })
    }

    fn compile_at_least(
        &self,
        expr: &Hir,
        greedy: bool,
        n: u32,
    ) -> Result<ThompsonRef> {
        if n == 0 {
            let union =
                if greedy {
                    self.add_union()
                } else {
                    self.add_reverse_union()
                };
            let compiled = self.compile(expr)?;
            self.patch(union, compiled.start);
            self.patch(compiled.end, union);
            Ok(ThompsonRef { start: union, end: union })
        } else if n == 1 {
            let compiled = self.compile(expr)?;
            let union =
                if greedy {
                    self.add_union()
                } else {
                    self.add_reverse_union()
                };
            self.patch(compiled.end, union);
            self.patch(union, compiled.start);
            Ok(ThompsonRef { start: compiled.start, end: union })
        } else {
            let prefix = self.compile_exactly(expr, n - 1)?;
            let last = self.compile(expr)?;
            let union =
                if greedy {
                    self.add_union()
                } else {
                    self.add_reverse_union()
                };
            self.patch(prefix.end, last.start);
            self.patch(last.end, union);
            self.patch(union, last.start);
            Ok(ThompsonRef { start: prefix.start, end: union })
        }
    }

    fn compile_zero_or_one(
        &self,
        expr: &Hir,
        greedy: bool,
    ) -> Result<ThompsonRef> {
        let union =
            if greedy {
                self.add_union()
            } else {
                self.add_reverse_union()
            };
        let compiled = self.compile(expr)?;
        let empty = self.add_empty();
        self.patch(union, compiled.start);
        self.patch(union, empty);
        self.patch(compiled.end, empty);
        Ok(ThompsonRef { start: union, end: empty })
    }

    fn compile_exactly(&self, expr: &Hir, n: u32) -> Result<ThompsonRef> {
        let it = iter::repeat(())
            .take(n as usize)
            .map(|_| self.compile(expr));
        self.compile_concat(it)
    }

    fn compile_unicode_class(
        &self,
        cls: &hir::ClassUnicode,
    ) -> Result<ThompsonRef> {
        use utf8_ranges::Utf8Sequences;

        let it = cls
            .iter()
            .flat_map(|rng| Utf8Sequences::new(rng.start(), rng.end()))
            .map(|seq| {
                let it = seq.as_slice()
                    .iter()
                    .map(|rng| Ok(self.compile_range(rng.start, rng.end)));
                self.compile_concat(it)
            });
        self.compile_alternation(it)
    }

    fn compile_range(&self, start: u8, end: u8) -> ThompsonRef {
        let id = self.add_range(start, end);
        ThompsonRef { start: id, end: id }
    }

    fn compile_empty(&self) -> ThompsonRef {
        let id = self.add_empty();
        ThompsonRef { start: id, end: id }
    }

    fn patch(&self, from: NFAStateID, to: NFAStateID) {
        match self.states.borrow_mut()[from as usize] {
            NFAState::Empty { ref mut next } => {
                *next = to;
            }
            NFAState::Range { ref mut next, .. } => {
                *next = to;
            }
            NFAState::Union { ref mut alternates, reverse: false } => {
                alternates.push(to);
            }
            NFAState::Union { ref mut alternates, reverse: true } => {
                alternates.insert(0, to);
            }
            NFAState::Match => {}
        }
    }

    fn add_empty(&self) -> NFAStateID {
        let id = self.states.borrow().len() as NFAStateID;
        self.states.borrow_mut().push(NFAState::Empty { next: 0 });
        id
    }

    fn add_range(&self, start: u8, end: u8) -> NFAStateID {
        let id = self.states.borrow().len() as NFAStateID;
        let state = NFAState::Range { start, end, next: 0 };
        self.states.borrow_mut().push(state);
        id
    }

    fn add_union(&self) -> NFAStateID {
        let id = self.states.borrow().len() as NFAStateID;
        let state = NFAState::Union { alternates: vec![], reverse: false };
        self.states.borrow_mut().push(state);
        id
    }

    fn add_reverse_union(&self) -> NFAStateID {
        let id = self.states.borrow().len() as NFAStateID;
        let state = NFAState::Union { alternates: vec![], reverse: true };
        self.states.borrow_mut().push(state);
        id
    }

    fn add_match(&self) -> NFAStateID {
        let id = self.states.borrow().len() as NFAStateID;
        self.states.borrow_mut().push(NFAState::Match);
        id
    }
}

impl NFAState {
    fn is_epsilon(&self) -> bool {
        match *self {
            NFAState::Range { .. } | NFAState::Match => false,
            NFAState::Empty { .. } | NFAState::Union { .. } => true,
        }
    }
}

/// A sparse set used for representing ordered NFA states.
///
/// This supports constant time addition and membership testing. Clearing an
/// entire set can also be done in constant time. Iteration yields elements
/// in the order in which they were inserted.
///
/// The data structure is based on: http://research.swtch.com/sparse
/// Note though that we don't actually use uninitialized memory. We generally
/// reuse allocations, so the initial allocation cost is bareable. However,
/// its other properties listed above are extremely useful.
#[derive(Clone, Debug)]
struct SparseSet {
    /// Dense contains the instruction pointers in the order in which they
    /// were inserted.
    dense: Vec<NFAStateID>,
    /// Sparse maps instruction pointers to their location in dense.
    ///
    /// An instruction pointer is in the set if and only if
    /// sparse[ip] < dense.len() && ip == dense[sparse[ip]].
    sparse: Box<[NFAStateID]>,
}

impl SparseSet {
    fn new(size: usize) -> SparseSet {
        SparseSet {
            dense: Vec::with_capacity(size),
            sparse: vec![0; size].into_boxed_slice(),
        }
    }

    fn len(&self) -> usize {
        self.dense.len()
    }

    fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    fn capacity(&self) -> usize {
        self.dense.capacity()
    }

    fn insert(&mut self, value: NFAStateID) {
        let i = self.len();
        assert!(i < self.capacity());
        self.dense.push(value);
        self.sparse[value as usize] = i as u32;
    }

    fn contains(&self, value: NFAStateID) -> bool {
        let i = self.sparse[value as usize];
        self.dense.get(i as usize) == Some(&value)
    }

    fn clear(&mut self) {
        self.dense.clear();
    }
}

impl<'a> IntoIterator for &'a SparseSet {
    type Item = &'a NFAStateID;
    type IntoIter = slice::Iter<'a, NFAStateID>;
    fn into_iter(self) -> Self::IntoIter { self.dense.iter() }
}

impl fmt::Debug for DFA {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn state_status(id: DFAStateID, state: &DFAState) -> String {
            let mut status = vec![b' ', b' '];
            if id == 0 {
                status[0] = b'D';
            } else if id == 1 {
                status[0] = b'>';
            }
            if state.is_match {
                status[1] = b'*';
            }
            String::from_utf8(status).unwrap()
        }

        for (i, state) in self.states.iter().enumerate() {
            let id = i as DFAStateID;
            writeln!(f, "{}{:04}: {:?}", state_status(id, state), id, state)?;
        }
        Ok(())
    }
}

impl fmt::Debug for DFAState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut transitions = vec![];
        for (start, end, next_id) in self.sparse_transitions() {
            if next_id == DFA_DEAD {
                continue;
            }
            let line =
                if start == end {
                    format!("{} => {}", escape(start), next_id)
                } else {
                    format!(
                        "{}-{} => {}",
                        escape(start), escape(end), next_id,
                    )
                };
            transitions.push(line);
        }
        write!(f, "{}", transitions.join(", "))?;
        Ok(())
    }
}

/// Return the given byte as its escaped string form.
fn escape(b: u8) -> String {
    use std::ascii;

    String::from_utf8(ascii::escape_default(b).collect::<Vec<_>>()).unwrap()
}

#[cfg(test)]
mod tests {
    use ucd_parse::GraphemeClusterBreakTest;
    use super::*;

    fn print_automata(pattern: &str) {
        let (nfa, mut dfa) = build_automata(pattern);

        println!("{}", "#".repeat(100));
        println!("PATTERN: {:?}", pattern);
        println!("NFA:");
        for (i, state) in nfa.states.borrow().iter().enumerate() {
            println!("{:03X}: {:X?}", i, state);
        }

        println!("{}", "~".repeat(79));

        println!("DFA:");
        print!("{:?}", dfa);
        println!("{}", "~".repeat(79));

        Minimizer::new(&mut dfa).run();

        println!("Minimal DFA:");
        print!("{:?}", dfa);
        println!("{}", "~".repeat(79));

        println!("{}", "#".repeat(100));
    }

    fn build_automata(pattern: &str) -> (NFA, DFA) {
        let nfa = NFA::from_pattern(pattern).unwrap();
        let dfa = DFA::from_nfa(&nfa);
        (nfa, dfa)
    }

    fn build_automata_min(pattern: &str) -> DFA {
        let (_, mut dfa) = build_automata(pattern);
        Minimizer::new(&mut dfa).run();
        dfa
    }

    #[test]
    fn scratch() {
        // print_automata(grapheme_pattern());
        let (nfa, mut dfa) = build_automata(grapheme_pattern());
        // let (nfa, dfa) = build_automata(r"a");
        println!("# dfa states: {}", dfa.states.len());
        println!("# dfa transitions: {}", 256 * dfa.states.len());
        Minimizer::new(&mut dfa).run();
        println!("# minimal dfa states: {}", dfa.states.len());
        println!("# minimal dfa transitions: {}", 256 * dfa.states.len());
        // print_automata(r"\p{any}");
        // print_automata(r"[\u007F-\u0080]");

        // println!("building...");
        // let dfa = grapheme_dfa();
        // let dfa = build_automata_min(r"a|\p{gcb=RI}\p{gcb=RI}|\p{gcb=RI}");
        // println!("searching...");
        // let string = "\u{1f1e6}\u{1f1e6}";
        // let bytes = string.as_bytes();
        // println!("{:?}", dfa.find(bytes));

        // print_automata("a|zz|z");
        // let dfa = build_automata_min(r"a|zz|z");
        // println!("searching...");
        // let string = "zz";
        // let bytes = string.as_bytes();
        // println!("{:?}", dfa.find(bytes));
    }

    fn grapheme_dfa() -> DFA {
        let nfa = NFA::from_pattern(grapheme_pattern()).unwrap();
        let mut dfa = DFA::from_nfa(&nfa);
        Minimizer::new(&mut dfa).run();
        dfa
    }

    fn grapheme_pattern() -> &'static str {
        r"(?x)
            (?:
                \p{gcb=CR}\p{gcb=LF}
                |
                [\p{gcb=Control}\p{gcb=CR}\p{gcb=LF}]
                |
                \p{gcb=Prepend}*
                (?:
                    (?:
                        (?:
                            \p{gcb=L}*
                            (?:\p{gcb=V}+|\p{gcb=LV}\p{gcb=V}*|\p{gcb=LVT})
                            \p{gcb=T}*
                        )
                        |
                        \p{gcb=L}+
                        |
                        \p{gcb=T}+
                    )
                    |
                    \p{gcb=RI}\p{gcb=RI}
                    |
                    \p{Extended_Pictographic}
                    (?:\p{gcb=Extend}*\p{gcb=ZWJ}\p{Extended_Pictographic})*
                    |
                    [^\p{gcb=Control}\p{gcb=CR}\p{gcb=LF}]
                )
                [\p{gcb=Extend}\p{gcb=ZWJ}\p{gcb=SpacingMark}]*
            )
        "
    }

    fn grapheme_pattern_tweak() -> &'static str {
        r"(?x)
            (?:
                (?:
                    a
                    |
                    \p{gcb=RI}\p{gcb=RI}
                    |
                    \p{gcb=RI}
                )
            )
        "
    }

    const TESTDATA: &'static str = include_str!(
        "../tmp/ucd-11.0.0/auxiliary/GraphemeBreakTest.txt",
    );

    lazy_static! {
        static ref GRAPHEME_DFA: DFA = grapheme_dfa();
    }

    #[derive(Clone, Debug)]
    struct Graphemes<'a> {
        bytes: &'a [u8],
    }

    impl<'a> Iterator for Graphemes<'a> {
        type Item = &'a str;

        #[inline]
        fn next(&mut self) -> Option<&'a str> {
            let end = match GRAPHEME_DFA.find(self.bytes) {
                None => return None,
                Some(end) => end,
            };
            let grapheme = &self.bytes[..end];
            self.bytes = &self.bytes[end..];
            Some(::std::str::from_utf8(grapheme).unwrap())
        }
    }

    #[test]
    fn ucdtests() {
        let start = ::std::time::Instant::now();
        assert_eq!(Some(1), GRAPHEME_DFA.find(b"a"));
        println!("building took: {:?}", ::std::time::Instant::now().duration_since(start));

        let start = ::std::time::Instant::now();
        let mut count = 0;
        for (i, mut line) in TESTDATA.lines().enumerate() {
            line = line.trim();
            if line.starts_with("#") || line.contains("surrogate") {
                continue;
            }
            let test: GraphemeClusterBreakTest = line.parse().unwrap();
            let given: String = test.grapheme_clusters
                .iter()
                .map(|cluster| cluster.to_string())
                .collect();
            let got: Vec<String> = Graphemes { bytes: given.as_bytes() }
                .map(|cluster| cluster.to_string())
                .collect();
            assert_eq!(
                test.grapheme_clusters,
                got,
                "\nGraphemeBreakTest.txt, line {}: {}\n\
                   expected: {:?}\n\
                   got:      {:?}\n",
                i+1, line,
                uniescape_vec(&test.grapheme_clusters),
                uniescape_vec(&got),
            );
            count += 1;
        }
        println!("{} tests took: {:?}", count, ::std::time::Instant::now().duration_since(start));
    }

    fn uniescape(s: &str) -> String {
        s.chars().flat_map(|c| c.escape_unicode()).collect::<String>()
    }

    fn uniescape_vec(strs: &[String]) -> Vec<String> {
        strs.iter().map(|s| uniescape(s)).collect()
    }
}
