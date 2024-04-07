use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
};

use crate::{context::ActorId, lookup::DependenceRelation};

pub fn has_cycles(relations: &[DependenceRelation]) -> bool {
    let mut graph: HashMap<ActorId, Node> = HashMap::new();

    for relation in relations {
        graph
            .entry(relation.from)
            .or_default()
            .children
            .insert(relation.to);
        graph.entry(relation.to).or_default();
    }

    for actor in graph.keys() {
        if dfs(*actor, &graph) {
            return true;
        }
    }
    false
}

#[derive(Default)]
struct Node {
    children: HashSet<ActorId>,
    state: Cell<State>,
}

#[derive(Default, Clone, Copy)]
enum State {
    #[default]
    Unvisited,
    Visiting,
    Visited,
}

fn dfs(node: ActorId, graph: &HashMap<ActorId, Node>) -> bool {
    use State as S;
    let node = graph.get(&node).unwrap();
    match node.state.get() {
        S::Visiting => return true,
        S::Visited => return false,
        S::Unvisited => {
            node.state.set(S::Visiting);
            for child in &node.children {
                if dfs(*child, graph) {
                    return true;
                }
            }
            node.state.set(S::Visited);
            false
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn has_cycles(t: impl IntoIterator<Item = (u32, u32)>) -> bool {
        let f = |a| ActorId::new(a + 1).unwrap();
        let v: Vec<DependenceRelation> = t
            .into_iter()
            .map(|(a, b)| DependenceRelation {
                from: f(a),
                to: f(b),
            })
            .collect();
        super::has_cycles(&v)
    }

    #[test]
    fn test_2cycle() {
        assert!(has_cycles([(1, 2), (2, 1)]));
    }

    #[test]
    fn diamond() {
        assert!(!has_cycles([(1, 2), (1, 3), (2, 4), (3, 4)]));
    }

    #[test]
    fn test_3cycle_with_offshoot() {
        assert!(has_cycles([(1, 2), (2, 3), (3, 4), (3, 1)]));
    }
}
