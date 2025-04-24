//! Integration test for serialization
//! 
//! Run with: cargo test --test test_serialization --features=serde,arena

#[cfg(feature = "serde")]
mod tests {
    use rs_poker::arena::GameState;
    use rs_poker::arena::cfr::{CFRState, Node, NodeData, PlayerData, StateStore, TerminalData, TraversalState};
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;
    
    #[test]
    fn test_node_serialization() {
        let original_node = Node::new(5, 1, 2, NodeData::Chance);
        
        // Set some children and increment counts to test array serialization
        let mut node = original_node.clone();
        node.set_child(0, 10);
        node.set_child(3, 15);
        node.increment_count(0);
        node.increment_count(0);
        node.increment_count(3);
        
        // Serialize to JSON
        let json = serde_json::to_string(&node).expect("Failed to serialize Node");
        println!("Serialized Node: {}", json);
        
        // Deserialize from JSON
        let deserialized_node: Node = serde_json::from_str(&json).expect("Failed to deserialize Node");
        
        // Verify node properties
        assert_eq!(deserialized_node.idx, 5);
        assert_eq!(deserialized_node.parent, Some(1));
        assert_eq!(deserialized_node.parent_child_idx, Some(2));
        assert!(matches!(deserialized_node.data, NodeData::Chance));
        
        // Verify children and count arrays were preserved
        assert_eq!(deserialized_node.get_child(0), Some(10));
        assert_eq!(deserialized_node.get_child(3), Some(15));
        assert_eq!(deserialized_node.get_count(0), 2);
        assert_eq!(deserialized_node.get_count(3), 1);
    }
    
    #[test]
    fn test_cfr_state_serialization() {
        let game_state = GameState::new_starting(vec![100.0; 3], 10.0, 5.0, 0.0, 0);
        let cfr_state = CFRState::new(game_state.clone());
        
        // Serialize to JSON
        let json = serde_json::to_string(&cfr_state).expect("Failed to serialize CFRState");
        println!("Serialized CFRState: {}", json);
        
        // Deserialize from JSON
        let deserialized_state: CFRState = serde_json::from_str(&json).expect("Failed to deserialize CFRState");
        
        // Check that the deserialized state has the root node
        let root_node = deserialized_state.get(0).expect("No root node found");
        assert!(matches!(root_node.data, NodeData::Root), "Root node data should be NodeData::Root");
        
        // Check that the starting game state was properly serialized
        assert_eq!(deserialized_state.starting_game_state().big_blind, 10.0);
        assert_eq!(deserialized_state.starting_game_state().small_blind, 5.0);
    }
    
    #[test]
    fn test_traversal_state_serialization() {
        let traversal = TraversalState::new(42, 7, 3);
        
        // Serialize to JSON
        let json = serde_json::to_string(&traversal).expect("Failed to serialize TraversalState");
        println!("Serialized TraversalState: {}", json);
        
        // Deserialize from JSON
        let deserialized_traversal: TraversalState = serde_json::from_str(&json).expect("Failed to deserialize TraversalState");
        
        // Check that the properties were preserved
        assert_eq!(deserialized_traversal.node_idx(), 42);
        assert_eq!(deserialized_traversal.chosen_child_idx(), 7);
        assert_eq!(deserialized_traversal.player_idx(), 3);
    }
    
    #[test]
    fn test_player_data_serialization() {
        // Create PlayerData with a RegretMatcher
        let regret_matcher = little_sorry::RegretMatcher::new(5).unwrap();
        let player_data = PlayerData {
            regret_matcher: Some(Box::new(regret_matcher)),
            player_idx: 7,
        };
        
        // Serialize to JSON
        let json = serde_json::to_string(&player_data).expect("Failed to serialize PlayerData");
        println!("Serialized PlayerData: {}", json);
        
        // Deserialize from JSON
        let deserialized_data: PlayerData = serde_json::from_str(&json).expect("Failed to deserialize PlayerData");
        
        // Verify player index was preserved
        assert_eq!(deserialized_data.player_idx, 7);
        
        // Verify RegretMatcher was dropped during serialization (as expected)
        assert!(deserialized_data.regret_matcher.is_none());
    }
    
    #[test]
    fn test_state_store_save_load() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("state_store.json");
        
        // Create state store with some data
        let mut state_store = StateStore::new();
        let game_state = GameState::new_starting(vec![100.0; 3], 10.0, 5.0, 0.0, 0);
        let (_state, _traversal) = state_store.new_state(game_state.clone(), 0);
        let (_state2, _traversal2) = state_store.new_state(game_state.clone(), 1);
        
        // Save to file
        state_store.save_to_file(&file_path).unwrap();
        println!("State store saved to: {:?}", file_path);
        
        // Read the file content
        let content = fs::read_to_string(&file_path).unwrap();
        println!("File content: {}", content);
        
        // Load from file
        let loaded_store = StateStore::load_from_file(&file_path).unwrap();
        
        // Verify loaded store has the same number of states
        assert_eq!(loaded_store.len(), state_store.len(), 
            "Loaded state store should have the same number of states");
        
        println!("Test passed! State store serialization works correctly.");
    }
} 