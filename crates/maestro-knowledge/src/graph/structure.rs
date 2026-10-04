//! The tables of a canonical document as canonicalization gives them: each
//! table block with its rows, header first, and each row with its cells, in
//! source order. No Markdown is parsed here, and nothing here is projected:
//! tables, rows and cells are source references, never graph nodes.

use maestro_canonicalization::{Block, BlockType, CanonicalDocument};

/// A canonical table and its rows.
pub(super) struct Table<'d> {
    /// The table block.
    pub(super) block: &'d Block,
    /// Its header and body rows, in source order.
    pub(super) rows: Vec<Row<'d>>,
}

/// A canonical table row and its cells.
pub(super) struct Row<'d> {
    /// The row block: a header or a body row.
    pub(super) block: &'d Block,
    /// Its cells, in source order.
    pub(super) cells: Vec<&'d Block>,
}

/// Every table of `document`, in source order.
pub(super) fn tables(document: &CanonicalDocument) -> Vec<Table<'_>> {
    let blocks = &document.blocks;
    blocks
        .iter()
        .filter(|block| block.block_type == BlockType::Table)
        .map(|table| Table {
            block: table,
            rows: children(blocks, table, &[BlockType::TableHead, BlockType::TableRow])
                .into_iter()
                .map(|row| Row {
                    block: row,
                    cells: children(blocks, row, &[BlockType::TableCell]),
                })
                .collect(),
        })
        .collect()
}

/// The blocks of `blocks` whose parent is `parent` and whose type is one of
/// `types`, in source order.
fn children<'d>(blocks: &'d [Block], parent: &Block, types: &[BlockType]) -> Vec<&'d Block> {
    blocks
        .iter()
        .filter(|block| {
            block.parent_block_id.as_ref() == Some(&parent.block_id)
                && types.contains(&block.block_type)
        })
        .collect()
}
