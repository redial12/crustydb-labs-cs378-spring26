use super::OpIterator;
use crate::Managers;

use common::bytecode_expr::ByteCodeExpr;
use common::{CrustyError, Field, TableSchema, Tuple};
use std::collections::HashMap;

/// Hash equi-join implementation. (You can add any other fields that you think are neccessary)
pub struct HashEqJoin {
    // Static objects (No need to reset on close)
    managers: &'static Managers,

    // Parameters (No need to reset on close)
    schema: TableSchema,
    left_expr: ByteCodeExpr,
    right_expr: ByteCodeExpr,
    left_child: Box<dyn OpIterator>,
    right_child: Box<dyn OpIterator>,
    // States (Need to reset on close)
    // todo!("Your code here")
    open: bool,
    current_tuple: Option<Tuple>,
    map: Option<HashMap<Field, Vec<Tuple>>>,
    current_matches: Vec<Tuple>,
    match_index: usize,
}

impl HashEqJoin {
    /// NestedLoopJoin constructor. Creates a new node for a nested-loop join.
    ///
    /// # Arguments
    ///
    /// * `left_expr` - ByteCodeExpr for the left field in join condition.
    /// * `right_expr` - ByteCodeExpr for the right field in join condition.
    /// * `left_child` - Left child of join operator.
    /// * `right_child` - Left child of join operator.
    pub fn new(
        managers: &'static Managers,
        schema: TableSchema,
        left_expr: ByteCodeExpr,
        right_expr: ByteCodeExpr,
        left_child: Box<dyn OpIterator>,
        right_child: Box<dyn OpIterator>,
    ) -> Self {
        Self {
            managers,
            schema,
            left_expr,
            right_expr,
            left_child,
            right_child,
            open: false,
            current_tuple: None,
            map: None,
            current_matches: Vec::new(),
            match_index: 0,
        }
    }
}

impl OpIterator for HashEqJoin {
    fn configure(&mut self, will_rewind: bool) {
        self.left_child.configure(will_rewind); // left child is rewound when HJ is rewound
        self.right_child.configure(false); // right child is consumed once in open() to build the map
    }

    fn open(&mut self) -> Result<(), CrustyError> {
        if !self.open {
            self.left_child.open()?;
            self.right_child.open()?;
            self.current_tuple = self.left_child.next()?;
            self.open = true;
            self.map = Some(HashMap::new());

            //construct hashmap from right

            //iterate through right child
            while let Some(right_tuple) = self.right_child.next()? {
                //evaluate right tuple againt right_expr
                let right_val = self.right_expr.eval(&right_tuple);

                //insert vec if empty, then append right tuple
                self.map
                    .as_mut()
                    .unwrap()
                    .entry(right_val)
                    .or_insert_with(Vec::new)
                    .push(right_tuple);
            }

            if let Some(left_tuple) = &self.current_tuple {
                let left_val = self.left_expr.eval(left_tuple);
                self.current_matches = self
                    .map
                    .as_mut()
                    .unwrap()
                    .get(&left_val)
                    .cloned()
                    .unwrap_or_default();
            }
        }

        Ok(())
    }

    fn next(&mut self) -> Result<Option<Tuple>, CrustyError> {
        if !self.open {
            panic!("Iterator is not open");
        }

        loop {
            if self.current_tuple.is_none() {
                return Ok(None);
            }

            // Still have matches for the current left tuple
            if self.match_index < self.current_matches.len() {
                let left_tuple = self.current_tuple.as_ref().unwrap();
                let right_tuple = &self.current_matches[self.match_index];
                let t = left_tuple.merge(right_tuple);
                self.match_index += 1;
                return Ok(Some(t));
            }

            // current_matches exhausted — advance to next left tuple
            self.current_tuple = self.left_child.next()?;
            if let Some(left_tuple) = &self.current_tuple {
                let left_val = self.left_expr.eval(left_tuple);
                self.current_matches = self
                    .map
                    .as_ref()
                    .unwrap()
                    .get(&left_val)
                    .cloned()
                    .unwrap_or_default();
            }
            self.match_index = 0;
        }
    }

    fn close(&mut self) -> Result<(), CrustyError> {
        self.left_child.close()?;
        self.right_child.close()?;
        self.open = false;
        Ok(())
    }

    fn rewind(&mut self) -> Result<(), CrustyError> {
        if !self.open {
            panic!("Iterator is not open");
        }
        self.left_child.rewind()?;
        self.current_tuple = self.left_child.next()?;
        if let Some(left_tuple) = &self.current_tuple {
            let left_val = self.left_expr.eval(left_tuple);
            self.current_matches = self
                .map
                .as_ref()
                .unwrap()
                .get(&left_val)
                .cloned()
                .unwrap_or_default();
        }
        self.match_index = 0;
        Ok(())
    }

    fn get_schema(&self) -> &TableSchema {
        &self.schema
    }
}

#[cfg(test)]
mod test {
    use super::super::TupleIterator;
    use super::*;
    use crate::testutil::execute_iter;
    use crate::testutil::new_test_managers;
    use crate::testutil::TestTuples;
    use common::bytecode_expr::{ByteCodeExpr, ByteCodes};

    fn get_join_predicate() -> (ByteCodeExpr, ByteCodeExpr) {
        // Joining two tables each containing the following tuples:
        // 1 1 3 E
        // 2 1 3 G
        // 3 1 4 A
        // 4 2 4 G
        // 5 2 5 G
        // 6 2 5 G

        // left(col(0) + col(1)) OP right(col(2))
        let mut left = ByteCodeExpr::new();
        left.add_code(ByteCodes::PushField as usize);
        left.add_code(0);
        left.add_code(ByteCodes::PushField as usize);
        left.add_code(1);
        left.add_code(ByteCodes::Add as usize);

        let mut right = ByteCodeExpr::new();
        right.add_code(ByteCodes::PushField as usize);
        right.add_code(2);

        (left, right)
    }

    fn get_iter(left_expr: ByteCodeExpr, right_expr: ByteCodeExpr) -> Box<dyn OpIterator> {
        let setup = TestTuples::new("");
        let managers = new_test_managers();
        let mut iter = Box::new(HashEqJoin::new(
            managers,
            setup.schema.clone(),
            left_expr,
            right_expr,
            Box::new(TupleIterator::new(
                setup.tuples.clone(),
                setup.schema.clone(),
            )),
            Box::new(TupleIterator::new(
                setup.tuples.clone(),
                setup.schema.clone(),
            )),
        ));
        iter.configure(false);
        iter
    }

    fn run_hash_eq_join(left_expr: ByteCodeExpr, right_expr: ByteCodeExpr) -> Vec<Tuple> {
        let mut iter = get_iter(left_expr, right_expr);
        execute_iter(&mut *iter, true).unwrap()
    }

    mod hash_eq_join_test {
        use super::*;

        #[test]
        #[should_panic]
        fn test_empty_predicate_join() {
            let left_expr = ByteCodeExpr::new();
            let right_expr = ByteCodeExpr::new();
            let _ = run_hash_eq_join(left_expr, right_expr);
        }

        #[test]
        fn test_join() {
            // Joining two tables each containing the following tuples:
            // 1 1 3 E
            // 2 1 3 G
            // 3 1 4 A
            // 4 2 4 G
            // 5 2 5 G
            // 6 2 5 G

            // left(col(0) + col(1)) == right(col(2))

            // Output:
            // 2 1 3 G 1 1 3 E
            // 2 1 3 G 2 1 3 G
            // 3 1 4 A 3 1 4 A
            // 3 1 4 A 4 2 4 G
            let (left_expr, right_expr) = get_join_predicate();
            let t = run_hash_eq_join(left_expr, right_expr);
            assert_eq!(t.len(), 4);
            assert_eq!(
                t[0],
                Tuple::new(vec![
                    Field::Int(2),
                    Field::Int(1),
                    Field::Int(3),
                    Field::String("G".to_string()),
                    Field::Int(1),
                    Field::Int(1),
                    Field::Int(3),
                    Field::String("E".to_string()),
                ])
            );
            assert_eq!(
                t[1],
                Tuple::new(vec![
                    Field::Int(2),
                    Field::Int(1),
                    Field::Int(3),
                    Field::String("G".to_string()),
                    Field::Int(2),
                    Field::Int(1),
                    Field::Int(3),
                    Field::String("G".to_string()),
                ])
            );
            assert_eq!(
                t[2],
                Tuple::new(vec![
                    Field::Int(3),
                    Field::Int(1),
                    Field::Int(4),
                    Field::String("A".to_string()),
                    Field::Int(3),
                    Field::Int(1),
                    Field::Int(4),
                    Field::String("A".to_string()),
                ])
            );
            assert_eq!(
                t[3],
                Tuple::new(vec![
                    Field::Int(3),
                    Field::Int(1),
                    Field::Int(4),
                    Field::String("A".to_string()),
                    Field::Int(4),
                    Field::Int(2),
                    Field::Int(4),
                    Field::String("G".to_string()),
                ])
            );
        }
    }

    mod opiterator_test {
        use super::*;
        #[test]
        #[should_panic]
        fn test_next_not_open() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            let _ = iter.next();
        }

        #[test]
        #[should_panic]
        fn test_rewind_not_open() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            let _ = iter.rewind();
        }

        #[test]
        fn test_open() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            iter.open().unwrap();
        }

        #[test]
        fn test_close() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            iter.open().unwrap();
            iter.close().unwrap();
        }

        #[test]
        fn test_rewind() {
            let (left_expr, right_expr) = get_join_predicate();
            let mut iter = get_iter(left_expr, right_expr);
            iter.configure(true);
            let t_before = execute_iter(&mut *iter, false).unwrap();
            iter.rewind().unwrap();
            let t_after = execute_iter(&mut *iter, false).unwrap();
            assert_eq!(t_before, t_after);
        }
    }
}
