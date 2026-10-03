//! YAML frontmatter read node by node, never as a whole generic tree first:
//! each container is refused past the depth limit as it opens, and every
//! node read, each alias replay included, counts against a budget, so an
//! alias cannot expand a small file into a large tree.

use serde::de::{
    self, DeserializeSeed, EnumAccess, Error as _, IgnoredAny, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use std::{cell::Cell, collections::BTreeSet, fmt};

/// A YAML node as read, before the value model checks it.
#[derive(Debug)]
pub(super) enum Node {
    /// `null` or nothing.
    Null,
    /// A boolean.
    Boolean(bool),
    /// An integer that fits 64 signed bits.
    Integer(i64),
    /// An integer past 64 signed bits.
    Huge,
    /// A fraction, possibly not finite.
    Float(f64),
    /// A string.
    Text(String),
    /// A sequence.
    List(Vec<Node>),
    /// A mapping, its entries in order.
    Map(Vec<(Node, Node)>),
    /// A value under a YAML tag.
    Tagged,
}

/// Why reading stopped early.
#[derive(Debug, Clone, Copy)]
enum Stop {
    /// A container opened past the depth limit.
    Deep,
    /// More nodes than the budget.
    Budget,
}

/// What every node of one document shares.
#[derive(Debug)]
struct Reading {
    /// The most container levels, the root counting as one.
    depth: usize,
    /// The most nodes.
    budget: usize,
    /// The nodes read so far.
    nodes: Cell<usize>,
    /// Why reading stopped, if a limit stopped it.
    stop: Cell<Option<Stop>>,
}

impl Reading {
    /// Stops reading for `stop`.
    fn stop<E: de::Error>(&self, stop: Stop) -> E {
        self.stop.set(Some(stop));
        E::custom("limit reached")
    }
}

/// Reads one node `depth` containers deep.
#[derive(Debug, Clone, Copy)]
struct Seed<'a> {
    /// The containers around the node.
    depth: usize,
    /// The document's shared state.
    reading: &'a Reading,
}

impl Seed<'_> {
    /// The seed of a node inside a container this seed opens.
    fn inner<E: de::Error>(self) -> Result<Self, E> {
        let depth = self.depth + 1;
        if depth > self.reading.depth {
            return Err(self.reading.stop(Stop::Deep));
        }
        Ok(Self { depth, ..self })
    }
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Node;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Node, D::Error> {
        let nodes = self.reading.nodes.get() + 1;
        if nodes > self.reading.budget {
            return Err(self.reading.stop(Stop::Budget));
        }
        self.reading.nodes.set(nodes);
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Node;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a YAML node")
    }

    fn visit_bool<E: de::Error>(self, flag: bool) -> Result<Node, E> {
        Ok(Node::Boolean(flag))
    }

    fn visit_i64<E: de::Error>(self, integer: i64) -> Result<Node, E> {
        Ok(Node::Integer(integer))
    }

    fn visit_u64<E: de::Error>(self, integer: u64) -> Result<Node, E> {
        Ok(i64::try_from(integer).map_or(Node::Huge, Node::Integer))
    }

    fn visit_i128<E: de::Error>(self, _integer: i128) -> Result<Node, E> {
        Ok(Node::Huge)
    }

    fn visit_u128<E: de::Error>(self, _integer: u128) -> Result<Node, E> {
        Ok(Node::Huge)
    }

    fn visit_f64<E: de::Error>(self, number: f64) -> Result<Node, E> {
        Ok(Node::Float(number))
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Node, E> {
        Ok(Node::Text(text.to_owned()))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> Result<Node, A::Error> {
        let inner = self.inner()?;
        let mut list = Vec::new();
        while let Some(item) = items.next_element_seed(inner)? {
            list.push(item);
        }
        Ok(Node::List(list))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Node, A::Error> {
        let inner = self.inner()?;
        let mut seen = BTreeSet::new();
        let mut map = Vec::new();
        while let Some(key) = entries.next_key_seed(inner)? {
            if let Node::Text(name) = &key
                && !seen.insert(name.clone())
            {
                return Err(A::Error::custom(format!("duplicate key {name:?}")));
            }
            map.push((key, entries.next_value_seed(inner)?));
        }
        Ok(Node::Map(map))
    }

    fn visit_enum<A: EnumAccess<'de>>(self, tagged: A) -> Result<Node, A::Error> {
        let (IgnoredAny, value) = tagged.variant()?;
        value.newtype_variant_seed(self)?;
        Ok(Node::Tagged)
    }
}

/// The YAML document `yaml` as nodes, within `depth` container levels and
/// `budget` nodes.
///
/// # Errors
///
/// Why it cannot be read: a limit's message, or the parser's.
pub(super) fn read(yaml: &str, depth: usize, budget: usize) -> Result<Node, String> {
    let reading = Reading {
        depth,
        budget,
        nodes: Cell::new(0),
        stop: Cell::new(None),
    };
    let seed = Seed {
        depth: 0,
        reading: &reading,
    };
    seed.deserialize(serde_yaml_ng::Deserializer::from_str(yaml))
        .map_err(|error| match reading.stop.get() {
            Some(Stop::Deep) => format!("deeper than {depth} levels"),
            Some(Stop::Budget) => format!("more than {budget} YAML nodes"),
            None => format!("invalid frontmatter: {error}"),
        })
}
