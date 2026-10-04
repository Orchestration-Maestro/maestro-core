//! The dependency graph across resources, its nodes numbered in ID order:
//! strongly connected components by an iterative Tarjan walk, so no chain
//! length can exhaust the stack, and one memoized pass over them for what
//! each node reaches.

/// A node's state in the walk.
#[derive(Debug, Clone, Copy, Default)]
struct Visit {
    /// Its position in the walk, once entered.
    order: Option<usize>,
    /// The lowest position it reaches on the stack.
    low: usize,
    /// Whether it is on the stack.
    stacked: bool,
}

/// Tarjan's walk over `targets`.
#[derive(Debug)]
struct Tarjan<'a> {
    /// Each node's targets.
    targets: &'a [Vec<usize>],
    /// Each node's state.
    visits: Vec<Visit>,
    /// The nodes whose component is still open.
    stack: Vec<usize>,
    /// The next position.
    order: usize,
    /// The components found, each after every component it reaches.
    components: Vec<Vec<usize>>,
}

impl Tarjan<'_> {
    /// The state of `node`.
    fn visit(&self, node: usize) -> Visit {
        self.visits.get(node).copied().unwrap_or_default()
    }

    /// Enters `node`.
    fn enter(&mut self, node: usize) {
        if let Some(visit) = self.visits.get_mut(node) {
            *visit = Visit {
                order: Some(self.order),
                low: self.order,
                stacked: true,
            };
        }
        self.order += 1;
        self.stack.push(node);
    }

    /// Lowers the stack position `node` reaches to `low`.
    fn lower(&mut self, node: usize, low: usize) {
        if let Some(visit) = self.visits.get_mut(node) {
            visit.low = visit.low.min(low);
        }
    }

    /// Closes the component whose first-entered node is `node`.
    fn close(&mut self, node: usize) {
        let mut component = Vec::new();
        while let Some(member) = self.stack.pop() {
            if let Some(visit) = self.visits.get_mut(member) {
                visit.stacked = false;
            }
            component.push(member);
            if member == node {
                break;
            }
        }
        component.sort_unstable();
        self.components.push(component);
    }

    /// Follows the edge from `node`, the last open call of `calls`, to
    /// `target`.
    fn follow(&mut self, node: usize, target: usize, calls: &mut Vec<(usize, usize)>) {
        if let Some(call) = calls.last_mut() {
            call.1 += 1;
        }
        let seen = self.visit(target);
        match seen.order {
            None => {
                self.enter(target);
                calls.push((target, 0));
            }
            Some(order) if seen.stacked => self.lower(node, order),
            Some(_) => {}
        }
    }

    /// Leaves `node`, whose edges are all followed, back to `parent`.
    fn leave(&mut self, node: usize, parent: Option<usize>) {
        let visit = self.visit(node);
        if let Some(parent) = parent {
            self.lower(parent, visit.low);
        }
        if visit.order == Some(visit.low) {
            self.close(node);
        }
    }

    /// Walks every node `root` reaches that is not yet walked, with an
    /// explicit stack of each open node and its next edge.
    fn walk(&mut self, root: usize) {
        if self.visit(root).order.is_some() {
            return;
        }
        self.enter(root);
        let mut calls = vec![(root, 0)];
        while let Some(&(node, edge)) = calls.last() {
            let target = self
                .targets
                .get(node)
                .and_then(|targets| targets.get(edge))
                .copied();
            if let Some(target) = target {
                self.follow(node, target, &mut calls);
            } else {
                calls.pop();
                self.leave(node, calls.last().map(|&(parent, _)| parent));
            }
        }
    }
}

/// The strongly connected components of the graph whose node `n` has the
/// targets `targets[n]`, each sorted, each after every component it reaches.
pub(super) fn components(targets: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut tarjan = Tarjan {
        targets,
        visits: vec![Visit::default(); targets.len()],
        stack: Vec::new(),
        order: 0,
        components: Vec::new(),
    };
    for root in 0..targets.len() {
        tarjan.walk(root);
    }
    tarjan.components
}

/// Whether `component` is a cycle: two members or more, or one that
/// targets itself.
pub(super) fn is_cycle(component: &[usize], targets: &[Vec<usize>]) -> bool {
    match component {
        [node] => targets
            .get(*node)
            .is_some_and(|targets| targets.contains(node)),
        _ => true,
    }
}

/// For each node, the lowest node it reaches, itself included, that is
/// `flagged`, given the graph's `components` in the order
/// [`components`] returns them.
pub(super) fn first_flagged(
    targets: &[Vec<usize>],
    components: &[Vec<usize>],
    flagged: &[bool],
) -> Vec<Option<usize>> {
    let mut first = vec![None; targets.len()];
    for component in components {
        let reached = component
            .iter()
            .flat_map(|member| targets.get(*member).into_iter().flatten())
            .filter_map(|target| first.get(*target).copied().flatten());
        let own = component
            .iter()
            .copied()
            .filter(|member| flagged.get(*member).copied().unwrap_or_default());
        let lowest = own.chain(reached).min();
        for member in component {
            if let Some(slot) = first.get_mut(*member) {
                *slot = lowest;
            }
        }
    }
    first
}

#[cfg(test)]
mod tests {
    use super::components;
    use std::{sync::mpsc, thread, time::Duration};

    #[test]
    fn tarjan_follow_finishes_and_orders_connected_components() {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = components(&[vec![1], vec![0, 2], vec![]]);
            let _sent = sender.send(result);
        });
        assert_eq!(
            receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("Tarjan walk did not finish"),
            vec![vec![2], vec![0, 1]],
        );
    }
}
