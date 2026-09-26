use std::collections::{HashMap, HashSet, VecDeque};

use crate::core::CellError;

/// Coordinates for a cell (row, col)
pub type CellCoord = (u32, u32);

/// Tracks dependencies between cells for efficient recalculation
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// Maps a cell to the cells it depends on (formula inputs)
    /// e.g., if A1 = B1 + C1, then dependencies[A1] = {B1, C1}
    dependencies: HashMap<CellCoord, HashSet<CellCoord>>,

    /// Maps a cell to the cells that depend on it (reverse lookup)
    /// e.g., if A1 = B1 + C1, then dependents[B1] contains A1
    dependents: HashMap<CellCoord, HashSet<CellCoord>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update dependencies for a cell after formula change
    pub fn set_dependencies(&mut self, cell: CellCoord, deps: HashSet<CellCoord>) {
        // Remove old reverse dependencies
        if let Some(old_deps) = self.dependencies.get(&cell) {
            for dep in old_deps {
                if let Some(dependents) = self.dependents.get_mut(dep) {
                    dependents.remove(&cell);
                }
            }
        }

        // Add new reverse dependencies
        for dep in &deps {
            self.dependents.entry(*dep).or_default().insert(cell);
        }

        // Store new dependencies
        if deps.is_empty() {
            self.dependencies.remove(&cell);
        } else {
            self.dependencies.insert(cell, deps);
        }
    }

    /// Remove all dependencies for a cell (when cell is cleared)
    pub fn remove_cell(&mut self, cell: CellCoord) {
        self.set_dependencies(cell, HashSet::new());
    }

    /// Get cells that directly depend on the given cell
    pub fn get_direct_dependents(&self, cell: CellCoord) -> Option<&HashSet<CellCoord>> {
        self.dependents.get(&cell)
    }

    /// Get cells that the given cell directly depends on
    pub fn get_direct_dependencies(&self, cell: CellCoord) -> Option<&HashSet<CellCoord>> {
        self.dependencies.get(&cell)
    }

    /// Get all cells that need recalculation when a cell changes
    /// Returns cells in topological order (dependencies before dependents)
    pub fn get_recalc_order(&self, changed: CellCoord) -> Result<Vec<CellCoord>, CellError> {
        let mut to_recalc = Vec::new();
        let mut visited = HashSet::new();
        let mut in_progress = HashSet::new();

        // Find all cells affected by this change using BFS
        let mut queue = VecDeque::new();
        queue.push_back(changed);

        let mut affected = HashSet::new();
        while let Some(cell) = queue.pop_front() {
            if affected.contains(&cell) {
                continue;
            }
            affected.insert(cell);

            if let Some(dependents) = self.get_direct_dependents(cell) {
                for dependent in dependents {
                    queue.push_back(*dependent);
                }
            }
        }

        // Topologically sort the affected cells
        for cell in &affected {
            if !visited.contains(cell) {
                self.topological_sort(
                    *cell,
                    &affected,
                    &mut to_recalc,
                    &mut visited,
                    &mut in_progress,
                )?;
            }
        }

        Ok(to_recalc)
    }

    /// Depth-first topological sort
    fn topological_sort(
        &self,
        cell: CellCoord,
        affected: &HashSet<CellCoord>,
        result: &mut Vec<CellCoord>,
        visited: &mut HashSet<CellCoord>,
        in_progress: &mut HashSet<CellCoord>,
    ) -> Result<(), CellError> {
        if in_progress.contains(&cell) {
            return Err(CellError::CircularReference);
        }

        if visited.contains(&cell) {
            return Ok(());
        }

        in_progress.insert(cell);

        // Visit dependencies first (cells this cell depends on)
        if let Some(deps) = self.get_direct_dependencies(cell) {
            for dep in deps {
                if affected.contains(dep) {
                    self.topological_sort(*dep, affected, result, visited, in_progress)?;
                }
            }
        }

        in_progress.remove(&cell);
        visited.insert(cell);
        result.push(cell);

        Ok(())
    }

    /// Check if adding a dependency would create a circular reference
    pub fn would_create_cycle(&self, cell: CellCoord, new_dep: CellCoord) -> bool {
        // Check if new_dep (directly or indirectly) depends on cell
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(new_dep);

        while let Some(current) = queue.pop_front() {
            if current == cell {
                return true;
            }

            if visited.contains(&current) {
                continue;
            }
            visited.insert(current);

            if let Some(deps) = self.get_direct_dependencies(current) {
                for dep in deps {
                    queue.push_back(*dep);
                }
            }
        }

        false
    }

    /// Get all cells that have formulas (have dependencies)
    pub fn cells_with_formulas(&self) -> impl Iterator<Item = CellCoord> + '_ {
        self.dependencies.keys().copied()
    }

    /// Clear all dependencies
    pub fn clear(&mut self) {
        self.dependencies.clear();
        self.dependents.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_dependency() {
        let mut graph = DependencyGraph::new();

        // A1 = B1 + C1
        let a1 = (0, 0);
        let b1 = (0, 1);
        let c1 = (0, 2);

        let mut deps = HashSet::new();
        deps.insert(b1);
        deps.insert(c1);
        graph.set_dependencies(a1, deps);

        // Check direct dependencies
        assert!(graph.get_direct_dependencies(a1).unwrap().contains(&b1));
        assert!(graph.get_direct_dependencies(a1).unwrap().contains(&c1));

        // Check reverse dependencies
        assert!(graph.get_direct_dependents(b1).unwrap().contains(&a1));
        assert!(graph.get_direct_dependents(c1).unwrap().contains(&a1));
    }

    #[test]
    fn test_recalc_order() {
        let mut graph = DependencyGraph::new();

        // A1 = 10 (no dependencies)
        // B1 = A1 * 2
        // C1 = B1 + A1

        let a1 = (0, 0);
        let b1 = (0, 1);
        let c1 = (0, 2);

        // B1 depends on A1
        let mut deps_b1 = HashSet::new();
        deps_b1.insert(a1);
        graph.set_dependencies(b1, deps_b1);

        // C1 depends on B1 and A1
        let mut deps_c1 = HashSet::new();
        deps_c1.insert(b1);
        deps_c1.insert(a1);
        graph.set_dependencies(c1, deps_c1);

        // When A1 changes, recalc order should be: A1, B1, C1
        let order = graph.get_recalc_order(a1).unwrap();

        // B1 should come before C1 (since C1 depends on B1)
        let b1_pos = order.iter().position(|&c| c == b1);
        let c1_pos = order.iter().position(|&c| c == c1);

        assert!(b1_pos.is_some());
        assert!(c1_pos.is_some());
        assert!(b1_pos.unwrap() < c1_pos.unwrap());
    }

    #[test]
    fn test_circular_reference() {
        let mut graph = DependencyGraph::new();

        let a1 = (0, 0);
        let b1 = (0, 1);

        // A1 = B1
        let mut deps_a1 = HashSet::new();
        deps_a1.insert(b1);
        graph.set_dependencies(a1, deps_a1);

        // B1 = A1 (creates cycle)
        let mut deps_b1 = HashSet::new();
        deps_b1.insert(a1);
        graph.set_dependencies(b1, deps_b1);

        // Trying to recalculate should detect the cycle
        let result = graph.get_recalc_order(a1);
        assert!(matches!(result, Err(CellError::CircularReference)));
    }

    #[test]
    fn test_would_create_cycle() {
        let mut graph = DependencyGraph::new();

        let a1 = (0, 0);
        let b1 = (0, 1);
        let c1 = (0, 2);

        // A1 = B1
        let mut deps = HashSet::new();
        deps.insert(b1);
        graph.set_dependencies(a1, deps);

        // B1 = C1
        let mut deps = HashSet::new();
        deps.insert(c1);
        graph.set_dependencies(b1, deps);

        // Would C1 = A1 create a cycle? Yes!
        assert!(graph.would_create_cycle(c1, a1));

        // Would C1 = some other cell create a cycle? No
        assert!(!graph.would_create_cycle(c1, (0, 3)));
    }
}
