//! Port of `src/parser/htmlParser.ts`.
//!
//! The document tree is stored in an arena (`Vec<Node>`) addressed by
//! [`NodeId`], avoiding reference cycles in the parent links.

use crate::html_language_types::{ScannerState, TokenType};
use crate::parser::html_scanner::create_scanner;
use crate::utils::arrays::{binary_search, find_first};

/// Index of a node within an [`HtmlDocument`]'s arena.
pub type NodeId = usize;

/// Port of the parser `Node` class.
#[derive(Debug, Clone)]
pub struct Node {
    pub tag: Option<String>,
    pub closed: bool,
    pub start_tag_end: Option<usize>,
    pub end_tag_start: Option<usize>,
    /// Insertion-ordered attributes. `None` value means a valueless attribute.
    pub attributes: Vec<(String, Option<String>)>,
    pub start: usize,
    pub end: usize,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
}

impl Node {
    fn new(start: usize, end: usize, parent: Option<NodeId>) -> Self {
        Self {
            tag: None,
            closed: false,
            start_tag_end: None,
            end_tag_start: None,
            attributes: Vec::new(),
            start,
            end,
            children: Vec::new(),
            parent,
        }
    }

    /// Port of `Node.attributeNames` (`Object.keys(attributes)`).
    pub fn attribute_names(&self) -> Vec<&str> {
        self.attributes
            .iter()
            .map(|(name, _)| name.as_str())
            .collect()
    }

    pub fn get_attribute(&self, name: &str) -> Option<&Option<String>> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }

    /// Upserts an attribute, preserving the position of an existing name.
    fn set_attribute(&mut self, name: &str, value: Option<String>) {
        if let Some(entry) = self.attributes.iter_mut().find(|(n, _)| n == name) {
            entry.1 = value;
        } else {
            self.attributes.push((name.to_string(), value));
        }
    }

    /// Port of `Node.isSameTag`.
    pub fn is_same_tag(&self, tag_in_lower_case: Option<&str>) -> bool {
        match &self.tag {
            None => tag_in_lower_case.is_none(),
            Some(tag) => match tag_in_lower_case {
                None => false,
                Some(other) => tag.len() == other.len() && tag.to_lowercase() == other,
            },
        }
    }
}

/// Parsed HTML document: an arena of [`Node`]s plus the root node ids.
#[derive(Debug, Clone)]
pub struct HtmlDocument {
    nodes: Vec<Node>,
    roots: Vec<NodeId>,
}

impl HtmlDocument {
    /// Root ids (children of the synthetic document node).
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id].children
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id].parent
    }

    /// Port of the top-level `findNodeBefore` (starts at the document node).
    pub fn find_node_before(&self, offset: usize) -> NodeId {
        self.find_node_before_at(0, offset)
    }

    /// Port of the top-level `findNodeAt` (starts at the document node).
    pub fn find_node_at(&self, offset: usize) -> NodeId {
        self.find_node_at_node(0, offset)
    }

    fn find_node_before_at(&self, id: NodeId, offset: usize) -> NodeId {
        let children = &self.nodes[id].children;
        let idx = find_first(children, |c| offset <= self.nodes[*c].start) as isize - 1;
        if idx >= 0 {
            let child = children[idx as usize];
            if offset > self.nodes[child].start {
                if offset < self.nodes[child].end {
                    return self.find_node_before_at(child, offset);
                }
                if let Some(&last_child) = self.nodes[child].children.last()
                    && self.nodes[last_child].end == self.nodes[child].end
                {
                    return self.find_node_before_at(child, offset);
                }
                return child;
            }
        }
        id
    }

    fn find_node_at_node(&self, id: NodeId, offset: usize) -> NodeId {
        let children = &self.nodes[id].children;
        let idx = find_first(children, |c| offset <= self.nodes[*c].start) as isize - 1;
        if idx >= 0 {
            let child = children[idx as usize];
            if offset > self.nodes[child].start && offset <= self.nodes[child].end {
                return self.find_node_at_node(child, offset);
            }
        }
        id
    }
}

/// Options for [`parse`], mirroring the parser `options` argument.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseOptions {
    /// When true, closing tags without a matching opening tag produce nodes.
    pub create_nodes_for_orphan_end_tags: bool,
}

/// Port of `HTMLParser.parse`.
pub fn parse(text: &str, void_elements: &[String], options: ParseOptions) -> HtmlDocument {
    let len = text.encode_utf16().count();
    let mut scanner = create_scanner(text, 0, ScannerState::WithinContent, true);

    let mut nodes: Vec<Node> = vec![Node::new(0, len, None)];
    let mut curr: NodeId = 0;
    let mut end_tag_start: Option<usize> = None;
    let mut end_tag_name: Option<String> = None;
    let mut pending_attribute: Option<String> = None;

    let mut token = scanner.scan();
    while token != TokenType::Eos {
        match token {
            TokenType::StartTagOpen => {
                let child = nodes.len();
                nodes.push(Node::new(scanner.get_token_offset(), len, Some(curr)));
                nodes[curr].children.push(child);
                curr = child;
            }
            TokenType::StartTag => {
                nodes[curr].tag = Some(scanner.get_token_text());
            }
            TokenType::StartTagClose => {
                if nodes[curr].parent.is_some() {
                    nodes[curr].end = scanner.get_token_end();
                    if scanner.get_token_length() > 0 {
                        nodes[curr].start_tag_end = Some(scanner.get_token_end());
                        let is_void = nodes[curr]
                            .tag
                            .as_deref()
                            .map(|tag| is_void_element(tag, void_elements))
                            .unwrap_or(false);
                        if is_void {
                            nodes[curr].closed = true;
                            curr = nodes[curr].parent.unwrap();
                        }
                    } else {
                        curr = nodes[curr].parent.unwrap();
                    }
                }
            }
            TokenType::StartTagSelfClose => {
                if nodes[curr].parent.is_some() {
                    nodes[curr].closed = true;
                    nodes[curr].end = scanner.get_token_end();
                    nodes[curr].start_tag_end = Some(scanner.get_token_end());
                    curr = nodes[curr].parent.unwrap();
                }
            }
            TokenType::EndTagOpen => {
                end_tag_start = Some(scanner.get_token_offset());
                end_tag_name = None;
            }
            TokenType::EndTag => {
                end_tag_name = Some(scanner.get_token_text().to_lowercase());
            }
            TokenType::EndTagClose => {
                let mut node = curr;
                while !nodes[node].is_same_tag(end_tag_name.as_deref())
                    && nodes[node].parent.is_some()
                {
                    node = nodes[node].parent.unwrap();
                }
                if nodes[node].parent.is_some() {
                    while curr != node {
                        nodes[curr].end = end_tag_start.unwrap_or(0);
                        nodes[curr].closed = false;
                        curr = nodes[curr].parent.unwrap();
                    }
                    nodes[curr].closed = true;
                    nodes[curr].end_tag_start = end_tag_start;
                    nodes[curr].end = scanner.get_token_end();
                    curr = nodes[curr].parent.unwrap();
                } else if options.create_nodes_for_orphan_end_tags {
                    let orphan = nodes.len();
                    let start = end_tag_start.unwrap_or(0);
                    let end_pos = scanner.get_token_end();
                    let mut node = Node::new(start, end_pos, Some(curr));
                    node.tag = end_tag_name.clone();
                    node.closed = true;
                    node.end_tag_start = end_tag_start;
                    nodes.push(node);
                    nodes[curr].children.push(orphan);
                }
            }
            TokenType::AttributeName => {
                let name = scanner.get_token_text();
                nodes[curr].set_attribute(&name, None);
                pending_attribute = Some(name);
            }
            TokenType::AttributeValue => {
                let value = scanner.get_token_text();
                if pending_attribute.is_some() {
                    let name = pending_attribute.take().unwrap();
                    nodes[curr].set_attribute(&name, Some(value));
                }
            }
            _ => {}
        }
        token = scanner.scan();
    }

    while let Some(parent) = nodes[curr].parent {
        nodes[curr].end = len;
        nodes[curr].closed = false;
        curr = parent;
    }

    let roots = nodes[0].children.clone();
    HtmlDocument { nodes, roots }
}

/// Port of `HTMLDataManager.isVoidElement`. `void_elements` must be sorted.
pub fn is_void_element(e: &str, void_elements: &[String]) -> bool {
    if e.is_empty() {
        return false;
    }
    let lower = e.to_lowercase();
    binary_search(void_elements, &lower, |a, b| {
        a.as_str().cmp(b.as_str()) as i32
    }) >= 0
}
