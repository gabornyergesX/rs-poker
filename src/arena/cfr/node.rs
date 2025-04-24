use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde::ser::SerializeStruct;
use serde::de;

#[derive(Debug, Clone)]
pub struct PlayerData {
    pub regret_matcher: Option<Box<little_sorry::RegretMatcher>>,
    pub player_idx: usize,
}

// Custom Serialize for PlayerData to handle RegretMatcher
impl Serialize for PlayerData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("PlayerData", 2)?;
        // Skip RegretMatcher serialization for now (will need to be recreated)
        state.serialize_field("regret_matcher", &None::<()>)?;
        state.serialize_field("player_idx", &self.player_idx)?;
        state.end()
    }
}

// Custom Deserialize for PlayerData to handle RegretMatcher
impl<'de> Deserialize<'de> for PlayerData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct PlayerDataHelper {
            #[serde(default)]
            regret_matcher: Option<()>,
            player_idx: usize,
        }

        let helper = PlayerDataHelper::deserialize(deserializer)?;
        Ok(PlayerData {
            regret_matcher: None, // We recreate this as needed
            player_idx: helper.player_idx,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalData {
    pub total_utility: f32,
}

impl TerminalData {
    pub fn new(total_utility: f32) -> Self {
        TerminalData { total_utility }
    }
}

impl Default for TerminalData {
    fn default() -> Self {
        TerminalData::new(0.0)
    }
}

// The base node type for Poker CFR
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeData {
    /// The root node.
    ///
    /// This node is always the first node in the tree, we don't
    /// use the GameStart action to create the node. By egarly
    /// creating the root node we can simplify the traversal.
    /// All that's required is to ignore GameStart, ForcedBet, and
    /// PlayerSit actions as they are all assumed in the root node.
    ///
    /// For all traversals we start at the root node and then follow the
    /// 0th child node for the first real action that follows from
    /// the starting game state. That could be a chance card if the player
    /// is going to get dealt starting hands, or it could be the first
    /// player action if the gamestate starts with hands already dealt.
    Root,

    /// A chance node.
    ///
    /// This node represents the dealing of a single card.
    /// Each child index in the children array represents a card.
    /// The count array is used to track the number of times a card
    /// has been dealt.
    Chance,
    Player(PlayerData),
    Terminal(TerminalData),
}

impl NodeData {
    pub fn is_terminal(&self) -> bool {
        matches!(self, NodeData::Terminal(_))
    }

    pub fn is_chance(&self) -> bool {
        matches!(self, NodeData::Chance)
    }

    pub fn is_player(&self) -> bool {
        matches!(self, NodeData::Player(_))
    }

    pub fn is_root(&self) -> bool {
        matches!(self, NodeData::Root)
    }
}

impl std::fmt::Display for NodeData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeData::Root => write!(f, "Root"),
            NodeData::Chance => write!(f, "Chance"),
            NodeData::Player(_) => write!(f, "Player"),
            NodeData::Terminal(_) => write!(f, "Terminal"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub idx: usize,
    pub data: NodeData,
    pub parent: Option<usize>,
    pub parent_child_idx: Option<usize>,

    // We use an array of Option<usize> to represent the children of the node.
    // The index of the array is the action index or the card index for chance nodes.
    //
    // This limits the number of possible agent actions to 52, but in return we
    // get contiguous memory for no pointer chasing.
    children: [Option<usize>; 52],
    count: [u32; 52],
}

// Custom Serialize for Node to handle the arrays
impl Serialize for Node {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Node", 6)?;
        state.serialize_field("idx", &self.idx)?;
        state.serialize_field("data", &self.data)?;
        state.serialize_field("parent", &self.parent)?;
        state.serialize_field("parent_child_idx", &self.parent_child_idx)?;

        // Convert fixed arrays to Vec for serialization
        let children_vec: Vec<Option<usize>> = self.children.to_vec();
        let count_vec: Vec<u32> = self.count.to_vec();
        
        state.serialize_field("children", &children_vec)?;
        state.serialize_field("count", &count_vec)?;
        state.end()
    }
}

// Custom Deserialize for Node to handle the arrays
impl<'de> Deserialize<'de> for Node {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NodeHelper {
            idx: usize,
            data: NodeData,
            parent: Option<usize>,
            parent_child_idx: Option<usize>,
            children: Vec<Option<usize>>,
            count: Vec<u32>,
        }

        let helper = NodeHelper::deserialize(deserializer)?;
        
        // Convert Vec back to fixed arrays
        let mut children = [None; 52];
        let mut count = [0; 52];
        
        for (i, child) in helper.children.iter().enumerate().take(52) {
            children[i] = *child;
        }
        
        for (i, c) in helper.count.iter().enumerate().take(52) {
            count[i] = *c;
        }
        
        Ok(Node {
            idx: helper.idx,
            data: helper.data,
            parent: helper.parent,
            parent_child_idx: helper.parent_child_idx,
            children,
            count,
        })
    }
}

impl Node {
    pub fn new_root() -> Self {
        Node {
            idx: 0,
            data: NodeData::Root,
            parent: Some(0),
            parent_child_idx: None,
            children: [None; 52],
            count: [0; 52],
        }
    }

    /// Create a new node with the provided index, parent index, and data.
    ///
    /// # Arguments
    ///
    /// * `idx` - The index of the node
    /// * `parent` - The index of the parent node
    /// * `data` - The data for the node
    ///
    /// # Returns
    ///
    /// A new node with the provided index, parent index, and data.
    ///
    /// # Example
    ///
    /// ```
    /// use rs_poker::arena::cfr::{Node, NodeData};
    ///
    /// let idx = 1;
    /// let parent = 0;
    /// let parent_child_idx = 0;
    /// let data = NodeData::Chance;
    /// let node = Node::new(idx, parent, parent_child_idx, data);
    /// ```
    pub fn new(idx: usize, parent: usize, parent_child_idx: usize, data: NodeData) -> Self {
        Node {
            idx,
            data,
            parent: Some(parent),
            parent_child_idx: Some(parent_child_idx),
            children: [None; 52],
            count: [0; 52],
        }
    }

    // Set child node at the provided index
    pub fn set_child(&mut self, idx: usize, child: usize) {
        assert_eq!(self.children[idx], None);
        self.children[idx] = Some(child);
    }

    // Get the child node at the provided index
    pub fn get_child(&self, idx: usize) -> Option<usize> {
        self.children[idx]
    }

    // Increment the count for the provided index
    pub fn increment_count(&mut self, idx: usize) {
        assert!(idx == 0 || !self.data.is_terminal());
        self.count[idx] += 1;
    }

    /// Get an iterator over all the node's children with their indices
    ///
    /// This is useful for traversing the tree for visualization or debugging.
    ///
    /// # Returns
    ///
    /// An iterator over tuples of (child_idx, child_node_idx) where:
    /// - child_idx is the index in the children array
    /// - child_node_idx is the index of the child node in the nodes vector
    pub fn iter_children(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.children
            .iter()
            .enumerate()
            .filter_map(|(idx, &child)| child.map(|c| (idx, c)))
    }

    /// Get the count for a specific child index
    ///
    /// # Arguments
    ///
    /// * `idx` - The index of the child
    ///
    /// # Returns
    ///
    /// The count for the specified child
    pub fn get_count(&self, idx: usize) -> u32 {
        self.count[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_terminal_data_default() {
        let terminal_data = TerminalData::default();
        assert_eq!(terminal_data.total_utility, 0.0);
    }

    #[test]
    fn test_terminal_data_new() {
        let terminal_data = TerminalData::new(10.0);
        assert_eq!(terminal_data.total_utility, 10.0);
    }

    #[test]
    fn test_node_data_is_terminal() {
        let node_data = NodeData::Terminal(TerminalData::new(10.0));
        assert!(node_data.is_terminal());
    }

    #[test]
    fn test_node_data_is_chance() {
        let node_data = NodeData::Chance;
        assert!(node_data.is_chance());
    }

    #[test]
    fn test_node_data_is_player() {
        let node_data = NodeData::Player(PlayerData {
            regret_matcher: None,
            player_idx: 0,
        });
        assert!(node_data.is_player());
    }

    #[test]
    fn test_node_data_is_root() {
        let node_data = NodeData::Root;
        assert!(node_data.is_root());
    }

    #[test]
    fn test_node_new_root() {
        let node = Node::new_root();
        assert_eq!(node.idx, 0);
        // Root is it's own parent
        assert!(node.parent.is_some());
        assert_eq!(node.parent, Some(0));
        assert!(matches!(node.data, NodeData::Root));
    }

    #[test]
    fn test_node_new() {
        let node = Node::new(1, 0, 0, NodeData::Chance);
        assert_eq!(node.idx, 1);
        assert_eq!(node.parent, Some(0));
        assert!(matches!(node.data, NodeData::Chance));
    }

    #[test]
    fn test_node_set_get_child() {
        let mut node = Node::new(1, 0, 0, NodeData::Chance);
        node.set_child(0, 2);
        assert_eq!(node.get_child(0), Some(2));
    }

    #[test]
    fn test_node_increment_count() {
        let mut node = Node::new(1, 0, 0, NodeData::Chance);
        node.increment_count(0);
        assert_eq!(node.count[0], 1);
    }

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
    fn test_player_data_serialization() {
        // Create PlayerData with a RegretMatcher
        let mut regret_matcher = little_sorry::RegretMatcher::new(5).unwrap();
        let player_data = PlayerData {
            regret_matcher: Some(Box::new(regret_matcher)),
            player_idx: 7,
        };
        
        // Serialize to JSON
        let json = serde_json::to_string(&player_data).expect("Failed to serialize PlayerData");
        
        // Deserialize from JSON
        let deserialized_data: PlayerData = serde_json::from_str(&json).expect("Failed to deserialize PlayerData");
        
        // Verify player index was preserved
        assert_eq!(deserialized_data.player_idx, 7);
        
        // Verify RegretMatcher was dropped during serialization (as expected)
        assert!(deserialized_data.regret_matcher.is_none());
    }
    
    #[test]
    fn test_node_data_serialization() {
        // Test each NodeData variant
        let variants = vec![
            NodeData::Root,
            NodeData::Chance,
            NodeData::Player(PlayerData {
                regret_matcher: None,
                player_idx: 3,
            }),
            NodeData::Terminal(TerminalData::new(42.5)),
        ];
        
        for original_data in variants {
            // Serialize to JSON
            let json = serde_json::to_string(&original_data).expect("Failed to serialize NodeData");
            
            // Deserialize from JSON
            let deserialized_data: NodeData = serde_json::from_str(&json).expect("Failed to deserialize NodeData");
            
            // Verify type is preserved
            assert_eq!(original_data.is_root(), deserialized_data.is_root());
            assert_eq!(original_data.is_chance(), deserialized_data.is_chance());
            assert_eq!(original_data.is_player(), deserialized_data.is_player());
            assert_eq!(original_data.is_terminal(), deserialized_data.is_terminal());
            
            // For terminal data, also check the utility value
            if let NodeData::Terminal(ref original_terminal) = original_data {
                if let NodeData::Terminal(ref deserialized_terminal) = deserialized_data {
                    assert_eq!(original_terminal.total_utility, deserialized_terminal.total_utility);
                }
            }
            
            // For player data, check the player index
            if let NodeData::Player(ref original_player) = original_data {
                if let NodeData::Player(ref deserialized_player) = deserialized_data {
                    assert_eq!(original_player.player_idx, deserialized_player.player_idx);
                }
            }
        }
    }
}
