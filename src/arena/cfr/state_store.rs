use std::fs;
use std::path::Path;
use std::rc::Rc;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use crate::arena::GameState;
use anyhow::Result;

use super::{CFRState, TraversalState};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateStoreInternal {
    // The tree structure of counter factual regret.
    pub cfr_states: Vec<CFRState>,

    // The current place in the tree that each player is at. This is used as a stack
    pub traversal_states: Vec<Vec<TraversalState>>,
}

/// `StateStore` is a structure to hold all CFR states and other data needed for
/// a single game that is being solved. Since all players use the same store it
/// enables reuse of the memory and regret matchers of all players.
///
/// This state store is not thread safe so it has to be used in a single thread.
#[derive(Debug, Clone)]
pub struct StateStore {
    inner: Rc<std::cell::RefCell<StateStoreInternal>>,
}

// Custom Serialize implementation for StateStore
impl Serialize for StateStore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let internal = self.inner.borrow();
        internal.serialize(serializer)
    }
}

// Custom Deserialize implementation for StateStore
impl<'de> Deserialize<'de> for StateStore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let internal = StateStoreInternal::deserialize(deserializer)?;
        Ok(StateStore {
            inner: Rc::new(std::cell::RefCell::new(internal)),
        })
    }
}

impl StateStore {
    pub fn new() -> Self {
        StateStore {
            inner: Rc::new(std::cell::RefCell::new(StateStoreInternal {
                cfr_states: Vec::new(),
                traversal_states: Vec::new(),
            })),
        }
    }

    /// Merges the contents of another StateStore into this one.
    /// This appends all CFRStates and TraversalStates from `other` into `self`.
    pub fn merge_from(&mut self, other: &StateStore) {
        let mut self_inner = self.inner.borrow_mut();
        let other_inner = other.inner.borrow();

        for state in &other_inner.cfr_states {
            self_inner.cfr_states.push(state.clone());
        }

        for traversal in &other_inner.traversal_states {
            self_inner.traversal_states.push(traversal.clone());
        }
    }

    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let serialized = serde_json::to_string(self)?;
        fs::write(path, serialized)?;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&contents)?)
    }
    pub fn len(&self) -> usize {
        self.inner.borrow().cfr_states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn traversal_len(&self, player_idx: usize) -> usize {
        self.inner
            .borrow()
            .traversal_states
            .get(player_idx)
            .map_or(0, |traversal| traversal.len())
    }

    pub fn peek_traversal(&self, player_idx: usize) -> Option<TraversalState> {
        self.inner
            .borrow()
            .traversal_states
            .get(player_idx)
            .and_then(|traversal| traversal.last().cloned())
    }

    pub fn new_state(
        &mut self,
        game_state: GameState,
        player_idx: usize,
    ) -> (CFRState, TraversalState) {
        let mut inner = self.inner.borrow_mut();

        // Add the CFR State
        inner.cfr_states.push(CFRState::new(game_state));

        // We want a root traversal state for the new player
        // This won't ever be changed.
        inner
            .traversal_states
            .push(vec![TraversalState::new_root(player_idx)]);

        let traversal_states = inner
            .traversal_states
            .get_mut(player_idx)
            .unwrap_or_else(|| panic!("Traversal state for player {player_idx} not found"));

        let last = traversal_states.last().expect("No traversal state found");

        // Make a copy and put it in the stack
        let new_traversal_state =
            TraversalState::new(last.node_idx(), last.chosen_child_idx(), last.player_idx());

        // Create a new traversal state based on the last one
        traversal_states.push(new_traversal_state.clone());

        // Get a clone of the cfr state to give out.
        let state = inner
            .cfr_states
            .get(player_idx)
            .unwrap_or_else(|| panic!("State for player {player_idx} not found"))
            .clone();

        (state, new_traversal_state)
    }

    pub fn push_traversal(&mut self, player_idx: usize) -> (CFRState, TraversalState) {
        let mut inner = self.inner.borrow_mut();

        let traversal_states = inner
            .traversal_states
            .get_mut(player_idx)
            .unwrap_or_else(|| panic!("Traversal state for player {player_idx} not found"));

        let last = traversal_states.last().expect("No traversal state found");

        // Make a copy and put it in the stack
        let new_traversal_state =
            TraversalState::new(last.node_idx(), last.chosen_child_idx(), last.player_idx());

        // Create a new traversal state based on the last one
        traversal_states.push(new_traversal_state.clone());

        let cfr_state = inner
            .cfr_states
            .get(player_idx)
            .unwrap_or_else(|| panic!("State for player {player_idx} not found"))
            .clone();

        (cfr_state, new_traversal_state)
    }

    pub fn pop_traversal(&mut self, player_idx: usize) {
        let mut inner = self.inner.borrow_mut();
        let traversal_states = inner
            .traversal_states
            .get_mut(player_idx)
            .expect("Traversal state for player not found");
        assert!(
            !traversal_states.is_empty(),
            "No traversal state to pop for player {player_idx}"
        );
        traversal_states.pop();
    }
}

impl Default for StateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_new() {
        let store = StateStore::new();
        assert_eq!(store.len(), 0, "New state store should have no states");
    }

    #[test]
    fn test_push() {
        let mut state_store = StateStore::new();
        let game_state = GameState::new_starting(vec![100.0; 3], 10.0, 5.0, 0.0, 0);
        let (state, _traversal) = state_store.new_state(game_state.clone(), 0);
        assert_eq!(
            state_store.len(),
            1,
            "State store should have one state after push"
        );
        assert_eq!(
            state.starting_game_state(),
            game_state,
            "State should match the game state"
        );
    }

    #[test]
    fn test_push_len() {
        let mut state_store = StateStore::new();

        let game_state = GameState::new_starting(vec![100.0; 3], 10.0, 5.0, 0.0, 0);

        let _stores = (0..2)
            .map(|i| {
                let (state, traversal) = state_store.new_state(game_state.clone(), i);
                assert_eq!(
                    state_store.len(),
                    i + 1,
                    "State store should have one state after push"
                );
                (state, traversal)
            })
            .collect::<Vec<_>>();

        assert_eq!(2, state_store.len(), "State store should have two states");

        let mut store_clones = (0..2).map(|_| state_store.clone()).collect::<Vec<_>>();

        for (player_idx, cloned_state_store) in store_clones.iter_mut().enumerate() {
            assert_eq!(
                cloned_state_store.len(),
                2,
                "Cloned state store should have two states"
            );

            let (_, _) = cloned_state_store.push_traversal(player_idx);
            assert_eq!(
                cloned_state_store.len(),
                2,
                "Cloned state store should still have two states"
            );
        }

        for i in 0..2 {
            state_store.pop_traversal(i);
        }
    }
    
    #[test]
    fn test_save_load_file() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("state_store.json");
        
        // Create state store with some data
        let mut state_store = StateStore::new();
        let game_state = GameState::new_starting(vec![100.0; 3], 10.0, 5.0, 0.0, 0);
        let (_state, _traversal) = state_store.new_state(game_state.clone(), 0);
        let (_state2, _traversal2) = state_store.new_state(game_state.clone(), 1);
        
        // Save to file
        state_store.save_to_file(&file_path)?;
        
        // Load from file
        let loaded_store = StateStore::load_from_file(&file_path)?;
        
        // Verify loaded store has the same number of states
        assert_eq!(loaded_store.len(), state_store.len(), 
            "Loaded state store should have the same number of states");
        
        Ok(())
    }
}