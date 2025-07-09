//! # Search Match Types
//!
//! Provides types for representing search matches that can work with both
//! native ast-grep types and serializable results.

use crate::types::MatchResult;
use ast_grep_core::tree_sitter::StrDoc;
use ast_grep_core::{Node, NodeMatch};
use ast_grep_language::SupportLang as Language;
use std::borrow::Cow;

/// A search match that can be either a native AST node match or a converted result.
///
/// This allows us to work with the rich AST information internally while still
/// being able to serialize results for the MCP protocol.
#[derive(Clone)]
pub enum SearchMatch<'a> {
    /// A native ast-grep NodeMatch with full AST access
    Native(NodeMatch<'a, StrDoc<Language>>),
    /// A converted, serializable match result
    Converted(MatchResult),
}

// Manual Debug implementation since NodeMatch doesn't implement Debug
impl<'a> std::fmt::Debug for SearchMatch<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchMatch::Native(n) => {
                let node = n.get_node();
                let pos = node.start_pos();
                write!(
                    f,
                    "SearchMatch::Native(NodeMatch at {}:{})",
                    pos.line(),
                    pos.column(n)
                )
            }
            SearchMatch::Converted(m) => write!(f, "SearchMatch::Converted({m:?})"),
        }
    }
}

impl<'a> SearchMatch<'a> {
    /// Create a SearchMatch from a NodeMatch
    pub fn from_node_match(node_match: NodeMatch<'a, StrDoc<Language>>) -> Self {
        SearchMatch::Native(node_match)
    }

    /// Convert this match to a serializable MatchResult
    pub fn to_match_result(&self) -> MatchResult {
        match self {
            SearchMatch::Native(n) => MatchResult::from_node_match(n),
            SearchMatch::Converted(m) => m.clone(),
        }
    }

    /// Get the underlying AST node if this is a native match
    pub fn get_node(&self) -> Option<&Node<'a, StrDoc<Language>>> {
        match self {
            SearchMatch::Native(n) => Some(n.get_node()),
            SearchMatch::Converted(_) => None,
        }
    }

    /// Get the matched text
    pub fn text(&self) -> Cow<str> {
        match self {
            SearchMatch::Native(n) => {
                let text = n.get_node().text();
                match text {
                    Cow::Borrowed(s) => Cow::Borrowed(s),
                    Cow::Owned(s) => Cow::Owned(s),
                }
            }
            SearchMatch::Converted(m) => Cow::Borrowed(&m.text),
        }
    }

    /// Get the start line number (0-based)
    pub fn start_line(&self) -> usize {
        match self {
            SearchMatch::Native(n) => n.get_node().start_pos().line(),
            SearchMatch::Converted(m) => m.start_line,
        }
    }

    /// Get the end line number (0-based)
    pub fn end_line(&self) -> usize {
        match self {
            SearchMatch::Native(n) => n.get_node().end_pos().line(),
            SearchMatch::Converted(m) => m.end_line,
        }
    }

    /// Get the start column (0-based)
    pub fn start_col(&self) -> usize {
        match self {
            SearchMatch::Native(n) => n.get_node().start_pos().column(n),
            SearchMatch::Converted(m) => m.start_col,
        }
    }

    /// Get the end column (0-based)
    pub fn end_col(&self) -> usize {
        match self {
            SearchMatch::Native(n) => n.get_node().end_pos().column(n),
            SearchMatch::Converted(m) => m.end_col,
        }
    }

    /// Check if this match has a native AST node
    pub fn has_node(&self) -> bool {
        matches!(self, SearchMatch::Native(_))
    }

    /// Get the node kind if this is a native match
    pub fn kind(&self) -> Option<Cow<str>> {
        self.get_node().map(|n| n.kind())
    }

    /// Get parent node if this is a native match
    pub fn parent(&self) -> Option<Node<'a, StrDoc<Language>>> {
        self.get_node().and_then(|n| n.parent())
    }

    /// Get children if this is a native match
    pub fn children(&self) -> Vec<Node<'a, StrDoc<Language>>> {
        self.get_node()
            .map(|n| n.children().collect())
            .unwrap_or_default()
    }

    /// Find matches within this node using a pattern
    pub fn find_all(
        &self,
        pattern: &ast_grep_core::Pattern,
    ) -> Vec<NodeMatch<'a, StrDoc<Language>>> {
        self.get_node()
            .map(|n| n.find_all(pattern).collect())
            .unwrap_or_default()
    }
}

/// Collection of search matches that can be converted to serializable results
#[derive(Debug)]
pub struct SearchMatches<'a> {
    matches: Vec<SearchMatch<'a>>,
}

impl<'a> SearchMatches<'a> {
    /// Create a new collection of search matches
    pub fn new(matches: Vec<SearchMatch<'a>>) -> Self {
        SearchMatches { matches }
    }

    /// Create from native NodeMatch objects
    pub fn from_node_matches(
        node_matches: impl Iterator<Item = NodeMatch<'a, StrDoc<Language>>>,
    ) -> Self {
        let matches = node_matches.map(SearchMatch::from_node_match).collect();
        SearchMatches { matches }
    }

    /// Convert all matches to serializable MatchResults
    pub fn to_match_results(&self) -> Vec<MatchResult> {
        self.matches.iter().map(|m| m.to_match_result()).collect()
    }

    /// Get an iterator over the matches
    pub fn iter(&self) -> impl Iterator<Item = &SearchMatch<'a>> {
        self.matches.iter()
    }

    /// Get the number of matches
    pub fn len(&self) -> usize {
        self.matches.len()
    }

    /// Check if there are no matches
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Filter matches that have native AST nodes
    pub fn with_nodes(&self) -> impl Iterator<Item = &SearchMatch<'a>> {
        self.matches.iter().filter(|m| m.has_node())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MatchResult;
    use std::collections::HashMap;

    #[test]
    fn test_search_match_converted() {
        let match_result = MatchResult {
            text: "test".to_string(),
            start_line: 1,
            end_line: 1,
            start_col: 0,
            end_col: 4,
            vars: HashMap::new(),
            context_before: None,
            context_after: None,
        };

        let search_match = SearchMatch::Converted(match_result.clone());

        assert_eq!(search_match.text(), "test");
        assert_eq!(search_match.start_line(), 1);
        assert_eq!(search_match.end_line(), 1);
        assert_eq!(search_match.start_col(), 0);
        assert_eq!(search_match.end_col(), 4);
        assert!(!search_match.has_node());
        assert!(search_match.get_node().is_none());
    }
}
