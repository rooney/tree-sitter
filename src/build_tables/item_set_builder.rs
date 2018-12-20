use super::inline_variables::InlinedProductionMap;
use super::item::{LookaheadSet, ParseItem, ParseItemSet};
use crate::grammars::{LexicalGrammar, SyntaxGrammar};
use crate::rules::Symbol;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
struct TransitiveClosureAddition {
    item: ParseItem,
    info: FollowSetInfo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FollowSetInfo {
    lookaheads: LookaheadSet,
    propagates_lookaheads: bool,
}

pub(crate) struct ParseItemSetBuilder {
    first_sets: HashMap<Symbol, LookaheadSet>,
    last_sets: HashMap<Symbol, LookaheadSet>,
    transitive_closure_additions: Vec<Vec<TransitiveClosureAddition>>,
    pub inlines: InlinedProductionMap,
}

fn find_or_push<T: Eq>(vector: &mut Vec<T>, value: T) {
    if !vector.contains(&value) {
        vector.push(value);
    }
}

impl ParseItemSetBuilder {
    pub fn new(syntax_grammar: &SyntaxGrammar, lexical_grammar: &LexicalGrammar) -> Self {
        let mut result = Self {
            first_sets: HashMap::new(),
            last_sets: HashMap::new(),
            transitive_closure_additions: vec![Vec::new(); syntax_grammar.variables.len()],
            inlines: InlinedProductionMap::new(syntax_grammar),
        };

        // For each grammar symbol, populate the FIRST and LAST sets: the set of
        // terminals that appear at the beginning and end that symbol's productions,
        // respectively.
        //
        // For a terminal symbol, the FIRST and LAST set just consists of the
        // terminal itself.
        for i in 0..lexical_grammar.variables.len() {
            let symbol = Symbol::terminal(i);
            let mut set = LookaheadSet::new();
            set.insert(symbol);
            result.first_sets.insert(symbol, set.clone());
            result.last_sets.insert(symbol, set);
        }

        for i in 0..syntax_grammar.external_tokens.len() {
            let symbol = Symbol::external(i);
            let mut set = LookaheadSet::new();
            set.insert(symbol);
            result.first_sets.insert(symbol, set.clone());
            result.last_sets.insert(symbol, set);
        }

        // The FIRST set of a non-terminal `i` is the union of the following sets:
        // * the set of all terminals that appear at the beginings of i's productions
        // * the FIRST sets of all the non-terminals that appear at the beginnings
        //   of i's productions
        //
        // Rather than computing these sets using recursion, we use an explicit stack
        // called `symbols_to_process`.
        let mut symbols_to_process = Vec::new();
        let mut processed_non_terminals = HashSet::new();
        for i in 0..syntax_grammar.variables.len() {
            let symbol = Symbol::non_terminal(i);

            let first_set = &mut result
                .first_sets
                .entry(symbol)
                .or_insert(LookaheadSet::new());
            processed_non_terminals.clear();
            symbols_to_process.clear();
            symbols_to_process.push(symbol);
            while let Some(current_symbol) = symbols_to_process.pop() {
                if current_symbol.is_terminal() || current_symbol.is_external() {
                    first_set.insert(current_symbol);
                } else if processed_non_terminals.insert(current_symbol) {
                    for production in syntax_grammar.variables[current_symbol.index]
                        .productions
                        .iter()
                    {
                        if let Some(step) = production.steps.first() {
                            symbols_to_process.push(step.symbol);
                        }
                    }
                }
            }

            // The LAST set is defined in a similar way to the FIRST set.
            let last_set = &mut result
                .last_sets
                .entry(symbol)
                .or_insert(LookaheadSet::new());
            processed_non_terminals.clear();
            symbols_to_process.clear();
            symbols_to_process.push(symbol);
            while let Some(current_symbol) = symbols_to_process.pop() {
                if current_symbol.is_terminal() || current_symbol.is_external() {
                    last_set.insert(current_symbol);
                } else if processed_non_terminals.insert(current_symbol) {
                    for production in syntax_grammar.variables[current_symbol.index]
                        .productions
                        .iter()
                    {
                        if let Some(step) = production.steps.last() {
                            symbols_to_process.push(step.symbol);
                        }
                    }
                }
            }
        }

        // To compute an item set's transitive closure, we find each item in the set
        // whose next symbol is a non-terminal, and we add new items to the set for
        // each of that symbols' productions. These productions might themselves begin
        // with non-terminals, so the process continues recursively. In this process,
        // the total set of entries that get added depends only on two things:
        //   * the set of non-terminal symbols that occur at each item's current position
        //   * the set of terminals that occurs after each of these non-terminal symbols
        //
        // So we can avoid a lot of duplicated recursive work by precomputing, for each
        // non-terminal symbol `i`, a final list of *additions* that must be made to an
        // item set when `i` occurs as the next symbol in one if its core items. The
        // structure of an *addition* is as follows:
        //   * `item` - the new item that must be added as part of the expansion of `i`
        //   * `lookaheads` - lookahead tokens that can always come after that item in
        //      the expansion of `i`
        //   * `propagates_lookaheads` - a boolean indicating whether or not `item` can
        //      occur at the *end* of the expansion of `i`, so that i's own current
        //      lookahead tokens can occur after `item`.
        //
        // Again, rather than computing these additions recursively, we use an explicit
        // stack called `entries_to_process`.
        for i in 0..syntax_grammar.variables.len() {
            let empty_lookaheads = LookaheadSet::new();
            let mut entries_to_process = vec![(i, &empty_lookaheads, true)];

            // First, build up a map whose keys are all of the non-terminals that can
            // appear at the beginning of non-terminal `i`, and whose values store
            // information about the tokens that can follow each non-terminal.
            let mut follow_set_info_by_non_terminal = HashMap::new();
            while let Some(entry) = entries_to_process.pop() {
                let (variable_index, lookaheads, propagates_lookaheads) = entry;
                let existing_info = follow_set_info_by_non_terminal
                    .entry(variable_index)
                    .or_insert_with(|| FollowSetInfo {
                        lookaheads: LookaheadSet::new(),
                        propagates_lookaheads: false,
                    });

                let did_add_follow_set_info;
                if propagates_lookaheads {
                    did_add_follow_set_info = !existing_info.propagates_lookaheads;
                    existing_info.propagates_lookaheads = true;
                } else {
                    did_add_follow_set_info = existing_info.lookaheads.insert_all(lookaheads);
                }

                if did_add_follow_set_info {
                    for production in &syntax_grammar.variables[variable_index].productions {
                        if let Some(symbol) = production.first_symbol() {
                            if symbol.is_non_terminal() {
                                if production.steps.len() == 1 {
                                    entries_to_process.push((
                                        symbol.index,
                                        lookaheads,
                                        propagates_lookaheads,
                                    ));
                                } else {
                                    entries_to_process.push((
                                        symbol.index,
                                        &result.first_sets[&production.steps[1].symbol],
                                        false,
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            // Store all of those non-terminals' productions, along with their associated
            // lookahead info, as *additions* associated with non-terminal `i`.
            let additions_for_non_terminal = &mut result.transitive_closure_additions[i];
            for (variable_index, follow_set_info) in follow_set_info_by_non_terminal {
                let variable = &syntax_grammar.variables[variable_index];
                let non_terminal = Symbol::non_terminal(variable_index);
                if syntax_grammar.variables_to_inline.contains(&non_terminal) {
                    continue;
                }
                for production_index in 0..variable.productions.len() {
                    let item = ParseItem::Normal {
                        variable_index: variable_index as u32,
                        production_index: production_index as u32,
                        step_index: 0,
                    };

                    if let Some(inlined_items) = result.inlines.inlined_items(item) {
                        for inlined_item in inlined_items {
                            find_or_push(
                                additions_for_non_terminal,
                                TransitiveClosureAddition {
                                    item: inlined_item,
                                    info: follow_set_info.clone(),
                                },
                            );
                        }
                    } else {
                        find_or_push(
                            additions_for_non_terminal,
                            TransitiveClosureAddition {
                                item,
                                info: follow_set_info.clone(),
                            },
                        );
                    }
                }
            }
        }

        result
    }

    pub(crate) fn transitive_closure(
        &mut self,
        item_set: &ParseItemSet,
        grammar: &SyntaxGrammar,
    ) -> ParseItemSet {
        let mut result = ParseItemSet::default();
        for (item, lookaheads) in &item_set.entries {
            if let Some(items) = self.inlines.inlined_items(*item) {
                for item in items {
                    self.add_item(&mut result, item, lookaheads, grammar);
                }
            } else {
                self.add_item(&mut result, *item, lookaheads, grammar);
            }
        }
        result
    }

    pub fn first_set(&self, symbol: &Symbol) -> &LookaheadSet {
        &self.first_sets[symbol]
    }

    fn add_item(
        &self,
        set: &mut ParseItemSet,
        item: ParseItem,
        lookaheads: &LookaheadSet,
        grammar: &SyntaxGrammar,
    ) {
        if let Some(step) = item.step(grammar, &self.inlines) {
            if step.symbol.is_non_terminal() {
                let next_step = item.successor().step(grammar, &self.inlines);

                // Determine which tokens can follow this non-terminal.
                let following_tokens = if let Some(next_step) = next_step {
                    self.first_sets.get(&next_step.symbol).unwrap()
                } else {
                    &lookaheads
                };

                // Use the pre-computed *additions* to expand the non-terminal.
                for addition in &self.transitive_closure_additions[step.symbol.index] {
                    let lookaheads = set
                        .entries
                        .entry(addition.item)
                        .or_insert_with(|| LookaheadSet::new());
                    lookaheads.insert_all(&addition.info.lookaheads);
                    if addition.info.propagates_lookaheads {
                        lookaheads.insert_all(following_tokens);
                    }
                }
            }
        }
        set.entries.insert(item, lookaheads.clone());
    }
}
