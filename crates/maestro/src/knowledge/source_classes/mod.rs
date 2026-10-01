//! The source-class table this machine binds as `source_classes`: the
//! default adapter of search's source classifier. With nothing bound, search
//! ranks and labels as it did without one.

mod load;
#[cfg(test)]
mod tests;

pub(crate) use load::load;
